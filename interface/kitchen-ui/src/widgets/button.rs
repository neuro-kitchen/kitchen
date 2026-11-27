//! The icon button.

use gpui_kit::SharedString;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::ElementId;

/// A small ghost icon button with a tooltip (the tooltip names what it does, since the icon
/// alone may not). gpui-kit's small size: about 24 px square at the default text size
/// (designparser `touch-target`).
pub fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>, tooltip: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().small().icon(icon).tooltip(tooltip.into())
}
