//! The empty state.

use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled as _, Window, div, px};

use crate::theme::{self, color};

/// Centred icon, title and a line of explanation, with actions under it (added as children).
#[derive(IntoElement)]
pub struct EmptyState {
    icon: Icon,
    title: SharedString,
    description: SharedString,
    actions: Vec<AnyElement>,
}

impl EmptyState {
    /// An empty state with `icon`, `title` and `description` (one or two sentences: what is
    /// missing and how to get it).
    pub fn new(icon: impl Into<Icon>, title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self { icon: icon.into(), title: title.into(), description: description.into(), actions: Vec::new() }
    }
}

impl ParentElement for EmptyState {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.actions.extend(elements);
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let has_actions = !self.actions.is_empty();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(theme::SM))
            .p(px(theme::LG))
            .child(self.icon.size(px(32.)).text_color(color::text_muted()))
            .child(div().pt(px(theme::XS)).text_lg().font_weight(FontWeight::SEMIBOLD).text_center().child(self.title))
            // A readable line length; long words wrap instead of overflowing
            .child(div().max_w(px(460.)).text_sm().text_center().text_color(color::text_muted()).child(self.description))
            .when(has_actions, |d| d.child(h_flex().pt(px(theme::SM)).gap(px(theme::SM)).flex_wrap().justify_center().children(self.actions)))
    }
}
