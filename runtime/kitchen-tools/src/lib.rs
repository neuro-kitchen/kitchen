//! Tools and plugins for the neuro-kitchen apps: separate programs an app or the orchestrator runs,
//! never links, over JSON lines on stdin and stdout.
//!
//! - [`plugin`]: the plugin protocol (handshake, capabilities, progress), discovery, a client kept
//!   alive between requests, and the manager (install from a file or a verified download, update,
//!   remove, disk use). The manager's Settings page is kitchen-shell's.
//! - [`tool`]: what a tool answers beside it: `describe` (its operations, their ports as the
//!   contract's type and format), `validate` and `run`; the tool's side ([`HeadlessTool`],
//!   [`serve_stdio`]) and the orchestrator's ([`ToolClient`]).
//!
//! kitchen-plugin and kitchen-tool until phase 2, step 3d.

pub mod plugin;
pub mod tool;

pub use plugin::{CoreRequest, Event, Installed, Manager, PROTOCOL, PluginInfo, PluginKind, PluginProcess, RuntimeStatus};
pub use tool::{HeadlessTool, InputPort, OutputPort, ReviewFlag, RunInput, RunOutput, RunReport, RunRequest, TOOL_PROTOCOL, ToolClient, ToolDescription, ToolRef, ToolRequest, Validation, check_inputs, plugin_info, serve_stdio};
