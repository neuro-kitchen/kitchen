//! The agent behind the chat: any program that speaks the Agent Client Protocol (ACP, the stdio
//! protocol editors use to embed agents), started by the app: `agy acp` (Antigravity), Claude Code
//! through its ACP adapter, or any other. The agent keeps its own login and model: the app holds
//! no key. It reaches the app's commands through MCP: the app passes it an MCP server (usually the
//! app's own program bridging to its socket, [`crate::bridge`]), so every command it runs goes
//! through the app's [`crate::McpHost`] and its policy.
//!
//! [`AcpClient::start`] runs the connection on its own thread; the app sends prompts and cancels
//! ([`AcpClient::prompt`], [`AcpClient::cancel`]) and reads what comes back as [`AcpUpdate`]s
//! (streamed text, the agent's own tool calls, permission questions, the end of a turn).

use std::path::PathBuf;
use std::str::FromStr as _;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{
    CancelNotification, ContentBlock, InitializeRequest, McpServer, McpServerStdio, NewSessionRequest, PermissionOptionKind, PromptRequest, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate, TextContent, ToolCallStatus,
};
use agent_client_protocol::{AcpAgent, Agent};
use serde_json::Value;

/// An agent the app can start: its name and the command that starts it as an ACP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSpec {
    /// What Settings calls it ("Antigravity").
    pub name: String,
    /// The command, split as a shell would (`agy acp`), or ACP's JSON form.
    pub command: String,
}

impl AgentSpec {
    /// The agent `name` started by `command`.
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self { name: name.into(), command: command.into() }
    }

    /// Whether its program is found (on the `PATH`, or as a path).
    pub fn available(&self) -> bool {
        let program = self.command.split_whitespace().next().unwrap_or_default();
        !program.is_empty() && find_program(program).is_some()
    }
}

/// The ACP agents the apps know, in order of preference: Antigravity (`agy acp`), then Claude Code
/// through Zed's adapter (`claude-code-acp`, from npm). Others can be named in Settings.
pub fn known_agents() -> Vec<AgentSpec> {
    vec![AgentSpec::new("Antigravity", "agy acp"), AgentSpec::new("Claude Code", "claude-code-acp")]
}

/// Where `program` is: itself when it is a path, else the first match on the `PATH`.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let p = std::path::Path::new(program);
    if p.components().count() > 1 {
        return p.is_file().then(|| p.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(program)).find(|c| c.is_file())
}

/// An MCP server the agent starts for the session (stdio): usually the app bridging to its
/// socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpStdio {
    /// Its name, as the agent lists its tools (`lumen`).
    pub name: String,
    /// The program (an absolute path).
    pub command: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
}

/// A question from the agent: may it do this (its own tool: edit a file, run a shell command)?
#[derive(Debug)]
pub struct PermissionAsk {
    /// What it wants to do.
    pub title: String,
    /// The answers it offers: id, name and whether it allows.
    pub options: Vec<(String, String, bool)>,
    reply: async_channel::Sender<Option<String>>,
}

impl PermissionAsk {
    /// Answers with the option `id` (`None`: refuse).
    pub fn answer(self, id: Option<String>) {
        let _ = self.reply.try_send(id);
    }
}

/// What comes back from the agent.
#[derive(Debug)]
pub enum AcpUpdate {
    /// The agent started and the session is open (its name and version).
    Ready(String),
    /// A piece of the agent's answer (markdown).
    Text(String),
    /// A piece of the agent's reasoning.
    Thought(String),
    /// The agent started one of its tools (an MCP command of the app shows through the host).
    ToolCall {
        /// Its id in the session.
        id: String,
        /// What it does.
        title: String,
        /// Its input, when the agent says.
        input: Value,
    },
    /// One of its tools finished.
    ToolDone {
        /// Its id.
        id: String,
        /// Whether it succeeded.
        ok: bool,
        /// Its output, when the agent says.
        output: Value,
    },
    /// The agent asks the person.
    Permission(PermissionAsk),
    /// The turn ended (why: `end_turn`, `cancelled`, `max_tokens`…).
    TurnEnded(String),
    /// Something failed (the agent could not start, a prompt failed).
    Error(String),
    /// The connection closed.
    Closed,
}

enum Cmd {
    Prompt(String),
    Cancel,
    NewSession,
}

/// A running agent.
pub struct AcpClient {
    spec: AgentSpec,
    cmds: async_channel::Sender<Cmd>,
    /// What comes back (read it on the UI side).
    pub updates: async_channel::Receiver<AcpUpdate>,
}

impl AcpClient {
    /// Starts `spec`'s program in `cwd`, opens a session with the MCP servers `mcp`, and serves
    /// it on a thread of its own until the client is dropped or the agent exits.
    pub fn start(spec: AgentSpec, cwd: PathBuf, mcp: Vec<McpStdio>) -> Self {
        let (cmd_tx, cmd_rx) = async_channel::unbounded();
        let (up_tx, up_rx) = async_channel::unbounded();
        let command = spec.command.clone();
        let spawned = std::thread::Builder::new().name("kitchen-acp".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(r) => r,
                Err(e) => {
                    let _ = up_tx.try_send(AcpUpdate::Error(e.to_string()));
                    return;
                }
            };
            if let Err(e) = runtime.block_on(run(&command, cwd, mcp, cmd_rx, up_tx.clone())) {
                let _ = up_tx.try_send(AcpUpdate::Error(e));
            }
            let _ = up_tx.try_send(AcpUpdate::Closed);
        });
        if let Err(e) = spawned {
            let (tx, rx) = async_channel::unbounded();
            let _ = tx.try_send(AcpUpdate::Error(e.to_string()));
            let _ = tx.try_send(AcpUpdate::Closed);
            return Self { spec, cmds: cmd_tx, updates: rx };
        }
        Self { spec, cmds: cmd_tx, updates: up_rx }
    }

    /// The agent it runs.
    pub fn spec(&self) -> &AgentSpec {
        &self.spec
    }

    /// Sends a prompt (the answer streams back as updates; a prompt sent during a turn waits).
    pub fn prompt(&self, text: impl Into<String>) {
        let _ = self.cmds.try_send(Cmd::Prompt(text.into()));
    }

    /// Asks the agent to stop the current turn.
    pub fn cancel(&self) {
        let _ = self.cmds.try_send(Cmd::Cancel);
    }

    /// Starts a new session (the agent forgets the conversation so far).
    pub fn new_session(&self) {
        let _ = self.cmds.try_send(Cmd::NewSession);
    }
}

impl Drop for AcpClient {
    fn drop(&mut self) {
        // The connection ends when its commands stop: the agent's process is closed with it
        self.cmds.close();
    }
}

fn text_of(content: &ContentBlock) -> Option<String> {
    match content {
        ContentBlock::Text(t) => Some(t.text.clone()),
        _ => None,
    }
}

async fn run(command: &str, cwd: PathBuf, mcp: Vec<McpStdio>, cmds: async_channel::Receiver<Cmd>, up: async_channel::Sender<AcpUpdate>) -> Result<(), String> {
    let agent = AcpAgent::from_str(command).map_err(|e| format!("{command}: {e}"))?;
    let (notes, asks) = (up.clone(), up.clone());
    let servers: Vec<McpServer> = mcp.iter().map(|m| McpServer::Stdio(McpServerStdio::new(m.name.clone(), m.command.clone()).args(m.args.clone()))).collect();
    let mcp_names: Vec<String> = mcp.iter().map(|m| m.name.clone()).collect();
    agent_client_protocol::Client
        .v1()
        .name("kitchen")
        .on_receive_notification(
            async move |n: SessionNotification, _cx| {
                let update = match n.update {
                    SessionUpdate::AgentMessageChunk(c) => text_of(&c.content).map(AcpUpdate::Text),
                    SessionUpdate::AgentThoughtChunk(c) => text_of(&c.content).map(AcpUpdate::Thought),
                    SessionUpdate::ToolCall(t) => {
                        // The app's own commands show through its host: not twice
                        let name = t.name.clone().unwrap_or_default();
                        let own = mcp_names.iter().any(|m| name.contains(m.as_str()) || t.title.contains(&format!("{m}__")) || t.title.starts_with(&format!("{m}.")));
                        (!own).then(|| AcpUpdate::ToolCall { id: t.tool_call_id.to_string(), title: t.title.clone(), input: t.raw_input.clone().unwrap_or(Value::Null) })
                    }
                    SessionUpdate::ToolCallUpdate(u) => match u.fields.status {
                        Some(ToolCallStatus::Completed) => Some(AcpUpdate::ToolDone { id: u.tool_call_id.to_string(), ok: true, output: u.fields.raw_output.clone().unwrap_or(Value::Null) }),
                        Some(ToolCallStatus::Failed) => Some(AcpUpdate::ToolDone { id: u.tool_call_id.to_string(), ok: false, output: u.fields.raw_output.clone().unwrap_or(Value::Null) }),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(u) = update {
                    let _ = notes.send(u).await;
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _connection| {
                let (tx, rx) = async_channel::bounded(1);
                let options = request.options.iter().map(|o| (o.option_id.to_string(), o.name.clone(), matches!(o.kind, PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways))).collect();
                let title = request.tool_call.fields.title.clone().unwrap_or_else(|| "use one of its tools".to_string());
                let _ = asks.send(AcpUpdate::Permission(PermissionAsk { title, options, reply: tx })).await;
                let outcome = match rx.recv().await {
                    Ok(Some(id)) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id)),
                    _ => RequestPermissionOutcome::Cancelled,
                };
                responder.respond(RequestPermissionResponse::new(outcome))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
            let init = connection.send_request(InitializeRequest::new(ProtocolVersion::V1)).block_task().await?;
            let name = init.agent_info.as_ref().map(|i| format!("{} {}", i.name, i.version)).unwrap_or_else(|| "agent".to_string());
            let open = || connection.send_request(NewSessionRequest::new(cwd.clone()).mcp_servers(servers.clone())).block_task();
            let mut session = open().await?.session_id;
            let _ = up.send(AcpUpdate::Ready(name)).await;
            while let Ok(cmd) = cmds.recv().await {
                match cmd {
                    Cmd::Cancel => {}
                    Cmd::NewSession => session = open().await?.session_id,
                    Cmd::Prompt(text) => {
                        let mut turn = std::pin::pin!(connection.send_request(PromptRequest::new(session.clone(), vec![ContentBlock::Text(TextContent::new(text))])).block_task());
                        // While the turn runs, a cancel is passed on; the turn then ends as cancelled
                        let reply = loop {
                            let next = std::pin::pin!(cmds.recv());
                            match futures_lite_select(turn.as_mut(), next).await {
                                Either::Left(r) => break r,
                                Either::Right(Ok(Cmd::Cancel)) => {
                                    let _ = connection.send_notification(CancelNotification::new(session.clone()));
                                }
                                // Prompts and new sessions wait for the turn
                                Either::Right(Ok(_)) => {}
                                Either::Right(Err(_)) => return Ok(()),
                            }
                        };
                        match reply {
                            Ok(r) => {
                                let why = serde_json::to_value(r.stop_reason).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "end_turn".into());
                                let _ = up.send(AcpUpdate::TurnEnded(why)).await;
                            }
                            Err(e) => {
                                let _ = up.send(AcpUpdate::Error(e.to_string())).await;
                                let _ = up.send(AcpUpdate::TurnEnded("error".into())).await;
                            }
                        }
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

/// The first of two futures to finish (the other is left as it is).
async fn futures_lite_select<A: std::future::Future + Unpin, B: std::future::Future + Unpin>(mut a: A, mut b: B) -> Either<A::Output, B::Output> {
    std::future::poll_fn(move |cx| {
        if let std::task::Poll::Ready(v) = std::pin::Pin::new(&mut a).poll(cx) {
            return std::task::Poll::Ready(Either::Left(v));
        }
        if let std::task::Poll::Ready(v) = std::pin::Pin::new(&mut b).poll(cx) {
            return std::task::Poll::Ready(Either::Right(v));
        }
        std::task::Poll::Pending
    })
    .await
}
