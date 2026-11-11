//! `kitchen-agent`: the agent side of the neuro-kitchen apps.
//!
//! - [`host`]: the MCP host. The app's registered commands are an agent's tools, called with
//!   arguments checked against their schemas, run under a [`host::Policy`] (what an agent may run
//!   without asking), each call recorded.
//! - [`workspace`]: the agent workspace: sessions, the conversation (kitchen-ui's `ChatPane`) with
//!   the calls waiting for the person, and the Changes panel or the project's files beside it;
//!   "Open in Agents" and the Agents section of Settings.
//!
//! The agent (the model and its keys) is the app's or the client's: nothing here holds a key.
//! `cargo run -p kitchen-agent --example gallery` shows it with a small scripted agent.

#![warn(missing_docs)]

pub mod host;
pub mod workspace;

pub use host::{CallRecord, HostEvent, McpHost, McpServer, Policy, ToolInfo, serve_io, serve_stdio, tools_of};
pub use workspace::{AgentEvent, AgentWorkspace, Session, Side};
