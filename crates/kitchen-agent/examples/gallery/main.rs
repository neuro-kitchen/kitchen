//! Opens the agent workspace on a small demo app, with a scripted agent (no model, no key) and the
//! MCP host on a local socket: an MCP client connects with `kitchen-mcp-bridge <socket>` (the
//! path is in Settings ▸ Agents).
//!
//! `cargo run -p kitchen-agent --example agent-gallery`

mod app;
#[path = "../../../kitchen-shell/examples/gallery/project.rs"]
mod project;

use gpui_kit::{Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use kitchen_agent::Policy;
use kitchen_ui::icons::AppAssets;
use kitchen_ui::theme::{self, Look, Palette};

fn main() {
    gpui_kit::application().with_assets(AppAssets::new().all_icons()).run(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::EOSIN), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("kitchen-agent gallery".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1360.), px(820.)), cx))),
            ..Default::default()
        };
        let socket = kitchen_agent::host::default_socket("kitchen-agent-gallery");
        eprintln!("MCP: kitchen-mcp-bridge {}", socket.display());
        app::open(options, Some(socket), Policy::Ask, cx);
        cx.activate(true);
    });
}
