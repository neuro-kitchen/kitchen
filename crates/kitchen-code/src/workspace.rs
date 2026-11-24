//! The Code workspace: one space to write the app's custom commands and pipeline steps, in
//! Python or Rust. Scripts tree on the left, the editor (kitchen-ui's `CodeView`: highlighting,
//! errors on their lines) in the middle, the run panel (output, the result, errors) under it.
//!
//! A script runs against what is in front of the user ([`crate::run`]); its header says what it is
//! ([`crate::header`]). With `command`, the script becomes a command ([`CodeWorkspace::register`]);
//! with `node`, a pipeline step ([`CodeWorkspace::nodes`]) whose port types are the app's
//! ([`CodeWorkspace::new`] takes them). Running a script is a job with a cancel button; what it
//! ran is reported ([`CodeEvent::Finished`]) for the app to record in its session.

use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px,
};
use kitchen_shell::command::{CommandSpec, Registry};
use kitchen_shell::jobs;
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{CodeChanged, CodeLanguage, CodeView, EmptyState};
use serde_json::{Value, json};

use crate::changes::{ChangeBase, ChangeTracking, summary_text};
use crate::header::Port;
use crate::run::{RunOutput, Runner, default_runner};
use crate::script::Script;

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
pub enum CodeEvent {
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

/// The Code workspace.
pub struct CodeWorkspace {
    dir: PathBuf,
    port_types: Vec<String>,
    runner: Runner,
    context: ContextFn,
    scripts: Vec<Script>,
    selected: Option<usize>,
    editor: Option<(Entity<CodeView>, Subscription)>,
    /// The editor's text differs from the file.
    dirty: bool,
    /// Change marks shown in the gutter (the toggle), against what.
    show_changes: bool,
    change_base: ChangeBase,
    /// The open script's baselines.
    tracking: Option<ChangeTracking>,
    /// `+3 ~1 −2` for the toolbar, while the marks show.
    changes_text: Option<String>,
    running: Option<u64>,
    output: Option<RunOutput>,
    message: Option<(bool, String)>,
    registered: Vec<String>,
}

impl EventEmitter<CodeEvent> for CodeWorkspace {}

impl CodeWorkspace {
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
            show_changes: false,
            change_base: ChangeBase::default(),
            tracking: None,
            changes_text: None,
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
        cx.emit(CodeEvent::ScriptsChanged);
        cx.notify();
    }

    /// Opens script `i` in the editor.
    pub fn select(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.scripts.get(i).cloned() else { return };
        let text = std::fs::read_to_string(&s.path).unwrap_or_default();
        let language = s.language;
        let problems = s.meta.problems.clone();
        let editor = cx.new(|cx| CodeView::new(language, text, window, cx).editable(true).errors(problems));
        let sub = cx.subscribe(&editor, |this, _, _: &CodeChanged, cx| {
            this.dirty = true;
            this.refresh_changes(cx);
            cx.notify();
        });
        self.tracking = Some(ChangeTracking::open(&s.path, &text, self.change_base));
        self.editor = Some((editor, sub));
        self.selected = Some(i);
        self.dirty = false;
        self.output = None;
        self.message = None;
        self.refresh_changes(cx);
        cx.notify();
    }

    /// Whether the change marks show.
    pub fn changes_shown(&self) -> bool {
        self.show_changes
    }

    /// What the change marks compare against.
    pub fn change_base(&self) -> ChangeBase {
        self.change_base
    }

    /// The open script's baselines.
    pub fn change_tracking(&self) -> Option<&ChangeTracking> {
        self.tracking.as_ref()
    }

    /// Shows or hides the change marks (the toolbar button and the `[code] toggle_changes` key).
    pub fn toggle_changes(&mut self, cx: &mut Context<Self>) {
        self.show_changes = !self.show_changes;
        self.refresh_changes(cx);
        cx.notify();
    }

    /// Compares the marks against `base` (shown at once); an error when it is the last commit and
    /// the script is not in a repository with one.
    pub fn set_change_base(&mut self, base: ChangeBase, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(t) = &mut self.tracking
            && !t.set_base(base)
        {
            return Err("This script is not in a git repository with a commit".into());
        }
        self.change_base = base;
        self.show_changes = true;
        self.refresh_changes(cx);
        cx.notify();
        Ok(())
    }

    /// Hands the marks to the editor (none while hidden).
    fn refresh_changes(&mut self, cx: &mut Context<Self>) {
        let Some((editor, _)) = &self.editor else { return };
        let editor = editor.clone();
        let marks = match (&self.tracking, self.show_changes) {
            (Some(t), true) => t.marks(&editor.read(cx).text(cx)),
            _ => Vec::new(),
        };
        self.changes_text = self.show_changes.then(|| summary_text(&marks));
        editor.update(cx, |e, cx| e.set_changes(marks, cx));
    }

    /// Writes the editor's text to the selected script, and reads its header again.
    pub fn save(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let (Some(i), Some(editor)) = (self.selected, self.editor()) else { return Err("no script open".into()) };
        let text = editor.read(cx).text(cx);
        let path = self.scripts[i].path.clone();
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.dirty = false;
        if let Some(t) = &mut self.tracking {
            t.saved(&path, &text);
        }
        self.refresh_changes(cx);
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
                        cx.emit(CodeEvent::Finished { script: script.path.clone(), input, output: output.clone() });
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
                None => Err("The Code workspace is closed".into()),
            });
            registry.add(c);
            self.registered.push(id);
        }
    }
}

fn id_of(s: &Script) -> String {
    s.command_id().unwrap_or_else(|| s.name())
}

impl Render for CodeWorkspace {
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
                .child(self.changes_controls(cx))
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

impl CodeWorkspace {
    /// The change marks' toggle, and while on, what they compare against with the count.
    fn changes_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.show_changes;
        let has_commit = self.tracking.as_ref().is_some_and(|t| t.has_commit());
        h_flex()
            .gap(px(theme::XS))
            .items_center()
            .child(
                Button::new("code-changes")
                    .small()
                    .ghost()
                    .icon(IconName::GitCompare)
                    .label(self.changes_text.clone().unwrap_or_else(|| "Changes".into()))
                    .selected(on)
                    .tooltip("Mark changed lines, git style")
                    .on_click(cx.listener(|w, _, _, cx| w.toggle_changes(cx))),
            )
            .when(on, |row| {
                row.children(ChangeBase::ALL.into_iter().map(|base| {
                    let unavailable = base == ChangeBase::Commit && !has_commit;
                    Button::new(SharedString::from(format!("code-changes-{base:?}")))
                        .xsmall()
                        .ghost()
                        .label(base.title())
                        .selected(self.change_base == base)
                        .disabled(unavailable)
                        .tooltip(if unavailable { "Not in a git repository with a commit" } else { base.title() })
                        .on_click(cx.listener(move |w, _, _, cx| {
                            if let Err(e) = w.set_change_base(base, cx) {
                                w.message = Some((true, e));
                            }
                        }))
                }))
            })
    }
}

fn ports(p: &[Port]) -> String {
    if p.is_empty() { "nothing".into() } else { p.iter().map(|p| format!("{}:{}", p.name, p.ty)).collect::<Vec<_>>().join(", ") }
}
