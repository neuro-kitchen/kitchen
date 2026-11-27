//! The labelled row.

use gpui_kit::component::h_flex;
use gpui_kit::{IntoElement, ParentElement as _, SharedString, Styled as _, div, px};

use crate::theme::{self, color};

/// `label    control` on one line when both fit: the label [`theme::LABEL_W`] wide, the control
/// the rest (at least [`theme::CONTROL_MIN_W`]). In a narrow panel the control drops under its
/// label, full width. Nothing is clipped (a cut label names nothing): a long label wraps, a long
/// value in a control ends in "…".
pub fn row(label: impl Into<SharedString>, control: impl IntoElement) -> impl IntoElement {
    h_flex()
        .w_full()
        .flex_wrap()
        .gap(px(theme::SM))
        .child(div().flex_none().w(px(theme::LABEL_W)).text_sm().text_color(color::text_muted()).child(label.into()))
        .child(h_flex().flex_1().min_w(px(theme::CONTROL_MIN_W)).justify_end().child(control))
}
