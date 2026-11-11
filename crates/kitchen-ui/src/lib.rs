//! The neuro-kitchen apps' shared UI (GPUI through gpui-kit).

#![warn(missing_docs)]

pub mod param_editor;
pub mod theme;
pub mod widgets;

pub use param_editor::{Commit, ParamEditor, Params};
pub use theme::{Look, Palette};
