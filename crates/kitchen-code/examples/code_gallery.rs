//! The Code workspace on its own: a folder with a few scripts, the workspace over it in the shared
//! frame, its commands in the registry. `cargo run -p kitchen-code --example code-gallery`; the
//! tests open the same window.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, Window, WindowOptions};
use kitchen_code::{CodeEvent, CodeWorkspace};
use kitchen_shell::{Registry, Shell, ShellSpec, WorkspaceSpec};
use serde_json::json;

pub const COUNT: &str = r#"# kitchen: command = Count cells
# kitchen: hint = Counts the cells of the shown image above a minimum area
# kitchen: args = {"type": "object", "properties": {"min_area": {"type": "number", "description": "Smallest cell in µm²"}}}
# kitchen: node = in image:image, mask:mask; out counts:table
import json, sys

input = json.load(sys.stdin)
args = input.get("args") or {}
min_area = args.get("min_area", 20)
print(f"Counting on {input['context']['image']} (min area {min_area})")
print(json.dumps({"image": input["context"]["image"], "cells": 2712 if min_area <= 20 else 2539}))
"#;

pub const BROKEN: &str = r#"# kitchen: command = Broken script
import json, sys

def main():
    return 1 / 0

main()
"#;

pub const SIGNAL: &str = r#"// kitchen: node = in trace:signal; out spikes:events
// A node of another app's types: listed with the reason
fn main() {}
"#;

/// Writes the demo scripts into `dir`.
pub fn write(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for (name, text) in [("count_cells.py", COUNT), ("broken.py", BROKEN), ("spikes.rs", SIGNAL)] {
        std::fs::write(dir.join(name), text).unwrap();
    }
}

/// The folder for the demo scripts (unique per process).
pub fn dir() -> PathBuf {
    std::env::temp_dir().join(format!("kitchen-code-scripts-{}", std::process::id()))
}

/// The Code workspace over the demo scripts, its commands registered (and kept in step).
pub fn workspace(registry: &Registry, window: &mut Window, cx: &mut App) -> Entity<CodeWorkspace> {
    let dir = dir();
    write(&dir);
    let types = ["image", "mask", "labels", "table"].map(String::from).to_vec();
    let ws = cx.new(|cx| CodeWorkspace::new(dir, types, |_| json!({"image": "slide-01.ome.zarr", "selection": null}), window, cx));
    let r = registry.clone();
    let weak = ws.downgrade();
    ws.update(cx, |w, _| w.register(&r, weak.clone()));
    let r = registry.clone();
    cx.subscribe(&ws, move |ws, e: &CodeEvent, cx| {
        if matches!(e, CodeEvent::ScriptsChanged) {
            let weak = ws.downgrade();
            ws.update(cx, |w, _| w.register(&r, weak));
        }
    })
    .detach();
    ws
}

/// The workspace in the shared frame, in a window: the shell, the workspace, the registry.
pub fn open(options: WindowOptions, cx: &mut App) -> (AnyWindowHandle, Entity<Shell>, Entity<CodeWorkspace>, Registry) {
    let registry = Registry::new(Vec::new());
    let mut ws = None;
    let (window, shell) = gpui_kit::open_window(options, cx, |window, cx| {
        let code = workspace(&registry, window, cx);
        let spec = ShellSpec::new("Code", registry.clone(), vec![WorkspaceSpec::new("code", "Code", IconName::SquareTerminal, code.clone()).hint("Scripts as commands and pipeline steps")]);
        ws = Some(code);
        cx.new(|cx| Shell::new(spec, window, cx))
    })
    .expect("failed to open the window");
    (window, shell, ws.expect("the window was built"), registry)
}

fn main() {
    use gpui_kit::{Bounds, TitlebarOptions, WindowBounds, px, size};
    use kitchen_ui::icons::AppAssets;
    use kitchen_ui::theme::{self, Look, Palette};

    let light = std::env::args().any(|a| a == "--light");
    gpui_kit::application().with_assets(AppAssets::new().all_icons()).run(move |cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::LIME).with_dark(!light), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("kitchen-code gallery".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1180.), px(760.)), cx))),
            ..Default::default()
        };
        kitchen_code::keys::bind_keys(cx);
        let (_, _, _, registry) = open(options, cx);
        for b in registry.bind_keys(None, cx) {
            eprintln!("key not bound: {b}");
        }
        cx.activate(true);
    });
}
