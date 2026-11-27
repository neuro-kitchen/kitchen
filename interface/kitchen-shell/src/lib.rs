//! The frame the neuro-kitchen apps share. An app registers what it can do; the shell draws it
//! the same way everywhere.
//!
//! - [`command`]: one list of commands ([`command::CommandSpec`] in a [`command::Registry`]) that
//!   becomes the menus, the search, the key bindings and an agent's tools.
//! - [`search`]: the search over the registry (Ctrl+K).
//! - [`frame`]: the frame ([`frame::Shell`]): title bar (general menus, the app's views, the
//!   shared workspaces), menu row, status bar with jobs and meters, Settings, search; it adapts
//!   to the window's width.
//! - [`jobs`]: what runs in the background, with progress and cancel.
//! - [`keys`]: `keybindings.toml`, the keys the user gives the commands.
//! - [`activity`]: what happened, for the timeline: every command run (who, how, with what),
//!   time in each workspace, jobs, what steps produced.
//! - [`env`]: a project's environments for its scripts: the Python venv (made and synced by uv
//!   from `pyproject.toml` and `uv.lock`) and the Rust workspace and toolchain pin.
//! - [`changes`]: the Changes panel: watched project files as diffs against a baseline, an
//!   agent's proposed change reviewed hunk by hunk.
//! - [`picker`] and [`picker_view`]: the file picker File → Open… brings up: the folder model
//!   (tested without a window) and its popup.
//! - [`container`]: the view container: a dock of views, "Add view or container", arrangements,
//!   the buttons in a view's tab.
//!
//! `cargo run -p kitchen-shell --example shell-gallery` shows each piece.

#![warn(missing_docs)]

// GPUI's derive macros name `gpui`; through the kit, that is gpui-kit
extern crate gpui_kit as gpui;

pub mod activity;
pub mod changes;
pub mod command;
/// Jobs, environments and people: kitchen-services', drawn by the frame and its Settings.
pub use kitchen_services::{env, jobs, people};
/// The view container, now kitchen-views' (re-exported: apps keep `kitchen_shell::container`).
pub use kitchen_views::container;
pub mod frame;
pub mod keys;
pub mod people_view;
pub mod picker;
pub mod picker_view;
pub mod search;

pub use changes::{Baseline, ChangesEvent, ChangesPanel};
pub use command::{CommandSpec, MenuItem, MenuProvider, MenuSpec, Registry, RunCommand};
pub use container::{AddViewDialog, Arrangement, DialogEvent, ViewOption};
pub use frame::{OpenSpec, Placement, RemoteSpec, SettingsSection, Shell, ShellEvent, ShellSpec, StatusItems, TitleFit, WorkspaceSpec, fit, frame_rounded, resize_edge};
pub use picker::{FileKind, Picker};
pub use picker_view::{FilePickerView, PickerEvent};
pub use search::{Search, SearchEvent};
