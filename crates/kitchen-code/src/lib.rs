//! The Code workspace the neuro-kitchen apps share. Code is how a user takes an app further than
//! it was written: a garnish statistic, a dspk operation, a lucem filter, each a script that
//! becomes a command (menus, search, an agent's tools) or a pipeline step with typed ports.
//!
//! - [`header`]: what a script says it is (`kitchen:` lines at its top).
//! - [`script`]: a script file.
//! - [`run`]: running one: JSON in on stdin, its result as JSON on its last line, its errors read
//!   back onto its own lines.
//! - [`workspace`]: the workspace: scripts tree, editor, run panel ([`CodeWorkspace`]).
//!
//! It was kitchen-shell's Scripts workspace, and lucem's Code workspace beside it; one crate now,
//! so every app gets the same one.

// GPUI's derive macros name `gpui`; through the kit, that is gpui-kit
extern crate gpui_kit as gpui;

pub mod header;
pub mod run;
pub mod script;
pub mod workspace;

pub use header::{Port, ScriptMeta};
pub use run::{RunOutput, Runner, default_runner, script_errors};
pub use script::Script;
pub use workspace::{CodeEvent, CodeWorkspace, ContextFn, NodeSpec};
