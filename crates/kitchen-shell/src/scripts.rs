//! The Scripts workspace: one space to write the app's custom commands and pipeline steps, in
//! Python or Rust. Scripts tree on the left, the editor (kitchen-ui's `CodeView`: highlighting,
//! errors on their lines) in the middle, the run panel (output, the result, errors) under it.
//!
//! A script runs against what is in front of the user: it reads one JSON object on its stdin,
//! `{"args": …, "context": …}` (the context is what the app says is shown: the image, the
//! selection, the table), and may print its result as JSON on its last line of output.
//!
//! A few comment lines at the top say what a script is ([`ScriptMeta`]):
//!
//! ```text
//! # kitchen: command = Count cells
//! # kitchen: hint = Counts the cells of the shown image
//! # kitchen: args = {"type": "object", "properties": {"min_area": {"type": "number"}}}
//! # kitchen: node = in image:image; out counts:table
//! ```
//!
//! (`//` in Rust.) With `command`, the script becomes a command ([`ScriptsWorkspace::register`]):
//! in the menus, the search and an agent's tools, its arguments the schema given. With `node`, it
//! becomes a pipeline node ([`ScriptsWorkspace::nodes`]) with typed inputs and outputs, whose
//! types are the app's ([`ScriptsWorkspace::new`] takes them). Running a script is a job with a
//! cancel button; what it ran is reported ([`ScriptsEvent::Finished`]) for the app to record in
//! its session.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px,
};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{CodeChanged, CodeLanguage, CodeView, EmptyState};
use serde_json::{Value, json};

use crate::command::{CommandSpec, Registry};
use crate::jobs;

/// A typed port of a script node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    /// Its name (`image`).
    pub name: String,
    /// Its type, one the app declares (`image`, `mask`, `labels`, `table`; `signal`, `events`).
    pub ty: String,
}

/// What a script's header says it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptMeta {
    /// As a command: its title.
    pub command: Option<String>,
    /// What it does.
    pub hint: Option<String>,
    /// The JSON Schema of its arguments (as a command).
    pub args: Option<Value>,
    /// As a pipeline node: its inputs and outputs.
    pub node: Option<(Vec<Port>, Vec<Port>)>,
    /// What in the header did not read (line, why).
    pub problems: Vec<(usize, String)>,
}

impl ScriptMeta {
    /// Reads the `kitchen:` lines at the top of `text` (`#` or `//` comments, up to the first
    /// line that is neither a comment nor blank).
    pub fn parse(text: &str) -> ScriptMeta {
        let mut m = ScriptMeta::default();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("#!") {
                continue;
            }
            let Some(comment) = t.strip_prefix('#').or_else(|| t.strip_prefix("//")) else { break };
            let Some(rest) = comment.trim().strip_prefix("kitchen:") else { continue };
            let Some((key, value)) = rest.split_once('=') else {
                m.problems.push((i + 1, "expected `kitchen: key = value`".into()));
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "command" => m.command = Some(value.to_string()),
                "hint" => m.hint = Some(value.to_string()),
                "args" => match serde_json::from_str::<Value>(value) {
                    Ok(v) if v.is_object() => m.args = Some(v),
                    Ok(_) => m.problems.push((i + 1, "args: a JSON Schema object".into())),
                    Err(e) => m.problems.push((i + 1, format!("args: {e}"))),
                },
                "node" => match parse_node(value) {
                    Ok(n) => m.node = Some(n),
                    Err(e) => m.problems.push((i + 1, format!("node: {e}"))),
                },
                other => m.problems.push((i + 1, format!("unknown key {other:?} (command, hint, args, node)"))),
            }
        }
        m
    }
}

/// `in image:image, mask:mask; out counts:table`.
fn parse_node(text: &str) -> Result<(Vec<Port>, Vec<Port>), String> {
    let ports = |list: &str| -> Result<Vec<Port>, String> {
        list.split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| match p.split_once(':') {
            Some((n, t)) if !n.trim().is_empty() && !t.trim().is_empty() => Ok(Port { name: n.trim().into(), ty: t.trim().into() }),
            _ => Err(format!("{p:?}: expected name:type")),
        }).collect()
    };
    let (mut inputs, mut outputs) = (Vec::new(), Vec::new());
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(l) = part.strip_prefix("in ") {
            inputs = ports(l)?;
        } else if let Some(l) = part.strip_prefix("out ") {
            outputs = ports(l)?;
        } else {
            return Err(format!("{part:?}: expected `in …` or `out …`"));
        }
    }
    if outputs.is_empty() {
        return Err("a node needs at least one output".into());
    }
    Ok((inputs, outputs))
}

/// A script file.
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// Its file.
    pub path: PathBuf,
    /// Python or Rust.
    pub language: CodeLanguage,
    /// What its header says.
    pub meta: ScriptMeta,
}

impl Script {
    /// The script at `path` (`.py` or `.rs`), its header read.
    pub fn load(path: &Path) -> Option<Script> {
        let language = CodeLanguage::from_name(path.extension()?.to_str()?);
        if !matches!(language, CodeLanguage::Python | CodeLanguage::Rust) {
            return None;
        }
        let text = std::fs::read_to_string(path).ok()?;
        Some(Script { path: path.to_path_buf(), language, meta: ScriptMeta::parse(&text) })
    }

    /// Its name (the file's stem).
    pub fn name(&self) -> String {
        self.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Its command id, when it is a command (`script.count_cells`).
    pub fn command_id(&self) -> Option<String> {
        self.meta.command.as_ref().map(|_| format!("script.{}", self.name()))
    }
}

/// What a run of a script produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunOutput {
    /// Whether it exited well.
    pub ok: bool,
    /// What it printed (stdout, then stderr).
    pub stdout: String,
    /// Its error output.
    pub stderr: String,
    /// Its result: the last line of its output, when that is JSON.
    pub result: Option<Value>,
    /// Errors on lines of the script (Python tracebacks, Rust compiler messages): line, message.
    pub errors: Vec<(usize, String)>,
}

/// Runs a script with its input (blocking; called off the UI thread).
pub type Runner = Arc<dyn Fn(&Script, &Value) -> Result<RunOutput, String> + Send + Sync>;

/// The runner used by default: Python with `python3`; Rust with `rust-script` (install it with
/// `cargo install rust-script`). The input goes to the script's stdin.
pub fn default_runner() -> Runner {
    Arc::new(|script: &Script, input: &Value| {
        let mut cmd = match script.language {
            CodeLanguage::Python => {
                let mut c = Command::new(if cfg!(windows) { "python" } else { "python3" });
                c.arg("-u").arg(&script.path);
                c
            }
            CodeLanguage::Rust => {
                let mut c = Command::new("rust-script");
                c.arg(&script.path);
                c
            }
            other => return Err(format!("{} scripts do not run", other.name())),
        };
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| match script.language {
            CodeLanguage::Rust => format!("Rust scripts run with rust-script ({e}): cargo install rust-script"),
            _ => format!("python3: {e}"),
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write as _;
            let _ = writeln!(stdin, "{input}");
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned());
        let result = stdout.lines().rev().find(|l| !l.trim().is_empty()).and_then(|l| serde_json::from_str::<Value>(l.trim()).ok()).filter(|v| v.is_object() || v.is_array());
        let errors = script_errors(script, &stderr);
        Ok(RunOutput { ok: out.status.success(), stdout, stderr, result, errors })
    })
}

/// Errors on the script's own lines in `stderr`: a Python traceback's last frame in the script
/// (`File "…/count.py", line 7`) with its last line, or a Rust message (`--> count.rs:7:5`).
pub fn script_errors(script: &Script, stderr: &str) -> Vec<(usize, String)> {
    let file = script.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    let mut out = Vec::new();
    match script.language {
        CodeLanguage::Python => {
            let line = stderr
                .lines()
                .filter(|l| l.contains("File \"") && l.contains(&file))
                .filter_map(|l| l.split(", line ").nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse::<usize>().ok())
                .last();
            if let Some(n) = line {
                out.push((n, last));
            }
        }
        CodeLanguage::Rust => {
            let mut message = String::new();
            for l in stderr.lines() {
                if l.starts_with("error") {
                    message = l.trim().to_string();
                } else if let Some(at) = l.trim().strip_prefix("--> ")
                    && at.contains(&file)
                    && let Some(n) = at.split(':').nth(1).and_then(|n| n.parse::<usize>().ok())
                {
                    out.push((n, message.clone()));
                }
            }
        }
        _ => {}
    }
    out
}

/// A script as a pipeline node: typed ports, run by its script.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    /// Its id (`script.<name>`).
    pub id: String,
    /// Its title (the command title, else the file name).
    pub title: String,
    /// Its inputs.
    pub inputs: Vec<Port>,
    /// Its outputs.
    pub outputs: Vec<Port>,
    /// The script that runs it.
    pub script: PathBuf,
}

/// What the workspace tells its owner.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptsEvent {
    /// A script ran (record it in the session).
    Finished {
        /// The script.
        script: PathBuf,
        /// Its input.
        input: Value,
        /// What it produced.
        output: RunOutput,
    },
    /// Scripts were added, saved or removed: commands and nodes may have changed.
    ScriptsChanged,
}

/// Gives the script the context: what is shown now.
pub type ContextFn = Rc<dyn Fn(&App) -> Value>;

/// The Scripts workspace.
pub struct ScriptsWorkspace {
    dir: PathBuf,
    port_types: Vec<String>,
    runner: Runner,
    context: ContextFn,
    scripts: Vec<Script>,
    selected: Option<usize>,
    editor: Option<(Entity<CodeView>, Subscription)>,
    /// The editor's text differs from the file.
    dirty: bool,
    running: Option<u64>,
    output: Option<RunOutput>,
    message: Option<(bool, String)>,
    registered: Vec<String>,
}

impl EventEmitter<ScriptsEvent> for ScriptsWorkspace {}

impl ScriptsWorkspace {
    /// The scripts in `dir` (created when missing); `port_types` are the types a node may use,
    /// `context` what a script sees as shown.
    pub fn new(dir: impl Into<PathBuf>, port_types: Vec<String>, context: impl Fn(&App) -> Value + 'static, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        let mut ws = Self {
            dir,
            port_types,
            runner: default_runner(),
            context: Rc::new(context),
            scripts: Vec::new(),
            selected: None,
            editor: None,
            dirty: false,
            running: None,
            output: None,
            message: None,
            registered: Vec::new(),
        };
        ws.rescan(cx);
        if !ws.scripts.is_empty() {
            ws.select(0, window, cx);
        }
        ws
    }

    /// Runs scripts with `runner` instead of the default.
    pub fn set_runner(&mut self, runner: Runner) {
        self.runner = runner;
    }

    /// The scripts found.
    pub fn scripts(&self) -> &[Script] {
        &self.scripts
    }

    /// The selected script.
    pub fn selected(&self) -> Option<&Script> {
        self.selected.and_then(|i| self.scripts.get(i))
    }

    /// The last run's output.
    pub fn output(&self) -> Option<&RunOutput> {
        self.output.as_ref()
    }

    /// Whether a script runs now.
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// The editor, when a script is open.
    pub fn editor(&self) -> Option<Entity<CodeView>> {
        self.editor.as_ref().map(|(e, _)| e.clone())
    }

    /// Reads the folder again.
    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        let selected = self.selected().map(|s| s.path.clone());
        let mut scripts: Vec<Script> = std::fs::read_dir(&self.dir).into_iter().flatten().flatten().filter_map(|e| Script::load(&e.path())).collect();
        scripts.sort_by(|a, b| a.path.cmp(&b.path));
        self.selected = selected.and_then(|p| scripts.iter().position(|s| s.path == p));
        self.scripts = scripts;
        cx.emit(ScriptsEvent::ScriptsChanged);
        cx.notify();
    }

    /// Opens script `i` in the editor.
    pub fn select(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.scripts.get(i) else { return };
        let text = std::fs::read_to_string(&s.path).unwrap_or_default();
        let language = s.language;
        let problems = s.meta.problems.clone();
        let editor = cx.new(|cx| CodeView::new(language, text, window, cx).editable(true).errors(problems));
        let sub = cx.subscribe(&editor, |this, _, _: &CodeChanged, cx| {
            this.dirty = true;
            cx.notify();
        });
        self.editor = Some((editor, sub));
        self.selected = Some(i);
        self.dirty = false;
        self.output = None;
        self.message = None;
        cx.notify();
    }

    /// Writes the editor's text to the selected script, and reads its header again.
    pub fn save(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let (Some(i), Some(editor)) = (self.selected, self.editor()) else { return Err("no script open".into()) };
        let text = editor.read(cx).text(cx);
        let path = self.scripts[i].path.clone();
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.dirty = false;
        self.rescan(cx);
        let problems = self.selected().map(|s| s.meta.problems.clone()).unwrap_or_default();
        editor.update(cx, |e, cx| e.set_errors(problems, cx));
        self.message = Some((false, format!("Saved {}", path.display())));
        Ok(())
    }

    /// A new script in `language`, from a template, opened.
    pub fn new_script(&mut self, language: CodeLanguage, window: &mut Window, cx: &mut Context<Self>) -> Result<PathBuf, String> {
        let ext = if language == CodeLanguage::Rust { "rs" } else { "py" };
        let path = (1..1000).map(|n| self.dir.join(format!("script_{n}.{ext}"))).find(|p| !p.exists()).ok_or("too many scripts")?;
        let text = match language {
            CodeLanguage::Rust => "// kitchen: command = My command\n// kitchen: hint = What it does\n\nuse std::io::Read;\n\nfn main() {\n    let mut input = String::new();\n    std::io::stdin().read_to_string(&mut input).unwrap();\n    println!(\"{{\\\"ok\\\": true}}\");\n}\n",
            _ => "# kitchen: command = My command\n# kitchen: hint = What it does\nimport json, sys\n\ninput = json.load(sys.stdin)\ncontext = input[\"context\"]\nprint(json.dumps({\"ok\": True}))\n",
        };
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.rescan(cx);
        if let Some(i) = self.scripts.iter().position(|s| s.path == path) {
            self.select(i, window, cx);
        }
        Ok(path)
    }

    /// Runs the selected script (saved first) with `args`, off the UI thread, as a job.
    pub fn run_selected(&mut self, args: Value, cx: &mut Context<Self>) -> Result<(), String> {
        if self.dirty {
            self.save(cx)?;
        }
        let script = self.selected().cloned().ok_or("no script open")?;
        self.run(script, args, cx)
    }

    /// Runs `script` with `args`, off the UI thread, as a job; the output shows in the run panel.
    pub fn run(&mut self, script: Script, args: Value, cx: &mut Context<Self>) -> Result<(), String> {
        if self.running.is_some() {
            return Err("a script is running".into());
        }
        let job = jobs::start("script", &format!("Running {}", script.name()), false)?;
        self.running = Some(job.id);
        self.message = None;
        let input = json!({ "args": args, "context": (self.context)(cx) });
        let runner = self.runner.clone();
        let (s2, i2) = (script.clone(), input.clone());
        cx.spawn(async move |this, cx| {
            let out = cx.background_executor().spawn(async move { runner(&s2, &i2) }).await;
            let cancelled = job.cancelled();
            drop(job);
            let _ = this.update(cx, |w, cx| {
                w.running = None;
                match out {
                    _ if cancelled => w.message = Some((true, "Stopped".into())),
                    Ok(output) => {
                        if let Some((editor, _)) = &w.editor
                            && w.selected().is_some_and(|s| s.path == script.path)
                        {
                            let errors = output.errors.clone();
                            editor.clone().update(cx, |e, cx| e.set_errors(errors, cx));
                        }
                        cx.emit(ScriptsEvent::Finished { script: script.path.clone(), input, output: output.clone() });
                        w.output = Some(output);
                    }
                    Err(e) => w.message = Some((true, e)),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        Ok(())
    }

    /// The scripts that are pipeline nodes, checked against the app's port types (a node naming
    /// another type is left out, with the reason in the second list).
    pub fn nodes(&self) -> (Vec<NodeSpec>, Vec<String>) {
        let (mut ok, mut bad) = (Vec::new(), Vec::new());
        for s in &self.scripts {
            let Some((inputs, outputs)) = &s.meta.node else { continue };
            let unknown: Vec<&str> = inputs.iter().chain(outputs).map(|p| p.ty.as_str()).filter(|t| !self.port_types.iter().any(|k| k == t)).collect();
            if unknown.is_empty() {
                ok.push(NodeSpec { id: format!("script.{}", s.name()), title: s.meta.command.clone().unwrap_or_else(|| s.name()), inputs: inputs.clone(), outputs: outputs.clone(), script: s.path.clone() });
            } else {
                bad.push(format!("{}: unknown port type {} (this app has {})", s.name(), unknown.join(", "), self.port_types.join(", ")));
            }
        }
        (ok, bad)
    }

    /// Registers each command script in `registry` (its arguments the schema its header gives;
    /// running it starts it as a job and answers at once), replacing the ones registered before.
    pub fn register(&mut self, registry: &Registry, me: WeakEntity<Self>) {
        for id in self.registered.drain(..) {
            registry.remove(&id);
        }
        for s in &self.scripts {
            let (Some(id), Some(title)) = (s.command_id(), s.meta.command.clone()) else { continue };
            let (me1, script) = (me.clone(), s.clone());
            let mut c = CommandSpec::raw(
                id.clone(),
                title,
                if s.language == CodeLanguage::Rust { IconName::FileCode } else { IconName::FileText },
                s.meta.args.clone(),
                Rc::new(move |args, _, cx| {
                    let started = id_of(&script);
                    let script = script.clone();
                    me1.update(cx, |w, cx| w.run(script, args, cx)).map_err(|e| e.to_string())??;
                    Ok(json!({ "started": started }))
                }),
            )
            .group("Scripts")
            .hint(s.meta.hint.clone().unwrap_or_else(|| format!("Runs {}", s.path.display())));
            let me2 = me.clone();
            c = c.enabled(move |cx| match me2.upgrade().map(|w| w.read(cx).is_running()) {
                Some(false) => Ok(()),
                Some(true) => Err("A script is running".into()),
                None => Err("The Scripts workspace is closed".into()),
            });
            registry.add(c);
            self.registered.push(id);
        }
    }
}

fn id_of(s: &Script) -> String {
    s.command_id().unwrap_or_else(|| s.name())
}

impl Render for ScriptsWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = v_flex()
            .id("scripts-tree")
            .w(px(240.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .p(px(theme::SM))
            .gap(px(2.))
            .border_r_1()
            .border_color(color::border())
            .child(
                h_flex()
                    .gap(px(theme::XS))
                    .pb(px(theme::SM))
                    .child(Button::new("scripts-new-python").small().outline().icon(IconName::Plus).label("Python").tooltip("A new Python script").on_click(cx.listener(|w, _, window, cx| {
                        if let Err(e) = w.new_script(CodeLanguage::Python, window, cx) {
                            w.message = Some((true, e));
                        }
                    })))
                    .child(Button::new("scripts-new-rust").small().outline().icon(IconName::Plus).label("Rust").tooltip("A new Rust script (runs with rust-script)").on_click(cx.listener(|w, _, window, cx| {
                        if let Err(e) = w.new_script(CodeLanguage::Rust, window, cx) {
                            w.message = Some((true, e));
                        }
                    }))),
            )
            .children(self.scripts.iter().enumerate().map(|(i, s)| {
                let kind = match (&s.meta.command, &s.meta.node) {
                    (Some(_), Some(_)) => "command · node",
                    (Some(_), None) => "command",
                    (None, Some(_)) => "node",
                    (None, None) => "script",
                };
                Button::new(SharedString::from(format!("script-{}", s.name())))
                    .ghost()
                    .small()
                    .w_full()
                    .justify_start()
                    .icon(if s.language == CodeLanguage::Rust { IconName::FileCode } else { IconName::FileText })
                    .child(h_flex().flex_1().min_w_0().gap(px(theme::SM)).child(div().flex_1().min_w_0().truncate().child(s.name())).child(div().text_xs().text_color(color::text_muted()).child(kind)))
                    .selected(self.selected == Some(i))
                    .on_click(cx.listener(move |w, _, window, cx| w.select(i, window, cx)))
            }));
        let running = self.running.is_some();
        let toolbar = self.selected().map(|s| {
            let what = match (&s.meta.command, &s.meta.node) {
                (Some(c), Some((i, o))) => format!("Command \"{c}\" · node {} → {}", ports(i), ports(o)),
                (Some(c), None) => format!("Command \"{c}\""),
                (None, Some((i, o))) => format!("Node {} → {}", ports(i), ports(o)),
                (None, None) => "A plain script (add `kitchen: command = …` to make it a command)".into(),
            };
            h_flex()
                .flex_none()
                .gap(px(theme::SM))
                .px(px(theme::MD))
                .py(px(theme::SM))
                .items_center()
                .border_b_1()
                .border_color(color::border())
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("{}{}", s.name(), if self.dirty { " •" } else { "" })))
                .child(div().flex_1().min_w_0().truncate().text_xs().text_color(color::text_muted()).child(what))
                .child(Button::new("scripts-save").small().outline().icon(IconName::Save).label("Save").disabled(!self.dirty).on_click(cx.listener(|w, _, _, cx| {
                    if let Err(e) = w.save(cx) {
                        w.message = Some((true, e));
                    }
                })))
                .child(Button::new("scripts-run").small().primary().icon(IconName::Play).label(if running { "Running…" } else { "Run" }).disabled(running).on_click(cx.listener(|w, _, _, cx| {
                    if let Err(e) = w.run_selected(Value::Null, cx) {
                        w.message = Some((true, e));
                    }
                })))
        });
        let run_panel = v_flex()
            .id("scripts-output")
            .flex_none()
            .h(px(200.))
            .overflow_y_scroll()
            .p(px(theme::MD))
            .gap(px(theme::XS))
            .border_t_1()
            .border_color(color::border())
            .bg(color::raised())
            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child("RUN"))
            .children(self.message.clone().map(|(err, m)| div().text_sm().text_color(if err { color::danger() } else { color::text_muted() }).child(m)))
            .when(running, |d| d.child(div().text_sm().text_color(color::accent_mark()).child("Running…")))
            .children(self.output.clone().map(|o| {
                v_flex()
                    .gap(px(theme::XS))
                    .child(div().text_sm().text_color(if o.ok { color::success() } else { color::danger() }).child(if o.ok { "Finished" } else { "Failed" }))
                    .children(o.result.map(|r| div().text_sm().font_family("monospace").child(format!("Result: {r}"))))
                    .children(o.stdout.lines().map(|l| div().text_sm().font_family("monospace").child(l.to_string())).collect::<Vec<_>>())
                    .children(o.stderr.lines().map(|l| div().text_sm().font_family("monospace").text_color(color::danger()).child(l.to_string())).collect::<Vec<_>>())
            }));
        let main = match &self.editor {
            Some((editor, _)) => v_flex().flex_1().min_w_0().h_full().children(toolbar).child(div().flex_1().min_h_0().child(editor.clone())).child(run_panel).into_any_element(),
            None => div()
                .flex_1()
                .child(EmptyState::new(IconName::SquareTerminal, "No script", format!("Scripts in {} become commands and pipeline nodes. Start one with Python or Rust.", self.dir.display())))
                .into_any_element(),
        };
        h_flex().size_full().items_start().child(tree).child(main)
    }
}

fn ports(p: &[Port]) -> String {
    if p.is_empty() { "nothing".into() } else { p.iter().map(|p| format!("{}:{}", p.name, p.ty)).collect::<Vec<_>>().join(", ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_say_what_a_script_is() {
        let m = ScriptMeta::parse(
            "#!/usr/bin/env python3\n# kitchen: command = Count cells\n# kitchen: hint = Counts them\n# kitchen: args = {\"type\": \"object\"}\n# kitchen: node = in image:image, mask:mask; out counts:table\nimport json\n# kitchen: command = ignored after code\n",
        );
        assert_eq!(m.command.as_deref(), Some("Count cells"));
        assert_eq!(m.hint.as_deref(), Some("Counts them"));
        assert_eq!(m.args, Some(json!({"type": "object"})));
        let (i, o) = m.node.unwrap();
        assert_eq!(i, vec![Port { name: "image".into(), ty: "image".into() }, Port { name: "mask".into(), ty: "mask".into() }]);
        assert_eq!(o, vec![Port { name: "counts".into(), ty: "table".into() }]);
        assert!(m.problems.is_empty());

        let bad = ScriptMeta::parse("// kitchen: args = [1]\n// kitchen: node = in a:image\n// kitchen: colour = red\n// kitchen: oops\nfn main() {}\n");
        let lines: Vec<usize> = bad.problems.iter().map(|p| p.0).collect();
        assert_eq!(lines, vec![1, 2, 3, 4]);
    }

    #[test]
    fn errors_point_at_the_scripts_lines() {
        let py = Script { path: "/x/count.py".into(), language: CodeLanguage::Python, meta: ScriptMeta::default() };
        let tb = "Traceback (most recent call last):\n  File \"/x/count.py\", line 7, in <module>\n    main()\n  File \"/x/count.py\", line 4, in main\n    1/0\nZeroDivisionError: division by zero\n";
        assert_eq!(script_errors(&py, tb), vec![(4, "ZeroDivisionError: division by zero".to_string())]);
        let rs = Script { path: "/x/count.rs".into(), language: CodeLanguage::Rust, meta: ScriptMeta::default() };
        let msg = "error[E0308]: mismatched types\n --> /x/count.rs:6:13\n";
        assert_eq!(script_errors(&rs, msg), vec![(6, "error[E0308]: mismatched types".to_string())]);
    }
}
