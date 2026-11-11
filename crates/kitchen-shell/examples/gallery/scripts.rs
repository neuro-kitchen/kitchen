//! The gallery's scripts: a folder with a few, the Scripts workspace over it, its commands in
//! the registry.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gpui_kit::{App, AppContext as _, Entity, Window};
use kitchen_shell::{Registry, ScriptsEvent, ScriptsWorkspace};
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
    std::env::temp_dir().join(format!("kitchen-shell-scripts-{}", std::process::id()))
}

/// The Scripts workspace over the demo scripts, its commands registered (and kept in step).
pub fn workspace(registry: &Registry, window: &mut Window, cx: &mut App) -> Entity<ScriptsWorkspace> {
    let dir = dir();
    write(&dir);
    let types = ["image", "mask", "labels", "table"].map(String::from).to_vec();
    let ws = cx.new(|cx| ScriptsWorkspace::new(dir, types, |_| json!({"image": "slide-01.ome.zarr", "selection": null}), window, cx));
    let r = registry.clone();
    let weak = ws.downgrade();
    ws.update(cx, |w, _| w.register(&r, weak.clone()));
    let r = registry.clone();
    cx.subscribe(&ws, move |ws, e: &ScriptsEvent, cx| {
        if matches!(e, ScriptsEvent::ScriptsChanged) {
            let weak = ws.downgrade();
            ws.update(cx, |w, _| w.register(&r, weak));
        }
    })
    .detach();
    ws
}
