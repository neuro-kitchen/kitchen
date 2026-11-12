//! `kitchen-agent`: the agent side of the neuro-kitchen apps.
//!
//! - [`host`]: the MCP host. The app's registered commands are an agent's tools, called with
//!   arguments checked against their schemas, run under a [`host::Policy`] (what an agent may run
//!   without asking), each call recorded.
//! - [`acp`]: the agent behind the chat: any program speaking the Agent Client Protocol (`agy
//!   acp`, Claude Code's adapter), started by the app, reaching its commands through MCP.
//! - [`bridge`]: stdin and stdout joined to the app's MCP socket (how an agent reaches the app).
//! - [`workspace`]: the agent workspace: sessions, the conversation (kitchen-ui's `ChatPane`) with
//!   the calls waiting for the person, and the Changes panel or the project's files beside it;
//!   "Open in Agents" and the Agents section of Settings.
//!
//! The agent (the model and its keys) is the app's or the client's: nothing here holds a key.
//! `cargo run -p kitchen-agent --example agent-gallery` shows it with a small scripted agent.

#![warn(missing_docs)]

pub mod acp;
pub mod bridge;
pub mod host;
pub mod workspace;

pub use acp::{AcpClient, AcpUpdate, AgentSpec, McpStdio, PermissionAsk, known_agents};
pub use host::{CallRecord, HostEvent, McpHost, McpServer, Policy, ToolInfo, default_socket, serve_io, serve_stdio, tool_name, tools_of};
pub use workspace::{AgentEvent, AgentWorkspace, Session, Side};
