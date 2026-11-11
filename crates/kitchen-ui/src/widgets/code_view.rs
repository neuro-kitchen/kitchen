//! The code view: gpui-kit's code editor (tree-sitter highlighting, line numbers, current line,
//! selection and copy), read-only or editable, with errors marked on their lines.

use gpui_kit::component::highlighter::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::{Editor, EditorState, GutterDecoration, GutterDecorationCollection, InputEvent, Position};
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window, div, px,
};

use crate::theme::{self, color};

/// The languages a [`CodeView`] highlights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeLanguage {
    /// Rust.
    Rust,
    /// Python.
    Python,
    /// JSON.
    Json,
    /// No highlighting.
    #[default]
    Plain,
}

impl CodeLanguage {
    /// The grammar name gpui-kit knows it by.
    pub fn name(self) -> &'static str {
        match self {
            CodeLanguage::Rust => "rust",
            CodeLanguage::Python => "python",
            CodeLanguage::Json => "json",
            CodeLanguage::Plain => "text",
        }
    }

    /// The language of a name or file extension (`"rs"`, `"py"`, `"json"`, …); anything else is
    /// [`CodeLanguage::Plain`].
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "rust" | "rs" => CodeLanguage::Rust,
            "python" | "py" | "pyi" => CodeLanguage::Python,
            "json" | "jsonc" => CodeLanguage::Json,
            _ => CodeLanguage::Plain,
        }
    }
}

/// What an editable [`CodeView`] reports when the user changes the text: the whole text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeChanged(pub String);

/// Code in a monospace editor: line numbers in a gutter, syntax highlighting (Rust, Python, JSON),
/// the current line highlighted, selection and copy (read-only too). Errors ([`CodeView::errors`],
/// 1-based line numbers as compilers report them) get a red mark in the gutter and a red
/// underline on their line, their message on hover, and are listed under the code: each entry
/// moves the cursor to its line.
///
/// Wraps gpui-kit's `EditorState` (its tree-sitter grammars), so the look follows the theme
/// ([`theme::apply`] sets the code colours). An editable view emits [`CodeChanged`].
///
/// ```ignore
/// let code = cx.new(|cx| CodeView::new(CodeLanguage::Rust, source, window, cx).errors(vec![(6, "mismatched types".into())]).editable(true));
/// cx.subscribe(&code, |this, _, CodeChanged(text), cx| { /* … */ }).detach();
/// ```
pub struct CodeView {
    editor: Entity<EditorState>,
    language: CodeLanguage,
    errors: Vec<(usize, String)>,
    editable: bool,
    max_lines: Option<usize>,
    marks: GutterDecorationCollection,
    /// Errors or editability set by a builder, handed to the editor on the next draw.
    pending: bool,
    _subs: Vec<Subscription>,
}

impl EventEmitter<CodeChanged> for CodeView {}

impl CodeView {
    /// A read-only view of `text` highlighted as `language`.
    pub fn new(language: CodeLanguage, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text: SharedString = text.into();
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language(language.name()).default_value(text).gutter_decorations_width(px(4.));
            state.set_readonly(true, cx);
            state
        });
        let marks = editor.update(cx, |state, cx| state.create_gutter_decorations_collection(Vec::new(), cx));
        let sub = cx.subscribe(&editor, |this: &mut Self, editor, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) && this.editable {
                let text = editor.read(cx).value().to_string();
                // Marks follow the lines they were on; the owner sends new errors when it has them
                cx.emit(CodeChanged(text));
            }
        });
        Self { editor, language, errors: Vec::new(), editable: false, max_lines: None, marks, pending: false, _subs: vec![sub] }
    }

    /// Marks these errors: `(line, message)`, lines 1-based.
    pub fn errors(mut self, errors: Vec<(usize, String)>) -> Self {
        self.errors = errors;
        self.pending = true;
        self
    }

    /// Editable (`true`) or read-only (the default).
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self.pending = true;
        self
    }

    /// As tall as its text up to `lines` lines, then it scrolls (by default it fills the height
    /// its parent gives it).
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines.max(1));
        self
    }

    /// Marks these errors instead of the ones shown: `(line, message)`, lines 1-based.
    pub fn set_errors(&mut self, errors: Vec<(usize, String)>, cx: &mut Context<Self>) {
        self.errors = errors;
        self.apply_errors(cx);
        cx.notify();
    }

    /// The errors shown.
    pub fn error_list(&self) -> &[(usize, String)] {
        &self.errors
    }

    /// Makes the view editable or read-only.
    pub fn set_editable(&mut self, editable: bool, cx: &mut Context<Self>) {
        self.editable = editable;
        self.editor.update(cx, |state, cx| state.set_readonly(!editable, cx));
        cx.notify();
    }

    /// Whether the user can edit the text.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Replaces the text (no [`CodeChanged`]); the errors shown are kept on their line numbers.
    pub fn set_text(&mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = text.into();
        self.editor.update(cx, |state, cx| state.set_value(text, window, cx));
        self.apply_errors(cx);
        cx.notify();
    }

    /// The text.
    pub fn text(&self, cx: &gpui_kit::App) -> String {
        self.editor.read(cx).value().to_string()
    }

    /// Highlights the text as `language`.
    pub fn set_language(&mut self, language: CodeLanguage, cx: &mut Context<Self>) {
        self.language = language;
        self.editor.update(cx, |state, cx| state.set_highlighter(language.name(), cx));
        cx.notify();
    }

    /// The language highlighted.
    pub fn language(&self) -> CodeLanguage {
        self.language
    }

    /// The editor underneath (for its focus handle, cursor, search).
    pub fn editor(&self) -> &Entity<EditorState> {
        &self.editor
    }

    /// Moves the cursor to the start of `line` (1-based) and focuses the code.
    pub fn go_to_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        let row = line.saturating_sub(1) as u32;
        self.editor.update(cx, |state, cx| {
            state.set_cursor_position(Position::new(row, 0), window, cx);
            state.focus(window, cx);
        });
    }

    /// Hands the errors to the editor: diagnostics (underline, hover) and gutter marks.
    fn apply_errors(&mut self, cx: &mut Context<Self>) {
        let danger = color::danger();
        let errors = self.errors.clone();
        let marks = self.editor.update(cx, |state, cx| {
            let text = state.value().to_string();
            let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
            let mut marks = Vec::new();
            if let Some(set) = state.diagnostics_mut() {
                set.clear();
                for (line, message) in &errors {
                    let Some(&start) = starts.get(line.saturating_sub(1)) else { continue };
                    let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
                    let row = line.saturating_sub(1) as u32;
                    let chars = text[start..end].encode_utf16().count() as u32;
                    set.push(Diagnostic::new(Position::new(row, 0)..Position::new(row, chars.max(1)), message.clone()).with_severity(DiagnosticSeverity::Error));
                    // A whole line; an empty line by its newline
                    let range = if end > start { start..end } else { start..(start + 1).min(text.len()) };
                    marks.push(GutterDecoration::new(range).with_color(danger));
                }
            }
            cx.notify();
            marks
        });
        self.marks.set(marks, cx);
    }

    fn height(&self, cx: &gpui_kit::App) -> Option<Pixels> {
        let max = self.max_lines?;
        let lines = (self.editor.read(cx).value().matches('\n').count() + 1).clamp(1, max);
        let row = cx.theme().mono_font_size * 1.5;
        Some(row * lines as f32 + px(theme::MD + 2.))
    }
}

impl Render for CodeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.pending) {
            let editable = self.editable;
            self.editor.update(cx, |state, cx| state.set_readonly(!editable, cx));
            self.apply_errors(cx);
        }
        let height = self.height(cx);
        let editor = Editor::new(&self.editor).readonly(!self.editable).when_some(height, |e, h| e.h(h)).when(height.is_none(), |e| e.h_full());
        let id = cx.entity_id();
        let list = self.errors.iter().enumerate().map(|(i, (line, message))| {
            let line = *line;
            h_flex()
                .id(SharedString::from(format!("code-{id}-error-{i}")))
                .w_full()
                .min_h(px(theme::TARGET_MIN))
                .px(px(theme::SM))
                .gap(px(theme::SM))
                .items_start()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|d| d.bg(color::hover()))
                .child(div().flex_none().mt(px(5.)).w(px(3.)).h(px(14.)).rounded(px(1.)).bg(color::danger()))
                .child(div().flex_none().py(px(3.)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(color::danger()).child(format!("Line {line}")))
                .child(div().flex_1().min_w_0().py(px(3.)).text_sm().child(message.clone()))
                .on_click(cx.listener(move |this, _, window, cx| this.go_to_line(line, window, cx)))
                .test_support()
        });
        let has_errors = !self.errors.is_empty();
        v_flex()
            .w_full()
            .when(height.is_none(), |d| d.h_full())
            .gap(px(theme::XS))
            .child(div().w_full().min_h_0().when(height.is_none(), |d| d.flex_1()).child(editor))
            .when(has_errors, |d| {
                d.child(
                    v_flex()
                        .flex_none()
                        .w_full()
                        .child(
                            h_flex()
                                .gap(px(theme::XS))
                                .px(px(theme::SM))
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(Icon::new(IconName::CircleAlert).size(px(12.)).text_color(color::danger()))
                                .child(if self.errors.len() == 1 { "1 error".to_string() } else { format!("{} errors", self.errors.len()) }),
                        )
                        .children(list),
                )
            })
    }
}
