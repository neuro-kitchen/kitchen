//! Headless UI tests of the Scripts workspace: scripts found with what their headers say, a
//! script run as a command (Python), its errors on their lines, nodes checked against the app's
//! port types.

mod common;

use std::time::Duration;

use common::{frame, open};
use gpui_kit::AppContext as _;
use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
use serde_json::{Value, json};

fn python() -> bool {
    std::process::Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn wait_done(cx: &mut TestAppContext, window: gpui_kit::AnyWindowHandle, ws: &gpui_kit::Entity<kitchen_shell::ScriptsWorkspace>) {
    for _ in 0..1000 {
        frame(cx, window);
        if !cx.update(|cx| ws.read(cx).is_running()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the script did not finish");
}

#[gpui_kit::test]
fn scripts_become_commands_and_nodes(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (window, g) = open(cx);
    cx.update(|cx| g.shell.update(cx, |s, cx| assert!(s.set_workspace("scripts", cx))));
    frame(cx, window);
    let ws = g.scripts.clone();
    let names: Vec<String> = cx.update(|cx| ws.read(cx).scripts().iter().map(|s| s.name()).collect());
    assert_eq!(names, vec!["broken", "count_cells", "spikes"]);

    // Nodes: the image one is a node; the signal one names types this app does not have
    let (nodes, bad) = cx.update(|cx| ws.read(cx).nodes());
    assert_eq!(nodes.len(), 1);
    assert_eq!((nodes[0].id.as_str(), nodes[0].inputs.len(), nodes[0].outputs[0].ty.as_str()), ("script.count_cells", 2, "table"));
    assert_eq!(bad.len(), 1);
    assert!(bad[0].contains("signal") && bad[0].contains("image, mask, labels, table"), "{}", bad[0]);

    // Command scripts are in the registry, with their schema, in the search
    let c = g.registry.get("script.count_cells").expect("registered");
    assert_eq!(c.args.as_ref().unwrap()["properties"]["min_area"]["type"], "number");
    assert!(g.registry.get("script.spikes").is_none(), "not a command");

    if !python() {
        eprintln!("python3 not found: running scripts is not tested");
        return;
    }
    // Run as a command, with arguments; the context is what the app says is shown
    let r = cx.update_window(window, |_, window, cx| g.registry.run("script.count_cells", json!({"min_area": 50}), window, cx)).unwrap();
    assert_eq!(r, Ok(json!({"started": "script.count_cells"})));
    wait_done(cx, window, &ws);
    let out = cx.update(|cx| ws.read(cx).output().cloned()).expect("output");
    assert!(out.ok, "{out:?}");
    assert_eq!(out.result, Some(json!({"image": "slide-01.ome.zarr", "cells": 2539})));
    assert!(out.stdout.contains("min area 50"));

    // The broken one: its error on its line in the editor
    let broken = cx.update(|cx| ws.read(cx).scripts().iter().position(|s| s.name() == "broken")).unwrap();
    cx.update_window(window, |_, window, cx| ws.update(cx, |w, cx| w.select(broken, window, cx))).unwrap();
    cx.update_window(window, |_, window, cx| window.click("scripts-run", cx)).unwrap();
    wait_done(cx, window, &ws);
    let out = cx.update(|cx| ws.read(cx).output().cloned()).unwrap();
    assert!(!out.ok);
    assert_eq!(out.errors, vec![(5, "ZeroDivisionError: division by zero".to_string())]);
    let marked = cx.update(|cx| ws.read(cx).editor().unwrap().read(cx).error_list().to_vec());
    assert_eq!(marked, out.errors);

    // A new script from the template runs too, and registers its command once saved
    cx.update_window(window, |_, window, cx| ws.update(cx, |w, cx| w.new_script(kitchen_ui::widgets::CodeLanguage::Python, window, cx))).unwrap().unwrap();
    frame(cx, window);
    assert!(g.registry.get("script.script_1").is_some());
    cx.update(|cx| ws.update(cx, |w, cx| w.run_selected(Value::Null, cx))).unwrap();
    wait_done(cx, window, &ws);
    assert_eq!(cx.update(|cx| ws.read(cx).output().and_then(|o| o.result.clone())), Some(json!({"ok": true})));
    let _ = std::fs::remove_dir_all(common::scripts::dir());
}
