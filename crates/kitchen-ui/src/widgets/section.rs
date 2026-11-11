//! The titled section.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled as _, Window, div, px};

use crate::theme::{self, color};

/// A titled block of a panel: a short caps title (with something at its right, if given), then
/// its rows (added as children), [`theme::SM`] apart. Boxed ([`Section::boxed`]), the rows sit in
/// a bordered box.
#[derive(IntoElement)]
pub struct Section {
    title: SharedString,
    extra: Option<AnyElement>,
    boxed: bool,
    children: Vec<AnyElement>,
}

impl Section {
    /// A section titled `title` (shown in capitals: a short label, designparser
    /// `all-caps-fatigue`).
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), extra: None, boxed: false, children: Vec::new() }
    }

    /// Something at the right of the title (a count, a button).
    pub fn extra(mut self, e: impl IntoElement) -> Self {
        self.extra = Some(e.into_any_element());
        self
    }

    /// The rows in a bordered box (dsp-kitchen's sections): padding [`theme::MD`], 1.5 × the row
    /// gap (designparser `padding-gap-ratio`).
    pub fn boxed(mut self) -> Self {
        self.boxed = true;
        self
    }
}

impl ParentElement for Section {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Section {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { title, extra, boxed, children } = self;
        let head = h_flex()
            .w_full()
            .min_h(px(20.))
            .gap(px(theme::SM))
            .child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child(title.to_uppercase()))
            .children(extra);
        let body = v_flex().w_full().min_w_0().gap(px(theme::SM)).children(children).when(boxed, |d| {
            d.p(px(theme::MD)).rounded(px(theme::RADIUS_LG)).border_1().border_color(color::border()).bg(color::raised())
        });
        v_flex().w_full().min_w_0().gap(px(theme::SM)).child(head).child(body)
    }
}
