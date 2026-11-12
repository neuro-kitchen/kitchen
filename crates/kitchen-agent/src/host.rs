//! The MCP host: an app's registered commands ([`kitchen_shell::Registry`]) served to agents as
//! MCP tools. An agent calls the same command a menu item calls, with arguments checked against
//! its schema; every call is recorded ([`HostEvent::Called`]) for the session.
//!
//! The commands run on the app's UI thread, in its window; the MCP protocol runs on a tokio
//! runtime. A call crosses between them over a channel: [`McpServer`] (`Send`, the protocol side)
//! sends it, [`McpHost`] (a GPUI entity, the app side) runs it and answers.
//!
//! What an agent may run without asking is the [`Policy`]: everything, only the commands marked
//! read-only, or read-only ones freely and the rest after the person allows each call
//! ([`McpHost::pending`], [`McpHost::decide`]).
//!
//! Transports: [`serve_io`] over any byte stream, [`serve_stdio`], and [`McpHost::listen`] on a
//! local socket (an MCP client reaches it through the `kitchen-mcp-bridge` program, which joins
//! its stdin and stdout to the socket). No API key is involved anywhere.

use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::{AnyWindowHandle, AppContext as _, AsyncApp, Context, EventEmitter, WeakEntity};
use kitchen_shell::Registry;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData, Implementation, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, ServiceExt as _};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What an agent may run without the person.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    /// Every command.
    AllowAll,
    /// Only the commands marked read-only; the others are refused.
    ReadOnly,
    /// Read-only commands freely; each other call waits until the person allows or denies it.
    #[default]
    Ask,
}

impl Policy {
    /// As Settings says it.
    pub fn title(self) -> &'static str {
        match self {
            Policy::AllowAll => "Run any command",
            Policy::ReadOnly => "Only read",
            Policy::Ask => "Ask before changing anything",
        }
    }
}

/// A command as an MCP tool.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolInfo {
    /// The command id (the tool's name).
    pub name: String,
    /// Its title.
    pub title: String,
    /// What it does.
    pub description: String,
    /// The JSON Schema of its arguments (an empty object schema when it takes none).
    pub schema: Value,
    /// Whether it only reads.
    pub read_only: bool,
}

/// The tool name of the command `id`: what MCP clients accept (`^[a-zA-Z0-9_-]{1,64}$`, room left
/// for a client's own prefix such as `mcp_<server>_`): every other character becomes `_`
/// (`hist.find_cells` → `hist_find_cells`). Calls by either name reach the command.
pub fn tool_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).take(48).collect()
}

/// The tools of every command in `registry` (named by [`tool_name`]).
pub fn tools_of(registry: &Registry) -> Vec<ToolInfo> {
    registry
        .all()
        .into_iter()
        .map(|c| ToolInfo {
            name: tool_name(&c.id),
            title: c.title.to_string(),
            description: if c.hint.is_empty() { c.title.to_string() } else { format!("{}: {}", c.title, c.hint) },
            schema: c.args.clone().unwrap_or_else(|| json!({"type": "object", "properties": {}})),
            read_only: c.read_only,
        })
        .collect()
}

/// A call an agent made, and how it went: what the session records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// The command.
    pub name: String,
    /// Its arguments.
    pub arguments: Value,
    /// Its result, or why it did not run.
    pub result: Result<Value, String>,
    /// When (seconds since 1970).
    pub at: u64,
    /// Through MCP (`true`) or from the app's own chat.
    pub external: bool,
}

/// What the host tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    /// A call ran (or was refused).
    Called(CallRecord),
    /// A call waits for the person ([`McpHost::pending`]).
    Waiting,
}

type Reply = tokio::sync::oneshot::Sender<Result<Value, String>>;

/// A call crossing from the protocol to the app.
struct Call {
    name: String,
    arguments: Value,
    reply: Reply,
}

/// A call waiting for the person.
pub struct Pending {
    /// The command.
    pub name: String,
    /// Its title.
    pub title: String,
    /// Its arguments.
    pub arguments: Value,
    reply: Option<Reply>,
}

/// The protocol side: what an MCP transport serves. `Send` and cheap to clone.
#[derive(Clone)]
pub struct McpServer {
    name: String,
    instructions: String,
    tools: Arc<RwLock<Vec<ToolInfo>>>,
    calls: async_channel::Sender<Call>,
}

impl McpServer {
    fn tool_list(&self) -> Vec<Tool> {
        let tools = self.tools.read().unwrap_or_else(|e| e.into_inner());
        tools
            .iter()
            .map(|t| {
                let schema = match &t.schema {
                    Value::Object(m) => m.clone(),
                    _ => serde_json::Map::new(),
                };
                let mut tool = Tool::new(t.name.clone(), t.description.clone(), Arc::new(schema));
                tool.title = Some(t.title.clone());
                tool.annotations = Some(ToolAnnotations::new().read_only(t.read_only));
                tool
            })
            .collect()
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new(self.name.clone(), env!("CARGO_PKG_VERSION"))).with_instructions(self.instructions.clone())
    }

    async fn list_tools(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tool_list()))
    }

    async fn call_tool(&self, request: CallToolRequestParams, _: RequestContext<RoleServer>) -> Result<CallToolResponse, ErrorData> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        let arguments = request.arguments.map(Value::Object).unwrap_or(Value::Null);
        let call = Call { name: request.name.to_string(), arguments, reply };
        self.calls.send(call).await.map_err(|_| ErrorData::internal_error("the app is closed", None))?;
        let result = match answer.await {
            Ok(Ok(v)) => {
                let text = serde_json::to_string_pretty(&v).unwrap_or_default();
                let mut r = CallToolResult::success(vec![ContentBlock::text(text)]);
                r.structured_content = Some(v);
                r
            }
            Ok(Err(e)) => CallToolResult::error(vec![ContentBlock::text(e)]),
            Err(_) => CallToolResult::error(vec![ContentBlock::text("the app dropped the call")]),
        };
        Ok(result.into())
    }
}

/// Serves `server` over a byte stream until the client goes away.
pub async fn serve_io<R, W>(server: McpServer, read: R, write: W) -> Result<(), String>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
    W: tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let service = server.serve((read, write)).await.map_err(|e| e.to_string())?;
    service.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Serves `server` over this program's stdin and stdout (a headless app run by an agent).
pub async fn serve_stdio(server: McpServer) -> Result<(), String> {
    serve_io(server, tokio::io::stdin(), tokio::io::stdout()).await
}

/// The app side: runs the calls in the app's window, as the policy allows.
pub struct McpHost {
    registry: Registry,
    window: AnyWindowHandle,
    policy: Policy,
    server: McpServer,
    pending: Vec<Pending>,
    log: Vec<CallRecord>,
}

impl EventEmitter<HostEvent> for McpHost {}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl McpHost {
    /// A host named `name` (what clients see) over `registry`'s commands, run in `window`.
    pub fn new(name: &str, instructions: &str, registry: Registry, window: AnyWindowHandle, policy: Policy, cx: &mut Context<Self>) -> Self {
        let (tx, rx) = async_channel::unbounded::<Call>();
        let server = McpServer { name: name.into(), instructions: instructions.into(), tools: Arc::new(RwLock::new(tools_of(&registry))), calls: tx };
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while let Ok(call) = rx.recv().await {
                let Ok(()) = this.update(cx, |h, cx| h.receive(call, true, cx)) else { break };
            }
        })
        .detach();
        Self { registry, window, policy, server, pending: Vec::new(), log: Vec::new() }
    }

    /// The protocol side, for a transport.
    pub fn server(&self) -> McpServer {
        self.server.clone()
    }

    /// Reads the registry's commands again (after the app registered or removed some).
    pub fn refresh_tools(&self) {
        *self.server.tools.write().unwrap_or_else(|e| e.into_inner()) = tools_of(&self.registry);
    }

    /// The tools served.
    pub fn tools(&self) -> Vec<ToolInfo> {
        self.server.tools.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// What an agent may run without the person.
    pub fn policy(&self) -> Policy {
        self.policy
    }

    /// Sets what an agent may run without the person.
    pub fn set_policy(&mut self, policy: Policy, cx: &mut Context<Self>) {
        self.policy = policy;
        cx.notify();
    }

    /// The calls waiting for the person.
    pub fn pending(&self) -> &[Pending] {
        &self.pending
    }

    /// Every call so far (the session's record).
    pub fn log(&self) -> &[CallRecord] {
        &self.log
    }

    /// Calls `name` from the app's own chat (no transport), under the same policy and record;
    /// the answer comes to `reply`.
    pub fn call(&mut self, name: &str, arguments: Value, cx: &mut Context<Self>) -> tokio::sync::oneshot::Receiver<Result<Value, String>> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.receive(Call { name: name.into(), arguments, reply }, false, cx);
        answer
    }

    fn receive(&mut self, call: Call, external: bool, cx: &mut Context<Self>) {
        // By its command id, or by its tool name (`hist_find_cells` for `hist.find_cells`)
        let command = self.registry.get(&call.name).or_else(|| self.registry.all().into_iter().find(|c| tool_name(&c.id) == call.name));
        let Some(command) = command else {
            let e = format!("no command {:?} (list_tools names them)", call.name);
            return self.finish(call.name, call.arguments, Err(e), call.reply, external, cx);
        };
        let call = Call { name: command.id.to_string(), ..call };
        match (self.policy, command.read_only) {
            (Policy::AllowAll, _) | (_, true) => self.run(call.name, call.arguments, call.reply, external, cx),
            (Policy::ReadOnly, false) => {
                let e = format!("{} changes the project; this app lets agents only read. The person can allow it in Settings.", call.name);
                self.finish(call.name, call.arguments, Err(e), call.reply, external, cx)
            }
            (Policy::Ask, false) => {
                self.pending.push(Pending { name: call.name, title: command.title.to_string(), arguments: call.arguments, reply: Some(call.reply) });
                cx.emit(HostEvent::Waiting);
                cx.notify();
            }
        }
    }

    /// Allows (`true`) or denies the waiting call `index`.
    pub fn decide(&mut self, index: usize, allow: bool, cx: &mut Context<Self>) {
        if index >= self.pending.len() {
            return;
        }
        let mut p = self.pending.remove(index);
        let reply = p.reply.take();
        if let Some(reply) = reply {
            if allow {
                self.run(p.name, p.arguments, reply, true, cx);
            } else {
                self.finish(p.name.clone(), p.arguments, Err(format!("The person did not allow {}", p.name)), reply, true, cx);
            }
        }
        cx.notify();
    }

    fn run(&mut self, name: String, arguments: Value, reply: Reply, external: bool, cx: &mut Context<Self>) {
        // The command runs once this update is over, in the window (it may change this host's
        // owner, or open something)
        let (registry, window, me) = (self.registry.clone(), self.window, cx.entity().downgrade());
        cx.defer(move |cx| {
            let result = cx.update_window(window, |_, window, cx| registry.run_as(kitchen_shell::activity::Actor::Agent, "agent", &name, arguments.clone(), window, cx)).unwrap_or_else(|e| Err(e.to_string()));
            let _ = me.update(cx, |h, cx| h.finish(name, arguments, result, reply, external, cx));
        });
    }

    fn finish(&mut self, name: String, arguments: Value, result: Result<Value, String>, reply: Reply, external: bool, cx: &mut Context<Self>) {
        let record = CallRecord { name, arguments, result: result.clone(), at: now(), external };
        self.log.push(record.clone());
        let _ = reply.send(result);
        cx.emit(HostEvent::Called(record));
        cx.notify();
    }

    /// Serves the host on the local socket `path` (Unix) from a background thread, one MCP
    /// session per connection, until the app quits. Clients reach it with `kitchen-mcp-bridge
    /// <path>`.
    #[cfg(unix)]
    pub fn listen(&self, path: std::path::PathBuf) -> Result<(), String> {
        let _ = std::fs::remove_file(&path);
        let server = self.server();
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())?;
        let listener = runtime.block_on(async { tokio::net::UnixListener::bind(&path) }).map_err(|e| format!("{}: {e}", path.display()))?;
        std::thread::Builder::new()
            .name("kitchen-mcp".into())
            .spawn(move || {
                runtime.block_on(async move {
                    while let Ok((stream, _)) = listener.accept().await {
                        let server = server.clone();
                        tokio::spawn(async move {
                            let (r, w) = stream.into_split();
                            let _ = serve_io(server, r, w).await;
                        });
                    }
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Where an app's MCP socket is by default: `$XDG_RUNTIME_DIR/<app>-mcp.sock`, else the temp
/// folder.
pub fn default_socket(app: &str) -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join(format!("{app}-mcp.sock"))
}
