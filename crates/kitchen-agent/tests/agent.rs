//! The agent workspace and the MCP host, headless: an MCP client (over an in-memory stream) lists
//! the app's commands as tools and calls them under each policy; a call waiting for the person
//! is allowed from the workspace; the scripted chat agent runs a command; sessions are saved.

#[path = "../examples/gallery/app.rs"]
mod app;
#[path = "../../kitchen-shell/examples/gallery/project.rs"]
mod project;

use std::sync::mpsc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_agent::Policy;
use kitchen_ui::chat::ChatMessage;
use kitchen_ui::theme::{self, Look, Palette};
use rmcp::ServiceExt as _;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};

fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

/// Waits (real time) until `done`, drawing frames meanwhile.
fn wait_until<T>(cx: &mut TestAppContext, window: AnyWindowHandle, what: &str, mut done: impl FnMut(&mut TestAppContext) -> Option<T>) -> T {
    for _ in 0..1000 {
        frame(cx, window);
        if let Some(v) = done(cx) {
            return v;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

/// What the client saw: tool names with their read-only hint, then each call's text and error flag.
enum Seen {
    Tools(Vec<(String, bool)>),
    Call(String, bool),
}

#[gpui_kit::test]
fn agents_call_commands_as_tools_under_the_policy(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (window, g) = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::EOSIN), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1360.), px(820.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        app::open(options, None, Policy::Ask, cx)
    });
    frame(cx, window);

    // An MCP client on its own runtime, talking to the host over an in-memory stream
    let server = cx.update(|cx| g.host.read(cx).server());
    let (tx, rx) = mpsc::channel::<Seen>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let (client_io, server_io) = tokio::io::duplex(1 << 16);
            let (sr, sw) = tokio::io::split(server_io);
            tokio::spawn(kitchen_agent::serve_io(server, sr, sw));
            let client = ().serve(client_io).await.expect("client connects");
            let tools = client.list_all_tools().await.unwrap();
            tx.send(Seen::Tools(tools.iter().map(|t| (t.name.to_string(), t.annotations.as_ref().and_then(|a| a.read_only_hint).unwrap_or(false))).collect())).unwrap();
            let call = |name: &str, args: Value| {
                let mut p = CallToolRequestParams::new(name.to_string());
                if let Value::Object(m) = args {
                    p = p.with_arguments(m);
                }
                p
            };
            // By tool name (what clients send) and by command id: both reach the command
            for (name, args) in [("image_describe", Value::Null), ("pipeline.set_blur", json!({"sigma": 3.0})), ("pipeline.set_blur", json!({"sigma": 1.0})), ("pipeline.set_blur", json!({"sigma": "big"}))] {
                // One call at a time: the test changes the policy between them
                go_rx.recv().unwrap();
                let r = client.call_tool(call(name, args)).await.unwrap();
                let text = r.content.iter().filter_map(|c| c.as_text().map(|t| t.text.clone())).collect::<Vec<_>>().join("");
                tx.send(Seen::Call(text, r.is_error.unwrap_or(false))).unwrap();
            }
            let _ = client.cancel().await;
        });
    });
    let next = |cx: &mut TestAppContext| wait_until(cx, window, "the client", |_| rx.try_recv().ok());

    // Every command is a tool, the read-only hint carried over (the frame's own commands too)
    let Seen::Tools(tools) = next(cx) else { panic!("tools first") };
    assert!(tools.contains(&("image_describe".into(), true)));
    assert!(tools.contains(&("pipeline_set_blur".into(), false)));
    assert!(tools.iter().any(|(n, _)| n == "shell_search") && tools.iter().any(|(n, _)| n == "agent_open"));
    // Every name is one MCP clients accept (`^[a-zA-Z0-9_-]{1,64}$`, with room for a prefix)
    for (n, _) in &tools {
        assert!(!n.is_empty() && n.len() <= 48 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), "tool name {n:?}");
    }

    // A read-only command runs at once under "Ask"
    go_tx.send(()).unwrap();
    let Seen::Call(_, err) = next(cx) else { panic!() };
    assert!(!err);

    // A command that changes the project waits for the person, who allows it in the workspace
    cx.update(|cx| g.shell.update(cx, |s, cx| s.set_workspace("agents", cx)));
    go_tx.send(()).unwrap();
    wait_until(cx, window, "a waiting call", |cx| (cx.update(|cx| g.host.read(cx).pending().len()) == 1).then_some(()));
    cx.update_window(window, |_, window, cx| window.click("agent-allow-0", cx)).unwrap();
    let Seen::Call(text, err) = next(cx) else { panic!() };
    assert!(!err && text.contains("3.0"), "{text}");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(g.project.join("pipeline.json")).unwrap()).unwrap();
    assert_eq!(written["nodes"][0]["sigma"], 3.0);
    assert!(cx.update(|cx| g.changes.read(cx).changed_lines()) > 0, "the change shows in Changes");

    // "Only read": refused, with the reason
    cx.update(|cx| g.host.update(cx, |h, cx| h.set_policy(Policy::ReadOnly, cx)));
    go_tx.send(()).unwrap();
    let Seen::Call(text, err) = next(cx) else { panic!() };
    assert!(err && text.contains("only read"), "{text}");

    // Arguments are checked against the schema
    cx.update(|cx| g.host.update(cx, |h, cx| h.set_policy(Policy::AllowAll, cx)));
    go_tx.send(()).unwrap();
    let Seen::Call(text, err) = next(cx) else { panic!() };
    assert!(err && text.starts_with("arguments:"), "{text}");

    // Every call is in the record and in the conversation, and the session is saved
    let log = cx.update(|cx| g.host.read(cx).log().to_vec());
    assert_eq!(log.len(), 4);
    assert!(log.iter().all(|r| r.external));
    let tools_in_chat = cx.update(|cx| g.agents.read(cx).chat().read(cx).messages().iter().filter(|m| matches!(m, ChatMessage::Tool(_))).count());
    assert_eq!(tools_in_chat, 4);
    assert_eq!(cx.update(|cx| g.agents.read(cx).sessions().len()), 1);

    // The scripted chat agent: a prompt runs a command through the same host
    let chat = cx.update(|cx| g.agents.read(cx).chat().clone());
    cx.update_window(window, |_, window, cx| chat.update(cx, |c, cx| {
        c.set_prompt("set the blur to 2.5", window, cx);
        c.send(window, cx);
    }))
    .unwrap();
    wait_until(cx, window, "the agent's answer", |cx| cx.update(|cx| (!chat.read(cx).is_running()).then_some(())));
    let written: Value = serde_json::from_str(&std::fs::read_to_string(g.project.join("pipeline.json")).unwrap()).unwrap();
    assert_eq!(written["nodes"][0]["sigma"], 2.5);
    let session = cx.update(|cx| g.agents.read(cx).session().clone());
    assert_eq!(session.title, "set the blur to 2.5");
    assert_eq!(session.calls.len(), 5);

    // A new session, then the first one again
    cx.update(|cx| g.agents.update(cx, |a, cx| a.new_session(cx)));
    assert!(cx.update(|cx| g.agents.read(cx).chat().read(cx).messages().is_empty()));
    let first = cx.update(|cx| g.agents.read(cx).sessions()[0].0.clone());
    cx.update(|cx| g.agents.update(cx, |a, cx| a.open_session(&first, cx))).unwrap();
    assert_eq!(cx.update(|cx| g.agents.read(cx).session().calls.len()), 5);

    // Both themes draw it; Settings shows the Agents section
    for dark in [false, true] {
        cx.update(|cx| theme::use_look(theme::look().with_dark(dark), cx));
        frame(cx, window);
    }
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.show_settings(Some(1), window, cx))).unwrap();
    frame(cx, window);
    let _ = std::fs::remove_dir_all(&g.project);
}
