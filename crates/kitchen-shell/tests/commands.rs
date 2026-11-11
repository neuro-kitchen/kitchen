//! Headless UI tests of the command registry: menus, search and keys from one list; commands that
//! cannot run say why; arguments are checked.

#[path = "../examples/gallery/demo.rs"]
mod demo;
#[path = "../examples/gallery/view.rs"]
mod view;
#[path = "../examples/gallery/views.rs"]
mod views;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_ui::theme::{self, Look, Palette};
use serde_json::{Value, json};

fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<view::Gallery>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::LIME), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        let (w, g) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("open test window");
        let registry = g.read(cx).registry.clone();
        let bad = registry.bind_keys(None, cx);
        assert!(bad.is_empty(), "keys that did not parse: {bad:?}");
        (w, g)
    })
}

fn log(cx: &mut TestAppContext, g: &Entity<view::Gallery>) -> Vec<String> {
    cx.update(|cx| g.read(cx).state.read(cx).0.log.clone())
}

#[gpui_kit::test]
fn commands_run_through_the_registry_and_say_why_not(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    frame(cx, window);
    let registry = cx.update(|cx| g.read(cx).registry.clone());

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

    // Ctrl+K is a command too: it opens the search
    cx.update_window(window, |_, window, cx| window.press("ctrl-k", cx)).unwrap();
    frame(cx, window);
    let search = cx.update(|cx| g.read(cx).search.as_ref().map(|(s, _)| s.clone())).expect("the search opened");

    // Typing filters by title, group and hint; a disabled command stays, with its reason
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("undo", window, cx))).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].command.id.as_ref(), "edit.undo");
    assert_eq!(rows[0].disabled, None, "the log has a line, so undo runs");
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("file", window, cx))).unwrap();
    let rows = cx.update(|cx| search.read(cx).rows(cx));
    let save = rows.iter().find(|r| r.command.id.as_ref() == "file.save").expect("save is listed");
    assert!(save.disabled.is_none(), "opening changed something, so save runs");

    // Enter runs the highlighted row; the search closes
    cx.update_window(window, |_, window, cx| search.update(cx, |s, cx| s.set_query("blur", window, cx))).unwrap();
    frame(cx, window);
    cx.update_window(window, |_, window, cx| window.press("enter", cx)).unwrap();
    frame(cx, window);
    assert!(cx.update(|cx| g.read(cx).search.is_none()), "the search closed");
    assert_eq!(log(cx, &g).last().map(String::as_str), Some("Blur, sigma 2"));

    // Both themes draw the page and the search
    for dark in [false, true] {
        cx.update(|cx| theme::use_look(theme::look().with_dark(dark), cx));
        cx.update_window(window, |_, window, cx| g.update(cx, |g, cx| g.open_search(window, cx))).unwrap();
        frame(cx, window);
    }
}
