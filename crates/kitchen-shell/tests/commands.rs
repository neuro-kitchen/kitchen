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

    // A search over one kind of command (Process ▸ Find an operation…)
    cx.update_window(window, |_, window, cx| {
        g.shell.update(cx, |s, cx| {
            let f: kitchen_shell::search::Filter = std::rc::Rc::new(|c: &kitchen_shell::CommandSpec| c.group.as_ref() == "Process");
            s.open_search_with(Some(("Find an operation".into(), f)), window, cx)
        })
    })
    .unwrap();
    let search = cx.update(|cx| g.shell.read(cx).search()).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    assert!(!rows.is_empty() && rows.iter().all(|r| r.command.group.as_ref() == "Process"));
    // Asked again while open: it closes
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.open_search_with(None, window, cx))).unwrap();
    assert!(cx.update(|cx| g.shell.read(cx).search().is_none()));

    // A disabled command named by search stays disabled with its reason
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.open_search(window, cx))).unwrap();
    let search = cx.update(|cx| g.shell.read(cx).search()).unwrap();
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("nothing", window, cx))).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    assert!(rows.is_empty() || rows.iter().all(|r| r.disabled.is_some()));
    frame(cx, window);
}

#[gpui_kit::test]
fn menus_run_commands_with_arguments_and_providers_fill_them(cx: &mut TestAppContext) {
    use kitchen_shell::{MenuItem, MenuSpec};
    let (window, g) = open(cx);
    frame(cx, window);
    let registry = g.registry.clone();

    // A provider makes entries from what the app holds when the menu opens: here one blur per
    // sigma, each a run of the same command with its own arguments
    registry.provide("demo.blurs", |_| [1.0, 2.0, 4.0].iter().map(|s| MenuItem::run(format!("Blur σ {s}"), "process.blur", json!({"sigma": s})).checked(*s == 2.0)).collect());
    let items = cx.update(|cx| registry.provided("demo.blurs", cx));
    assert_eq!(items.len(), 3);
    let MenuItem::Run { title, command, args, checked, .. } = &items[1] else { panic!("a run entry") };
    assert_eq!((title.as_ref(), command.as_ref(), checked), ("Blur σ 2", "process.blur", &true));
    // The entry's arguments go to the command
    let r = cx.update_window(window, |_, window, cx| registry.run(command, args.clone(), window, cx)).unwrap();
    assert_eq!(r, Ok(json!({"sigma": 2.0})));
    assert!(cx.update(|cx| registry.provided("no.such", cx)).is_empty());
    // A provider registered again replaces the first
    registry.provide("demo.blurs", |_| vec![MenuItem::label("none")]);
    assert_eq!(cx.update(|cx| registry.provided("demo.blurs", cx)), vec![MenuItem::label("none")]);

    // A workspace's menu row can change; run entries count as the command they name
    let menu = MenuSpec::new("Smooth", vec![MenuItem::provided("demo.blurs"), MenuItem::run("Blur σ 8", "process.blur", json!({"sigma": 8.0}))]);
    assert_eq!(menu.command_ids(), vec!["process.blur".into()] as Vec<gpui_kit::SharedString>);
    let id = cx.update(|cx| g.shell.read(cx).workspace_id());
    assert!(cx.update(|cx| g.shell.update(cx, |s, cx| s.set_menus(&id, vec![menu.clone()], cx))));
    assert!(!cx.update(|cx| g.shell.update(cx, |s, cx| s.set_menus("no.such", vec![], cx))));
    assert_eq!(cx.update(|cx| g.shell.read(cx).workspace().menus.clone()), vec![menu]);
    frame(cx, window);

    // An overlay shows over everything and goes away
    let overlay = cx.update(|cx| cx.new(|_| gpui_kit::Empty));
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.set_overlay(Some(overlay.into()), window, cx))).unwrap();
    frame(cx, window);
    assert!(cx.update(|cx| g.shell.read(cx).has_overlay()));
    cx.update_window(window, |_, window, cx| g.shell.update(cx, |s, cx| s.set_overlay(None, window, cx))).unwrap();
    assert!(!cx.update(|cx| g.shell.read(cx).has_overlay()));
    frame(cx, window);
}

#[test]
fn the_window_resizes_from_its_edge() {
    use gpui_kit::{ResizeEdge, point, px, size};
    let s = size(px(800.), px(600.));
    assert_eq!(kitchen_shell::resize_edge(point(px(2.), px(2.)), s), Some(ResizeEdge::TopLeft));
    assert_eq!(kitchen_shell::resize_edge(point(px(797.), px(300.)), s), Some(ResizeEdge::Right));
    assert_eq!(kitchen_shell::resize_edge(point(px(400.), px(598.)), s), Some(ResizeEdge::Bottom));
    assert_eq!(kitchen_shell::resize_edge(point(px(400.), px(300.)), s), None);
}
