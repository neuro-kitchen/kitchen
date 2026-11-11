//! Headless UI tests of the command registry: menus, search and keys from one list; commands that
//! cannot run say why; arguments are checked.

mod common;

use common::{frame, open};
use gpui_kit::AppContext as _;
use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
use serde_json::{Value, json};

fn log(cx: &mut TestAppContext, g: &common::app::Gallery) -> Vec<String> {
    cx.update(|cx| g.state.read(cx).0.log.clone())
}

#[gpui_kit::test]
fn commands_run_through_the_registry_and_say_why_not(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    frame(cx, window);
    let registry = g.registry.clone();

    // Not enabled: refused with the reason
    let r = cx.update_window(window, |_, window, cx| registry.run("file.save", Value::Null, window, cx)).unwrap();
    assert_eq!(r, Err("Nothing changed since the last save".to_string()));
    // Arguments are checked against the command's type
    let bad = cx.update_window(window, |_, window, cx| registry.run("process.threshold", json!({"level": "high"}), window, cx)).unwrap();
    assert!(bad.unwrap_err().starts_with("arguments:"));
    let missing = cx.update_window(window, |_, window, cx| registry.run("process.threshold", Value::Null, window, cx)).unwrap();
    assert!(missing.is_err(), "level is required");
    let ok = cx.update_window(window, |_, window, cx| registry.run("process.threshold", json!({"level": 0.25}), window, cx)).unwrap();
    assert_eq!(ok, Ok(json!({"kept_fraction": 0.75})));
    // All-optional arguments run with none
    let blur = cx.update_window(window, |_, window, cx| registry.run("process.blur", Value::Null, window, cx)).unwrap();
    assert_eq!(blur, Ok(json!({"sigma": 2.0})));
    assert!(cx.update_window(window, |_, window, cx| registry.run("no.such", Value::Null, window, cx)).unwrap().is_err());
    // Now something changed: saving runs
    cx.update(|cx| assert!(registry.check("file.save", cx).is_ok()));
    // The frame registered its own commands, one per workspace too
    for id in ["shell.search", "shell.settings", "shell.theme", "shell.workspace.commands", "shell.workspace.views"] {
        assert!(registry.get(id).is_some(), "{id} is registered");
    }
    frame(cx, window);
}

#[gpui_kit::test]
fn keys_and_the_search_run_the_same_commands(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    frame(cx, window);
    // A key from the registry runs its command
    cx.update_window(window, |_, window, cx| window.press("ctrl-o", cx)).unwrap();
    frame(cx, window);
    assert_eq!(log(cx, &g), vec!["Opened sample.ome.zarr".to_string()]);

    // Ctrl+K is the frame's command: it opens the search
    cx.update_window(window, |_, window, cx| window.press("ctrl-k", cx)).unwrap();
    frame(cx, window);
    let search = cx.update(|cx| g.shell.read(cx).search()).expect("the search opened");

    // Typing filters by title, group and hint; a disabled command stays, with its reason
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("undo", window, cx))).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].command.id.as_ref(), "edit.undo");
    assert_eq!(rows[0].disabled, None, "the log has a line, so undo runs");
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("go to", window, cx))).unwrap();
    assert_eq!(cx.update(|cx| search.read(cx).rows(cx)).len(), 4, "one per workspace");

    // Enter runs the highlighted row; the search closes
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("blur", window, cx))).unwrap();
    frame(cx, window);
    cx.update_window(window, |_, window, cx| window.press("enter", cx)).unwrap();
    frame(cx, window);
    assert!(cx.update(|cx| g.shell.read(cx).search().is_none()), "the search closed");
    assert_eq!(log(cx, &g).last().map(String::as_str), Some("Blur, sigma 2"));

    // A disabled command named by search stays disabled with its reason
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.open_search(window, cx))).unwrap();
    let search = cx.update(|cx| g.shell.read(cx).search()).unwrap();
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("nothing", window, cx))).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    assert!(rows.is_empty() || rows.iter().all(|r| r.disabled.is_some()));
    frame(cx, window);
}
