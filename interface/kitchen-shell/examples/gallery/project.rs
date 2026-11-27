//! The gallery's demo project: three files in a temporary folder, the Changes panel watching
//! them, and commands that change them as an app and an agent would.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName;
use gpui_kit::{App, AppContext as _, Entity};
use kitchen_shell::{ChangesPanel, CommandSpec, MenuItem, MenuSpec};
use serde_json::{Value, json};

/// Writes the demo project into `dir`; returns its three files.
pub fn write(dir: &Path) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).unwrap();
    let cells: Vec<u32> = (0..2539).collect();
    let files = [
        ("session.json", json!({"version": 1, "steps": [{"action": "tissue", "settings": {"threshold": 0.2}}]})),
        ("pipeline.json", json!({"nodes": [
            {"id": "blur", "op": "gaussian", "sigma": 2.0},
            {"id": "mask", "op": "threshold", "level": 0.5},
            {"id": "fill", "op": "fill_holes", "max_area": 64},
            {"id": "label", "op": "connected_components", "connectivity": 8}
        ], "edges": [["blur", "mask"], ["mask", "fill"], ["fill", "label"]]})),
        ("project.json", json!({"name": "Demo slide", "images": ["slide-01.ome.zarr"], "classes": ["Tumor", "Stroma"], "cells": cells})),
    ];
    files
        .iter()
        .map(|(name, v)| {
            let p = dir.join(name);
            std::fs::write(&p, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
            p
        })
        .collect()
}

/// A folder for the demo project (unique per process).
pub fn dir() -> PathBuf {
    std::env::temp_dir().join(format!("kitchen-shell-gallery-{}", std::process::id()))
}

fn edit_json(path: &Path, f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default()).unwrap_or(Value::Null);
    f(&mut v);
    let _ = std::fs::write(path, serde_json::to_string_pretty(&v).unwrap_or_default() + "\n");
}

/// The commands that change the demo project.
pub fn commands(dir: PathBuf, panel: &Entity<ChangesPanel>) -> Vec<CommandSpec> {
    let (d1, d2) = (dir.clone(), dir.clone());
    let (p1, p2, p3, p4, p5) = (panel.clone(), panel.clone(), panel.clone(), panel.clone(), panel.clone());
    vec![
        CommandSpec::new("project.run_step", "Run a step", IconName::Play, move |_, cx| {
            edit_json(&d1.join("session.json"), |v| v["steps"].as_array_mut().into_iter().for_each(|s| s.push(json!({"action": "cells", "settings": {"model": "stardist"}}))));
            edit_json(&d1.join("project.json"), |v| v["cells"] = json!((0..2712).collect::<Vec<u32>>()));
            p1.update(cx, |p, cx| p.refresh(cx));
        })
        .group("Project")
        .hint("Adds a step to session.json and finds more cells (watch Changes)"),
        CommandSpec::new("project.propose", "Agent: propose a change", IconName::Bot, move |_, cx| {
            let path = d2.join("pipeline.json");
            let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default()).unwrap_or(Value::Null);
            v["nodes"][0]["sigma"] = json!(1.5);
            v["nodes"].as_array_mut().into_iter().for_each(|n| n.push(json!({"id": "open", "op": "opening", "radius": 2})));
            v["edges"].as_array_mut().into_iter().for_each(|e| e.push(json!(["label", "open"])));
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            p2.update(cx, |p, cx| {
                let _ = p.propose("pipeline.json", &text, cx);
            });
        })
        .group("Project")
        .hint("Shows an agent's change to pipeline.json, to accept or reject per hunk"),
        CommandSpec::new("project.mark_saved", "Mark as saved", IconName::Save, move |_, cx| p3.update(cx, |p, cx| p.mark_saved(cx)))
            .group("Project")
            .hint("The files as they are now are the last save"),
        CommandSpec::new("project.start_session", "Start an agent session", IconName::Bot, move |_, cx| p4.update(cx, |p, cx| p.start_session(cx)))
            .group("Project")
            .hint("Compare against now: everything the session does"),
        CommandSpec::new("project.changes_count", "Count changed lines", IconName::GitCompare, move |_, cx| {
            let n = p5.read(cx).changed_lines();
            eprintln!("{n} changed lines");
        })
        .group("Project")
        .hint("The badge's number")
        .read_only(),
    ]
}

/// The Changes workspace's menus.
pub fn menus() -> Vec<MenuSpec> {
    vec![MenuSpec::new(
        "Project",
        vec![MenuItem::command("project.run_step"), MenuItem::command("project.propose"), MenuItem::Separator, MenuItem::command("project.mark_saved"), MenuItem::command("project.start_session")],
    )]
}

/// The panel over the demo project in `dir` (written first).
pub fn panel(dir: &Path, cx: &mut App) -> Entity<ChangesPanel> {
    let files = write(dir);
    cx.new(|cx| ChangesPanel::new(files, cx))
}
