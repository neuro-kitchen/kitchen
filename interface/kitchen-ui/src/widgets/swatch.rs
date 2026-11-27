//! The colour swatch.

use gpui_kit::{Hsla, IntoElement, Styled as _, div, px};

use crate::theme::{self, color};

/// A colour chip `size` px square, outlined so a colour close to the panel's still shows its
/// edge. For 0–255 RGB use [`crate::theme::rgb8`].
pub fn swatch(c: impl Into<Hsla>, size: f32) -> impl IntoElement {
    div().flex_none().size(px(size)).rounded(px(theme::RADIUS_SM)).border_1().border_color(color::border()).bg(c.into())
}
