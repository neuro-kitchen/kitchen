//! The neuro-kitchen apps' shared UI (GPUI through gpui-kit).
//!
//! - [`theme`]: design tokens (spacing, type, radii, pane gaps), the shared grey neutrals, the
//!   app's accent [`theme::Palette`] or the user's own ([`theme::Rgb`]), light and dark;
//!   [`theme::apply`] hands them to gpui-kit.
//! - [`widgets`]: accent picker, section, labelled row, slider, dropdown, stepper, dragged
//!   number, swatch, empty state, icon button; code view (highlighting, errors), diff view (text
//!   or JSON, Accept / Reject per hunk), text view (light markdown).
//! - [`diff`]: the line diffs the diff view shows, as plain data.
//! - [`param_editor`]: the parameters of any op that serializes to JSON, editable.
//! - [`icons`]: what an app ships (its icon set and files) and the test that every icon its code
//!   names is shipped.
//!
//! Start-up: `gpui_kit::init(cx)`, then `theme::use_look(Look::new(MY_PALETTE), cx)`; open
//! windows with `gpui_kit::open_window`. `cargo run -p kitchen-ui --example gallery` shows every
//! widget.

#![warn(missing_docs)]

pub mod diff;
pub mod icons;
pub mod param_editor;
pub mod theme;
pub mod widgets;

pub use param_editor::{Commit, ParamEditor, Params};
pub use theme::{Look, Palette, Rgb};
