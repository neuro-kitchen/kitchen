//! The agent gallery: a small app in the shared frame with an Agents workspace, a scripted agent
//! (no model, no key: it matches words) and the MCP host.

#![allow(dead_code)]

use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, WindowOptions};
use kitchen_agent::workspace::{self, AgentWorkspace};
use kitchen_agent::{AgentEvent, McpHost, Policy};
use kitchen_shell::{ChangesPanel, CommandSpec, MenuItem, MenuSpec, Registry, Shell, ShellSpec, WorkspaceSpec};
use kitchen_ui::chat::{ChatContext, ChatMessage, Location};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::project;

#[derive(Deserialize, JsonSchema)]
pub struct BlurArgs {
    /// The blur's standard deviation in pixels.
    pub sigma: f64,
}

/// The demo's commands: one that reads, one that changes the project.
pub fn commands(dir: PathBuf, changes: &Entity<ChangesPanel>) -> Vec<CommandSpec> {
    let ch = changes.clone();
    vec![
        CommandSpec::new("image.describe", "Describe the image", IconName::Info, |_, _| {})
            .group("Image")
            .hint("Size, channels and pixel size of the shown image")
            .read_only(),
        CommandSpec::with_args::<BlurArgs>("pipeline.set_blur", "Set the blur", IconName::Droplet, move |a, _, cx| {
            let path = dir.join("pipeline.json");
            let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            v["nodes"][0]["sigma"] = json!(a.sigma);
            std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
            ch.update(cx, |p, cx| p.refresh(cx));
            Ok(json!({ "sigma": a.sigma }))
        })
        .group("Pipeline")
        .hint("Sets the Gaussian blur of the pipeline's first node"),
    ]
}

/// The gallery's parts, for tests.
pub struct Gallery {
    pub shell: Entity<Shell>,
    pub host: Entity<McpHost>,
    pub agents: Entity<AgentWorkspace>,
    pub changes: Entity<ChangesPanel>,
    pub registry: Registry,
    pub project: PathBuf,
}

/// The scripted agent: it answers a prompt by running the command it names.
fn answer(agents: &Entity<AgentWorkspace>, host: &Entity<McpHost>, prompt: &str, cx: &mut App) {
    let p = prompt.to_lowercase();
    let chat = agents.read(cx).chat().clone();
    let call = if p.contains("blur") {
        let sigma = p.split_whitespace().filter_map(|w| w.parse::<f64>().ok()).next().unwrap_or(1.5);
        Some(("pipeline.set_blur", json!({ "sigma": sigma })))
    } else if p.contains("describe") || p.contains("image") {
        Some(("image.describe", Value::Null))
    } else {
        None
    };
    match call {
        Some((name, args)) => {
            let rx = host.update(cx, |h, cx| h.call(name, args, cx));
            chat.update(cx, |c, cx| c.append_agent(&format!("Running `{name}`…"), cx));
            let chat2 = chat.clone();
            cx.spawn(async move |cx| {
                let r = rx.await.unwrap_or_else(|_| Err("dropped".into()));
                let text = match r {
                    Ok(v) => format!("Done: {v}"),
                    Err(e) => format!("It did not run: {e}"),
                };
                let _ = chat2.update(cx, |c, cx| {
                    c.push(ChatMessage::Agent { text }, cx);
                    c.set_running(false, cx);
                });
            })
            .detach();
        }
        None => chat.update(cx, |c, cx| {
            c.push(ChatMessage::Agent { text: "I can **describe the image** or **set the blur** (say a sigma, such as `blur 2.5`).".into() }, cx);
            c.set_running(false, cx);
        }),
    }
}

/// Opens the gallery in a window (`socket`: also serve MCP on it).
pub fn open(options: WindowOptions, socket: Option<PathBuf>, policy: Policy, cx: &mut App) -> (AnyWindowHandle, Gallery) {
    let mut parts = None;
    let (window, shell) = gpui_kit::open_window(options, cx, |window, cx| {
        let dir = std::env::temp_dir().join(format!("kitchen-agent-gallery-{}", std::process::id()));
        let changes = project::panel(&dir, cx);
        let registry = Registry::new(commands(dir.clone(), &changes));
        let handle = window.window_handle();
        let host = cx.new(|cx| McpHost::new("kitchen-agent gallery", "A demo app: describe_image reads, pipeline.set_blur changes the project.", registry.clone(), handle, policy, cx));
        let context = ChatContext { workspace: "Demo slide".into(), location: Location::Local, model: "scripted (no model)".into() };
        let agents = cx.new(|cx| AgentWorkspace::new(dir.join("sessions"), dir.clone(), host.clone(), changes.clone(), context, window, cx));
        let (a, h) = (agents.clone(), host.clone());
        cx.subscribe(&agents, move |_, e: &AgentEvent, cx| {
            if let AgentEvent::Prompt(p) = e {
                answer(&a, &h, p, cx);
            }
        })
        .detach();
        let workspaces = vec![
            WorkspaceSpec::new("agents", "Agents", IconName::Bot, agents.clone()).hint("Sessions, the conversation and what it changed"),
            WorkspaceSpec::new("changes", "Changes", IconName::GitCompare, changes.clone()).menus(vec![MenuSpec::new("Pipeline", vec![MenuItem::command("pipeline.set_blur")])]),
        ];
        let mut spec = ShellSpec::new("Agents", registry.clone(), workspaces);
        spec.meters = false;
        spec.settings = vec![workspace::settings_section(host.clone(), socket.clone())];
        let shell = cx.new(|cx| Shell::new(spec, window, cx));
        parts = Some((host, agents, changes, registry, dir));
        shell
    })
    .expect("failed to open the gallery window");
    let (host, agents, changes, registry, project) = parts.expect("built");
    registry.add(workspace::open_command(&shell, "agents"));
    host.read(cx).refresh_tools();
    let _ = registry.bind_keys(None, cx);
    #[cfg(unix)]
    if let Some(path) = socket {
        if let Err(e) = host.read(cx).listen(path) {
            eprintln!("MCP socket: {e}");
        }
    }
    (window, Gallery { shell, host, agents, changes, registry, project })
}
