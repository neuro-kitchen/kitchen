//! The diff view: a git-style diff of two texts or two JSON documents, with unchanged runs folded
//! and, for reviewing an agent's proposed change, Accept / Reject per hunk.

use std::collections::BTreeSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window, div, px,
};

use crate::diff::{ARRAY_SUMMARY, CONTEXT, Diff, DiffLine, LineKind};
use crate::theme::{self, color};
use crate::widgets::icon_button;

/// What the reviewer decided for a hunk ([`DiffView::reviewable`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HunkDecision {
    /// The hunk ([`Diff::hunks`] index).
    pub hunk: usize,
    /// Accepted (`true`) or rejected.
    pub accept: bool,
}

/// A git-style diff: added lines on a green tint, removed lines on a red one (the shared
/// success / danger colours, contrast-checked in both themes), old and new line numbers, a header
/// per hunk, and the unchanged runs between hunks folded to one line ("⋯ 12 unchanged lines",
/// a click shows them). Each hunk keeps [`CONTEXT`] unchanged lines around its changes.
///
/// [`DiffView::json`] compares JSON by key (sorted keys, long arrays as one summary line: see
/// [`crate::diff`]). [`DiffView::reviewable`] adds Accept / Reject to each hunk; a choice emits
/// [`HunkDecision`] and stays shown. The header counts the lines and copies the diff as text.
pub struct DiffView {
    id: SharedString,
    title: Option<SharedString>,
    diff: Diff,
    expanded: BTreeSet<usize>,
    reviewable: bool,
    decisions: Vec<Option<bool>>,
}

impl EventEmitter<HunkDecision> for DiffView {}

impl DiffView {
    /// A view of `diff`. `id` names its elements (unique in the window).
    pub fn new(id: impl Into<SharedString>, diff: Diff) -> Self {
        let decisions = vec![None; diff.hunks.len()];
        Self { id: id.into(), title: None, diff, expanded: BTreeSet::new(), reviewable: false, decisions }
    }

    /// The line diff of two texts.
    pub fn text(id: impl Into<SharedString>, old: &str, new: &str) -> Self {
        Self::new(id, Diff::text(old, new, CONTEXT))
    }

    /// Two JSON documents compared by key ([`Diff::json`]): reordered keys are not changes, long
    /// arrays are one line.
    pub fn json(id: impl Into<SharedString>, old: &str, new: &str) -> Self {
        Self::new(id, Diff::json(old, new, CONTEXT, ARRAY_SUMMARY))
    }

    /// A title in the header (a file name: `session.json`).
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Accept / Reject buttons on each hunk (an agent's proposed change under review).
    pub fn reviewable(mut self, reviewable: bool) -> Self {
        self.reviewable = reviewable;
        self
    }

    /// Shows another diff (the decisions and unfolded runs are cleared).
    pub fn set_diff(&mut self, diff: Diff, cx: &mut Context<Self>) {
        self.decisions = vec![None; diff.hunks.len()];
        self.expanded.clear();
        self.diff = diff;
        cx.notify();
    }

    /// The diff shown.
    pub fn diff(&self) -> &Diff {
        &self.diff
    }

    /// The decision on each hunk so far (`None`: not decided).
    pub fn decisions(&self) -> &[Option<bool>] {
        &self.decisions
    }

    /// Accepts or rejects `hunk` and tells the owner.
    pub fn decide(&mut self, hunk: usize, accept: bool, cx: &mut Context<Self>) {
        if let Some(d) = self.decisions.get_mut(hunk) {
            *d = Some(accept);
            cx.emit(HunkDecision { hunk, accept });
            cx.notify();
        }
    }

    /// Shows (or folds again) the unchanged run [`Diff::gaps`]`[gap]`.
    pub fn toggle_gap(&mut self, gap: usize, cx: &mut Context<Self>) {
        if !self.expanded.remove(&gap) {
            self.expanded.insert(gap);
        }
        cx.notify();
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (added, removed) = (self.diff.added(), self.diff.removed());
        let unified = self.diff.unified();
        let copy = icon_button(SharedString::from(format!("{}-copy", self.id)), IconName::Copy, "Copy the diff").on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(unified.clone()));
        });
        let all_open = self.expanded.len() == self.diff.gaps().len();
        let gaps = self.diff.gaps().len();
        let unfold = icon_button(SharedString::from(format!("{}-unfold", self.id)), if all_open { IconName::ChevronUp } else { IconName::ChevronsUpDown }, if all_open { "Fold the unchanged lines" } else { "Show every line" })
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.expanded.len() == gaps {
                    this.expanded.clear();
                } else {
                    this.expanded = (0..gaps).collect();
                }
                cx.notify();
            }));
        h_flex()
            .w_full()
            .gap(px(theme::SM))
            .px(px(theme::SM))
            .py(px(theme::XS))
            .bg(color::raised())
            .border_b_1()
            .border_color(color::border())
            .when_some(self.title.clone(), |d, t| d.child(div().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(t)))
            .child(div().text_sm().font_family("monospace").text_color(color::success()).child(format!("+{added}")))
            .child(div().text_sm().font_family("monospace").text_color(color::danger()).child(format!("−{removed}")))
            .when(self.diff.json, |d| d.child(div().text_xs().text_color(color::text_muted()).child("JSON, by key")))
            .child(div().flex_1())
            .when(gaps > 0, |d| d.child(unfold))
            .child(copy)
    }

    fn hunk_header(&self, i: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let h = &self.diff.hunks[i];
        let decided = self.decisions.get(i).copied().flatten();
        let review = self.reviewable.then(|| {
            let accept = Button::new(SharedString::from(format!("{}-accept-{i}", self.id)))
                .small()
                .icon(IconName::Check)
                .label("Accept")
                .when(decided == Some(true), |b| b.primary())
                .when(decided != Some(true), |b| b.outline())
                .selected(decided == Some(true))
                .tooltip("Keep this change")
                .on_click(cx.listener(move |this, _, _, cx| this.decide(i, true, cx)));
            let reject = Button::new(SharedString::from(format!("{}-reject-{i}", self.id)))
                .small()
                .icon(IconName::Close)
                .label("Reject")
                .when(decided == Some(false), |b| b.danger())
                .when(decided != Some(false), |b| b.outline())
                .selected(decided == Some(false))
                .tooltip("Drop this change")
                .on_click(cx.listener(move |this, _, _, cx| this.decide(i, false, cx)));
            h_flex().gap(px(theme::XS)).child(accept).child(reject)
        });
        let status = decided.map(|a| {
            let (text, c) = if a { ("Accepted", color::success()) } else { ("Rejected", color::danger()) };
            div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c).child(text)
        });
        h_flex()
            .w_full()
            .min_h(px(theme::TARGET_MIN + theme::XS))
            .gap(px(theme::SM))
            .px(px(theme::SM))
            .bg(color::raised())
            .border_t_1()
            .border_b_1()
            .border_color(color::border())
            .child(div().flex_1().min_w_0().truncate().text_xs().font_family("monospace").text_color(color::text_muted()).child(h.header()))
            .children(status)
            .children(review)
    }

    fn gap_row(&self, gap: usize, len: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let label = if len == 1 { "⋯ 1 unchanged line".to_string() } else { format!("⋯ {} unchanged lines", crate::diff::group_digits(len)) };
        h_flex()
            .id(SharedString::from(format!("{}-gap-{gap}", self.id)))
            .w_full()
            .min_h(px(theme::TARGET_MIN))
            .px(px(theme::SM))
            .bg(color::panel())
            .text_xs()
            .text_color(color::text_muted())
            .cursor_pointer()
            .hover(|d| d.bg(color::hover()))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_gap(gap, cx)))
            .test_support()
    }
}

/// One line of the diff: old number, new number, sign, text.
fn line_row(l: &DiffLine) -> AnyElement {
    let (bg, sign, sign_color): (Option<Hsla>, &str, Hsla) = match l.kind {
        LineKind::Context => (None, " ", color::text_muted()),
        LineKind::Added => (Some(color::diff_added()), "+", color::success()),
        LineKind::Removed => (Some(color::diff_removed()), "−", color::danger()),
        LineKind::Summary => (Some(color::control()), "~", color::text_muted()),
    };
    let num = |n: Option<usize>| div().flex_none().w(px(40.)).pr(px(theme::XS)).text_right().text_color(color::text_muted()).child(n.map(|n| n.to_string()).unwrap_or_default());
    h_flex()
        .w_full()
        .items_start()
        .font_family("monospace")
        .text_xs()
        .when_some(bg, |d, bg| d.bg(bg))
        .child(num(l.old))
        .child(num(l.new))
        .child(div().flex_none().w(px(16.)).text_center().font_weight(FontWeight::SEMIBOLD).text_color(sign_color).child(sign))
        .child(div().flex_1().min_w_0().pr(px(theme::SM)).when(l.kind == LineKind::Summary, |d| d.italic()).child(if l.text.is_empty() { " ".to_string() } else { l.text.clone() }))
        .into_any_element()
}

impl Render for DiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let gaps = self.diff.gaps();
        let mut body: Vec<AnyElement> = Vec::new();
        if let Some(note) = &self.diff.note {
            body.push(div().px(px(theme::SM)).py(px(theme::XS)).text_xs().text_color(color::warning()).child(note.clone()).into_any_element());
        }
        if self.diff.is_empty() {
            body.push(div().px(px(theme::SM)).py(px(theme::SM)).text_sm().text_color(color::text_muted()).child("No changes.").into_any_element());
        } else {
            // Walk the lines: folded runs and hunks in order
            let mut gi = 0;
            for hi in 0..self.diff.hunks.len() {
                let start = self.diff.hunks[hi].lines.start;
                while gi < gaps.len() && gaps[gi].end <= start {
                    body.extend(self.gap(gi, &gaps, cx));
                    gi += 1;
                }
                body.push(self.hunk_header(hi, cx).into_any_element());
                body.extend(self.diff.lines[self.diff.hunks[hi].lines.clone()].iter().map(line_row));
            }
            while gi < gaps.len() {
                body.extend(self.gap(gi, &gaps, cx));
                gi += 1;
            }
        }
        v_flex()
            .id(self.id.clone())
            .size_full()
            .rounded(px(theme::RADIUS_LG))
            .border_1()
            .border_color(color::border())
            .bg(color::panel())
            .overflow_hidden()
            .child(self.header(cx))
            .child(v_flex().id(SharedString::from(format!("{}-body", self.id))).flex_1().min_h_0().w_full().overflow_y_scroll().children(body))
    }
}

impl DiffView {
    /// A folded run as its expander, or its lines when shown.
    fn gap(&self, gi: usize, gaps: &[std::ops::Range<usize>], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let range = gaps[gi].clone();
        if self.expanded.contains(&gi) {
            let mut out = vec![self.gap_row_open(gi, cx).into_any_element()];
            out.extend(self.diff.lines[range].iter().map(line_row));
            out
        } else {
            vec![self.gap_row(gi, range.len(), cx).into_any_element()]
        }
    }

    fn gap_row_open(&self, gap: usize, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id(SharedString::from(format!("{}-gap-{gap}", self.id)))
            .w_full()
            .min_h(px(theme::TARGET_MIN))
            .px(px(theme::SM))
            .text_xs()
            .text_color(color::text_muted())
            .cursor_pointer()
            .hover(|d| d.bg(color::hover()))
            .child("Fold the unchanged lines")
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_gap(gap, cx)))
            .test_support()
    }
}
