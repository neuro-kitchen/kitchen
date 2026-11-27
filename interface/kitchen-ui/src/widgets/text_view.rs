//! The text view: wrapped, selectable text with light markdown, for agent messages and notes.

use gpui_kit::component::text::{TextView as KitTextView, TextViewState};
use gpui_kit::{AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div};

/// Wrapped, selectable text with light markdown: headings, emphasis, `code` spans, bullet and
/// numbered lists, links, and fenced code highlighted with the same grammars and colours as
/// [`crate::widgets::CodeView`] (` ```rust `, ` ```python `, ` ```json `). As tall as its text.
///
/// Text can stream in ([`TextView::push_str`]), as an agent's reply does. Built on gpui-kit's
/// markdown view; selection and copy (Ctrl/Cmd+C) come with it.
pub struct TextView {
    text: String,
    state: Entity<TextViewState>,
}

impl TextView {
    /// A view of `markdown`.
    pub fn new(markdown: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let text = markdown.into();
        let state = cx.new(|cx| TextViewState::markdown(&text, cx).selectable(true));
        Self { text, state }
    }

    /// Appends `more` (a streamed chunk).
    pub fn push_str(&mut self, more: &str, cx: &mut Context<Self>) {
        self.text.push_str(more);
        self.state.update(cx, |s, cx| s.push_str(more, cx));
        cx.notify();
    }

    /// Replaces the text.
    pub fn set_text(&mut self, markdown: impl Into<String>, cx: &mut Context<Self>) {
        self.text = markdown.into();
        let text = self.text.clone();
        self.state.update(cx, |s, cx| s.set_text(&text, cx));
        cx.notify();
    }

    /// The markdown source.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text selected, as shown (no markdown marks).
    pub fn selected_text(&self, cx: &gpui_kit::App) -> String {
        self.state.read(cx).selected_text()
    }
}

impl Render for TextView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().min_w_0().text_sm().child(KitTextView::new(&self.state).selectable(true).w_full())
    }
}
