//! The agent chat as a view an app can dock: the conversation (the user's prompts, the agent's
//! replies, its tool calls) and a prompt box. No network or model code: the pane emits
//! [`ChatEvent`]s and the app pushes messages in. The conversation is plain data
//! ([`ChatMessage`], serializable) so an app can record it (a session file) and restore it.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window, div, px,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::theme::{self, color};
use crate::widgets::{CodeLanguage, CodeView, EmptyState, TextView};

/// One message of the conversation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessage {
    /// What the user asked.
    User {
        /// Plain text (shown as markdown).
        text: String,
    },
    /// What the agent answered (markdown).
    Agent {
        /// Markdown.
        text: String,
    },
    /// A command the agent ran.
    Tool(ToolCall),
}

/// A command the agent ran: its name, arguments, result and a one-line summary.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The command id (`hist.find_cells`).
    pub name: String,
    /// What it did, shortly (`2,539 cells`); shown after the name.
    #[serde(default)]
    pub summary: String,
    /// Its arguments.
    #[serde(default)]
    pub arguments: Value,
    /// Its result, once it finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Why it failed, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolCall {
    /// A call of `name` with `arguments`, still running.
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        ToolCall { name: name.into(), arguments, ..Default::default() }
    }

    /// The same call, finished with `result` and `summary`.
    pub fn finished(self, result: Value, summary: impl Into<String>) -> Self {
        ToolCall { result: Some(result), summary: summary.into(), ..self }
    }

    /// Whether it is still running (no result, no error).
    pub fn is_running(&self) -> bool {
        self.result.is_none() && self.error.is_none()
    }

    /// The collapsed line: "Ran hist.find_cells · 2,539 cells", "Running …", "Failed …".
    pub fn line(&self) -> String {
        let tail = |s: &str| if s.is_empty() { String::new() } else { format!(" · {s}") };
        match (&self.result, &self.error) {
            (_, Some(e)) => format!("Failed {}{}", self.name, tail(e)),
            (Some(_), None) => format!("Ran {}{}", self.name, tail(&self.summary)),
            (None, None) => format!("Running {}…", self.name),
        }
    }
}

/// Where the agent works.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    /// This computer.
    #[default]
    Local,
    /// A remote machine (its name).
    Remote(String),
}

/// What the chips above the prompt show: the workspace, where the agent runs, the model.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatContext {
    /// The workspace or project (empty: no chip).
    pub workspace: String,
    /// This computer or a remote one.
    pub location: Location,
    /// The model (empty: no chip).
    pub model: String,
}

/// What the pane asks of the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    /// The user sent this prompt (already added to the conversation as a user message, and the
    /// pane shows the agent as running until [`ChatPane::set_running`]`(false)`).
    Send(String),
    /// The user asked the running agent to stop.
    Stop,
}

/// What a message shows with: its text view, or a tool call's open state and code views.
enum RowView {
    Text(Entity<TextView>),
    Tool { open: bool, views: Option<(Entity<CodeView>, Option<Entity<CodeView>>)> },
}

/// The agent chat: a scrolling conversation (user, agent and tool-call rows) above a prompt box
/// with context chips. A tool call is one line ("Ran hist.find_cells · 2,539 cells") that opens
/// to its JSON arguments and result. Agent text streams in with [`ChatPane::append_agent`].
///
/// The prompt: Enter sends, Shift+Enter is a newline; the Send button becomes Stop while the agent
/// runs. The pane emits [`ChatEvent`]; the app pushes the agent's messages in
/// ([`ChatPane::push`], [`ChatPane::append_agent`], [`ChatPane::set_tool_result`]) and says when
/// it is done ([`ChatPane::set_running`]). [`ChatPane::messages`] is the whole conversation.
pub struct ChatPane {
    messages: Vec<ChatMessage>,
    rows: Vec<RowView>,
    context: ChatContext,
    running: bool,
    prompt: Entity<TextareaState>,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ChatEvent> for ChatPane {}

impl ChatPane {
    /// An empty conversation.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prompt = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 8).submit_on_enter(true).placeholder("Ask the agent…"));
        let sub = cx.subscribe_in(&prompt, window, |this, _, e: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = e {
                this.send(window, cx);
            }
        });
        Self { messages: Vec::new(), rows: Vec::new(), context: ChatContext::default(), running: false, prompt, scroll: ScrollHandle::new(), _subs: vec![sub] }
    }

    /// The conversation.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Replaces the conversation (a restored session).
    pub fn set_messages(&mut self, messages: Vec<ChatMessage>, cx: &mut Context<Self>) {
        self.messages.clear();
        self.rows.clear();
        for m in messages {
            self.push(m, cx);
        }
    }

    /// Adds a message; returns its index.
    pub fn push(&mut self, message: ChatMessage, cx: &mut Context<Self>) -> usize {
        let row = match &message {
            ChatMessage::User { text } | ChatMessage::Agent { text } => {
                let text = text.clone();
                RowView::Text(cx.new(|cx| TextView::new(text, cx)))
            }
            ChatMessage::Tool(_) => RowView::Tool { open: false, views: None },
        };
        self.messages.push(message);
        self.rows.push(row);
        self.scroll.scroll_to_bottom();
        cx.notify();
        self.messages.len() - 1
    }

    /// Appends streamed agent text: to the last message when it is the agent's, else as a new
    /// agent message.
    pub fn append_agent(&mut self, text: &str, cx: &mut Context<Self>) {
        let last = self.messages.len().checked_sub(1);
        if let Some(i) = last
            && let (ChatMessage::Agent { text: t }, RowView::Text(view)) = (&mut self.messages[i], &self.rows[i])
        {
            t.push_str(text);
            view.update(cx, |v, cx| v.push_str(text, cx));
            self.scroll.scroll_to_bottom();
            cx.notify();
            return;
        }
        self.push(ChatMessage::Agent { text: text.to_string() }, cx);
    }

    /// Finishes the tool call at `index`: its result and summary, or why it failed.
    pub fn set_tool_result(&mut self, index: usize, result: Result<(Value, String), String>, cx: &mut Context<Self>) {
        if let (Some(ChatMessage::Tool(call)), Some(RowView::Tool { views, .. })) = (self.messages.get_mut(index), self.rows.get_mut(index)) {
            match result {
                Ok((value, summary)) => {
                    call.result = Some(value);
                    call.summary = summary;
                    call.error = None;
                }
                Err(e) => call.error = Some(e),
            }
            // Built again with the result on the next draw
            *views = None;
            cx.notify();
        }
    }

    /// Whether the agent is running (the Send button shows Stop).
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Says whether the agent is running.
    pub fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        self.running = running;
        cx.notify();
    }

    /// The chips above the prompt.
    pub fn set_context(&mut self, context: ChatContext, cx: &mut Context<Self>) {
        self.context = context;
        cx.notify();
    }

    /// The prompt's text.
    pub fn prompt_text(&self, cx: &App) -> String {
        self.prompt.read(cx).value().to_string()
    }

    /// Puts `text` in the prompt.
    pub fn set_prompt(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_string();
        self.prompt.update(cx, |p, cx| p.set_value(text, window, cx));
    }

    /// Focuses the prompt.
    pub fn focus_prompt(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt.update(cx, |p, cx| p.focus(window, cx));
    }

    /// Sends the prompt (as Enter and the Send button do): adds it as a user message, clears the
    /// box, shows the agent as running and emits [`ChatEvent::Send`]. Nothing while the agent
    /// runs or when the prompt is blank.
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.prompt_text(cx);
        let text = text.trim();
        if self.running || text.is_empty() {
            return;
        }
        let text = text.to_string();
        self.prompt.update(cx, |p, cx| p.set_value("", window, cx));
        self.push(ChatMessage::User { text: text.clone() }, cx);
        self.running = true;
        cx.emit(ChatEvent::Send(text));
        cx.notify();
    }

    /// Asks the running agent to stop (emits [`ChatEvent::Stop`]; the app says when it stopped).
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.running {
            cx.emit(ChatEvent::Stop);
        }
    }

    /// Opens or closes the tool call at `index`.
    pub fn toggle_tool(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(RowView::Tool { open, .. }) = self.rows.get_mut(index) {
            *open = !*open;
            cx.notify();
        }
    }

    fn message_row(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let caption = |icon: IconName, who: &'static str| {
            h_flex()
                .gap(px(theme::XS))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color::text_muted())
                .child(Icon::new(icon).size(px(12.)))
                .child(who)
        };
        match (&self.messages[i], &mut self.rows[i]) {
            (ChatMessage::User { .. }, RowView::Text(view)) => v_flex()
                .w_full()
                .gap(px(theme::XS))
                .child(caption(IconName::User, "You"))
                .child(div().w_full().px(px(theme::MD)).py(px(theme::SM)).rounded(px(theme::RADIUS_LG)).bg(color::raised()).child(view.clone()))
                .into_any_element(),
            (ChatMessage::Agent { .. }, RowView::Text(view)) => {
                v_flex().w_full().gap(px(theme::XS)).child(caption(IconName::Bot, "Agent")).child(view.clone()).into_any_element()
            }
            (ChatMessage::Tool(call), RowView::Tool { open, views }) => {
                let (icon, icon_color) = match (&call.result, &call.error) {
                    (_, Some(_)) => (IconName::CircleX, color::danger()),
                    (Some(_), None) => (IconName::SquareTerminal, color::text_muted()),
                    (None, None) => (IconName::LoaderCircle, color::accent_mark()),
                };
                let line = h_flex()
                    .id(SharedString::from(format!("chat-tool-{i}")))
                    .w_full()
                    .min_h(px(theme::TARGET_MIN))
                    .px(px(theme::XS))
                    .gap(px(theme::SM))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|d| d.bg(color::hover()))
                    .child(Icon::new(if *open { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.)).text_color(color::text_muted()))
                    .child(Icon::new(icon).size(px(14.)).text_color(icon_color))
                    .child(div().flex_1().min_w_0().truncate().text_sm().text_color(color::text_muted()).child(call.line()))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_tool(i, cx)))
                    .test_support();
                if !*open {
                    return line.into_any_element();
                }
                if views.is_none() {
                    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default();
                    let args = pretty(&call.arguments);
                    let result = call.result.as_ref().map(pretty);
                    let a = cx.new(|cx| CodeView::new(CodeLanguage::Json, args, window, cx).max_lines(12));
                    let r = result.map(|r| cx.new(|cx| CodeView::new(CodeLanguage::Json, r, window, cx).max_lines(12)));
                    *views = Some((a, r));
                }
                let Some((a, r)) = views.clone() else { return line.into_any_element() };
                let label = |s: &'static str| div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child(s.to_uppercase());
                let error = call.error.clone();
                v_flex()
                    .w_full()
                    .gap(px(theme::XS))
                    .child(line)
                    .child(
                        v_flex()
                            .w_full()
                            .pl(px(theme::XL))
                            .gap(px(theme::XS))
                            .child(label("Arguments"))
                            .child(a)
                            .when_some(r, |d, r| d.child(label("Result")).child(r))
                            .when_some(error, |d, e| d.child(label("Error")).child(div().text_sm().text_color(color::danger()).child(e))),
                    )
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn chips(&self) -> Option<impl IntoElement> {
        let c = &self.context;
        let chip = |id: &'static str, icon: IconName, text: String, tip: &'static str| {
            h_flex()
                .id(id)
                .flex_none()
                .min_h(px(theme::TARGET_MIN))
                .px(px(theme::SM))
                .gap(px(theme::XS))
                .rounded(px(theme::RADIUS_SM))
                .bg(color::control())
                .text_xs()
                .text_color(color::text())
                .child(Icon::new(icon).size(px(12.)).text_color(color::text_muted()))
                .child(div().max_w(px(180.)).truncate().child(text))
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                .into_any_element()
        };
        let mut out: Vec<AnyElement> = Vec::new();
        if !c.workspace.is_empty() {
            out.push(chip("chat-chip-workspace", IconName::Folder, c.workspace.clone(), "Workspace"));
        }
        out.push(match &c.location {
            Location::Local => chip("chat-chip-location", IconName::HardDrive, "This computer".into(), "The agent runs on this computer"),
            Location::Remote(host) => chip("chat-chip-location", IconName::Globe, host.clone(), "The agent runs on a remote machine"),
        });
        if !c.model.is_empty() {
            out.push(chip("chat-chip-model", IconName::Cpu, c.model.clone(), "Model"));
        }
        Some(h_flex().w_full().flex_wrap().gap(px(theme::XS)).children(out))
    }
}

impl Render for ChatPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list: Vec<AnyElement> = (0..self.messages.len()).map(|i| self.message_row(i, window, cx)).collect();
        let body = if list.is_empty() {
            div()
                .flex_1()
                .min_h_0()
                .child(EmptyState::new(IconName::Bot, "Ask the agent", "Say what you want done with this workspace. Each command the agent runs shows here, with its arguments and result."))
                .into_any_element()
        } else {
            v_flex()
                .id("chat-messages")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .p(px(theme::MD))
                .gap(px(theme::LG))
                .children(list)
                .into_any_element()
        };
        let action = if self.running {
            Button::new("chat-stop").small().icon(IconName::Square).label("Stop").tooltip("Stop the agent").on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("chat-send").small().primary().icon(IconName::ArrowUp).label("Send").tooltip("Send (Enter; Shift+Enter for a new line)").on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };
        let footer = v_flex()
            .flex_none()
            .w_full()
            .p(px(theme::SM))
            .gap(px(theme::SM))
            .border_t_1()
            .border_color(color::border())
            .children(self.chips())
            .child(
                v_flex()
                    .w_full()
                    .gap(px(theme::XS))
                    .child(Textarea::new(&self.prompt).w_full())
                    .child(
                        h_flex()
                            .w_full()
                            .gap(px(theme::SM))
                            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(color::text_muted()).child(if self.running { "The agent is working…" } else { "Enter sends · Shift+Enter for a new line" }))
                            .child(action),
                    ),
            );
        v_flex().id("chat-pane").size_full().bg(color::panel()).rounded(px(theme::RADIUS_LG)).overflow_hidden().child(body).child(footer)
    }
}
