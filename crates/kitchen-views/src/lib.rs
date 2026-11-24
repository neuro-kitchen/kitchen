//! Views the neuro-kitchen apps share, and what views share between them.
//!
//! - [`container`]: the dock every workspace's views live in (tabs dragged into splits, the
//!   "Add view or container" dialog). Moved from kitchen-shell, which re-exports it.
//! - [`clock`]: one time cursor per workspace that every view follows and any view moves;
//!   playback, frame steps, and the playback keys (`[playback]`).
//!
//! A view moves here when a second app needs it, not before: dspk's traces and lucem's image
//! planes are tuned to their data and stay in their apps, behind the same clock.

// GPUI's derive macros name `gpui`; through the kit, that is gpui-kit
extern crate gpui_kit as gpui;

pub mod clock;
pub mod container;

pub use clock::{Clock, ClockEvent};
pub use container::{AddViewDialog, Arrangement, DialogEvent, ViewOption};
