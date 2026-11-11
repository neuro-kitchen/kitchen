//! The agent workspace: sessions on the left, the conversation in the middle (kitchen-ui's
//! `ChatPane`: messages, tool calls, the prompt and its context chips), and on the right the
//! Changes panel (what the session changed) or the project's files. Every command an agent runs
//! (through MCP or from this chat) shows in the conversation as a tool call, and a call waiting
//! for the person shows above the prompt with Allow and Deny. Sessions are saved as JSON files
//! (the conversation and the calls), so a session can be read, shared and continued.
//!
//! The agent itself is the app's (no API key here): the workspace tells the app what the person
//! asked ([`AgentEvent::Prompt`]); the app answers through [`AgentWorkspace::chat`] and runs
//! commands through the host ([`McpHost::call`]).

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyView, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use kitchen_shell::{CommandSpec, SettingsSection, Shell};
use kitchen_ui::chat::{ChatContext, ChatEvent, ChatMessage, ChatPane, ToolCall};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{MenuSelect, row};
use serde::{Deserialize, Serialize};

use crate::host::{CallRecord, HostEvent, McpHost, Policy};

/// A saved session: its title, the conversation and the calls it made.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Its title (the first prompt, shortened).
    pub title: String,
    /// The conversation.
    pub messages: Vec<ChatMessage>,
    /// Every command run, in order.
    pub calls: Vec<CallRecord>,
}

/// What the workspace tells the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// The person asked this (the app's agent answers).
    Prompt(String),
    /// The person asked the agent to stop.
    Stop,
    /// Another session was opened or started (the app's agent starts afresh).
    SessionChanged,
}

/// Which panel shows at the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// What the session changed.
    Changes,
    /// The project's files.
    Files,
}

/// The agent workspace.
pub struct AgentWorkspace {
    dir: PathBuf,
    files_root: PathBuf,
    sessions: Vec<(PathBuf, String)>,
    current: PathBuf,
    session: Session,
    chat: Entity<ChatPane>,
    host: Entity<McpHost>,
    changes: AnyView,
    side: Side,
    _subs: Vec<Subscription>,
}

impl EventEmitter<AgentEvent> for AgentWorkspace {}

impl AgentWorkspace {
    /// The workspace saving sessions in `dir`, over `host`'s commands, showing `changes` (the
    /// app's Changes panel) and the files under `files_root`; `context` fills the chips.
    pub fn new(dir: impl Into<PathBuf>, files_root: impl Into<PathBuf>, host: Entity<McpHost>, changes: impl Into<AnyView>, context: ChatContext, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        let chat = cx.new(|cx| ChatPane::new(window, cx));
        chat.update(cx, |c, cx| c.set_context(context, cx));
        let subs = vec![
            cx.subscribe(&chat, |this, _, e: &ChatEvent, cx| match e {
                ChatEvent::Send(text) => {
                    if this.session.title.is_empty() {
                        this.session.title = text.chars().take(60).collect();
                    }
                    this.save(cx);
                    cx.emit(AgentEvent::Prompt(text.clone()));
                }
                ChatEvent::Stop => cx.emit(AgentEvent::Stop),
            }),
            cx.subscribe(&host, |this, _, e: &HostEvent, cx| {
                if let HostEvent::Called(rec) = e {
                    this.show_call(rec, cx);
                }
                cx.notify();
            }),
            cx.observe(&chat, |this, chat, cx| {
                this.session.messages = chat.read(cx).messages().to_vec();
            }),
        ];
        let current = new_path(&dir);
        let mut ws = Self { dir, files_root: files_root.into(), sessions: Vec::new(), current, session: Session::default(), chat, host, changes: changes.into(), side: Side::Changes, _subs: subs };
        ws.rescan();
        ws
    }

    /// The conversation pane (the app's agent writes its answers here).
    pub fn chat(&self) -> &Entity<ChatPane> {
        &self.chat
    }

    /// The host.
    pub fn host(&self) -> &Entity<McpHost> {
        &self.host
    }

    /// The session open.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The saved sessions: file and title, newest first.
    pub fn sessions(&self) -> &[(PathBuf, String)] {
        &self.sessions
    }

    /// Shows the Changes panel or the files at the right.
    pub fn set_side(&mut self, side: Side, cx: &mut Context<Self>) {
        self.side = side;
        cx.notify();
    }

    fn show_call(&mut self, rec: &CallRecord, cx: &mut Context<Self>) {
        let call = ToolCall::new(rec.name.clone(), rec.arguments.clone());
        let call = match &rec.result {
            Ok(v) => call.finished(v.clone(), summary(v)),
            Err(e) => ToolCall { error: Some(e.clone()), ..call },
        };
        self.chat.update(cx, |c, cx| {
            c.push(ChatMessage::Tool(call), cx);
        });
        self.session.calls.push(rec.clone());
        self.session.messages = self.chat.read(cx).messages().to_vec();
        self.save(cx);
    }

    /// Writes the session to its file.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.session.messages.is_empty() && self.session.calls.is_empty() {
            return;
        }
        self.session.messages = self.chat.read(cx).messages().to_vec();
        if let Ok(text) = serde_json::to_string_pretty(&self.session) {
            let _ = std::fs::write(&self.current, text);
        }
        self.rescan();
        cx.notify();
    }

    /// Starts a new, empty session.
    pub fn new_session(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        self.current = new_path(&self.dir);
        self.session = Session::default();
        self.chat.update(cx, |c, cx| c.set_messages(Vec::new(), cx));
        cx.emit(AgentEvent::SessionChanged);
        cx.notify();
    }

    /// Opens the saved session at `path`.
    pub fn open_session(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let session: Session = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.save(cx);
        self.current = path.to_path_buf();
        self.chat.update(cx, |c, cx| c.set_messages(session.messages.clone(), cx));
        self.session = session;
        cx.emit(AgentEvent::SessionChanged);
        cx.notify();
        Ok(())
    }

    fn rescan(&mut self) {
        let mut found: Vec<(PathBuf, String, std::time::SystemTime)> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| {
                let s: Session = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
                let t = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?;
                Some((p, if s.title.is_empty() { "Untitled".to_string() } else { s.title }, t))
            })
            .collect();
        found.sort_by(|a, b| b.2.cmp(&a.2));
        self.sessions = found.into_iter().map(|(p, t, _)| (p, t)).collect();
    }

    fn files(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(&self.files_root).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        out.sort();
        out
    }
}

fn new_path(dir: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    dir.join(format!("session-{stamp}.json"))
}

/// A one-line summary of a result: its first few keys.
fn summary(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(m) => m.iter().take(3).map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", "),
        other => other.to_string().chars().take(80).collect(),
    }
}

impl Render for AgentWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions = v_flex()
            .id("agent-sessions")
            .w(px(220.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .p(px(theme::SM))
            .gap(px(2.))
            .border_r_1()
            .border_color(color::border())
            .child(Button::new("agent-new-session").small().outline().w_full().icon(IconName::Plus).label("New session").on_click(cx.listener(|w, _, _, cx| w.new_session(cx))))
            .child(div().pt(px(theme::SM)).pb(px(theme::XS)).text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child("SESSIONS"))
            .children(self.sessions.clone().into_iter().enumerate().map(|(i, (path, title))| {
                let current = path == self.current;
                Button::new(SharedString::from(format!("agent-session-{i}")))
                    .ghost()
                    .small()
                    .w_full()
                    .justify_start()
                    .child(div().flex_1().min_w_0().truncate().child(title))
                    .selected(current)
                    .on_click(cx.listener(move |w, _, _, cx| {
                        let _ = w.open_session(&path, cx);
                    }))
            }));
        let pending: Vec<_> = self
            .host
            .read(cx)
            .pending()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let host = self.host.clone();
                let host2 = self.host.clone();
                h_flex()
                    .gap(px(theme::SM))
                    .items_center()
                    .p(px(theme::SM))
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_color(color::warning())
                    .child(v_flex().flex_1().min_w_0().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("The agent wants to run {}", p.title))).child(div().text_xs().font_family("monospace").text_color(color::text_muted()).truncate().child(format!("{} {}", p.name, p.arguments))))
                    .child(Button::new(SharedString::from(format!("agent-allow-{i}"))).small().primary().label("Allow").on_click(move |_, _, cx| host.update(cx, |h, cx| h.decide(i, true, cx))))
                    .child(Button::new(SharedString::from(format!("agent-deny-{i}"))).small().outline().label("Deny").on_click(move |_, _, cx| host2.update(cx, |h, cx| h.decide(i, false, cx))))
            })
            .collect();
        let center = v_flex().flex_1().min_w_0().h_full().child(v_flex().flex_none().gap(px(theme::XS)).p(px(if pending.is_empty() { 0. } else { theme::SM })).children(pending)).child(div().flex_1().min_h_0().child(self.chat.clone()));
        let tab = |side: Side, label: &'static str, cx: &mut Context<Self>| Button::new(label).ghost().small().label(label).selected(self.side == side).on_click(cx.listener(move |w, _, _, cx| w.set_side(side, cx)));
        let side_body = match self.side {
            Side::Changes => div().flex_1().min_h_0().child(self.changes.clone()).into_any_element(),
            Side::Files => v_flex()
                .id("agent-files")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(theme::MD))
                .gap(px(theme::XS))
                .children(self.files().into_iter().map(|f| h_flex().gap(px(theme::SM)).child(gpui_kit::component::Icon::new(IconName::File).small().text_color(color::text_muted())).child(div().text_sm().truncate().child(f))))
                .into_any_element(),
        };
        let side = v_flex()
            .w(px(380.))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(color::border())
            .child(h_flex().flex_none().gap(px(2.)).p(px(theme::XS)).border_b_1().border_color(color::border()).child(tab(Side::Changes, "Changes", cx)).child(tab(Side::Files, "Files", cx)))
            .child(side_body);
        h_flex().size_full().items_start().child(sessions).child(center).child(side)
    }
}

/// "Open in Agents": a command switching `shell` to the agent workspace `workspace_id`.
pub fn open_command(shell: &Entity<Shell>, workspace_id: &str) -> CommandSpec {
    let (shell, id) = (shell.downgrade(), workspace_id.to_string());
    CommandSpec::new("agent.open", "Open in Agents", IconName::Bot, move |_, cx| {
        let _ = shell.update(cx, |s, cx| s.set_workspace(&id, cx));
    })
    .group("Agents")
    .hint("The agent workspace: sessions, the conversation, what it changed")
    .key("secondary-shift-a")
    .read_only()
}

/// The Agents section of Settings: what an agent may run without asking, and where it connects.
pub fn settings_section(host: Entity<McpHost>, socket: Option<PathBuf>) -> SettingsSection {
    SettingsSection::new("Agents", IconName::Bot, move |_, cx| {
        let policy = host.read(cx).policy();
        let all = [Policy::Ask, Policy::ReadOnly, Policy::AllowAll];
        let h = host.clone();
        let select = MenuSelect::new("agent-policy", all.iter().map(|p| SharedString::from(p.title())).collect(), all.iter().position(|p| *p == policy), move |i, _, cx| {
            h.update(cx, |h, cx| h.set_policy(all[i], cx));
        });
        let tools = host.read(cx).tools();
        let reads = tools.iter().filter(|t| t.read_only).count();
        let connect = match &socket {
            Some(p) => format!("MCP clients connect with: kitchen-mcp-bridge {}", p.display()),
            None => "The MCP socket is off.".to_string(),
        };
        v_flex()
            .gap(px(theme::MD))
            .child(row("Agents may", div().w_full().child(select)))
            .child(div().text_xs().text_color(color::text_muted()).child(format!("{} commands are tools; {reads} only read and run without asking.", tools.len())))
            .child(div().text_xs().font_family("monospace").text_color(color::text_muted()).child(connect))
            .into_any_element()
    })
}
