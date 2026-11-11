//! The frame the neuro-kitchen apps share. An app registers what it can do; the shell draws it
//! the same way everywhere.
//!
//! - [`command`]: one list of commands ([`command::CommandSpec`] in a [`command::Registry`]) that
//!   becomes the menus, the search, the key bindings and an agent's tools.
//! - [`search`]: the search over the registry (Ctrl+K).
//! - [`frame`]: the frame ([`frame::Shell`]): title bar, workspaces, menu row, status bar with
//!   jobs and meters, Settings, search.
//! - [`jobs`]: what runs in the background, with progress and cancel.
//! - [`container`]: the view container: a dock of views, "Add view or container", arrangements,
//!   the buttons in a view's tab.
//!
//! `cargo run -p kitchen-shell --example gallery` shows each piece.

#![warn(missing_docs)]

// GPUI's derive macros name `gpui`; through the kit, that is gpui-kit
extern crate gpui_kit as gpui;

pub mod command;
pub mod container;
pub mod frame;
pub mod jobs;
pub mod search;

pub use command::{CommandSpec, MenuItem, MenuSpec, Registry, RunCommand};
pub use container::{AddViewDialog, Arrangement, DialogEvent, ViewOption};
pub use frame::{RemoteSpec, SettingsSection, Shell, ShellEvent, ShellSpec, WorkspaceSpec};
pub use search::{Search, SearchEvent};
