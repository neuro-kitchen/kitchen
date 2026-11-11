//! Plugins end to end (Unix: the demo plugins are shell scripts): discovery and the handshake,
//! a server answering requests, the manager installing and removing, the Settings page.

#![cfg(unix)]

#[path = "../examples/support/demo_plugins.rs"]
mod demo_plugins;

use gpui_kit::AppContext as _;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_hub::Hub;
use kitchen_plugin::ui::PluginsPage;
use kitchen_plugin::{CoreRequest, Manager, PluginKind, PluginProcess};
use serde_json::json;

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("kitchen-plugin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn plugins_are_found_checked_served_installed_and_removed() {
    let root = scratch("e2e");
    let shelf = root.join("downloads");
    let echo = demo_plugins::write_all(&shelf);
    let kind = PluginKind::new("demo", "demo-ml-", "KITCHEN_PLUGIN_TEST_UNSET").with_user_dir(root.join("plugins")).without_system();
    let manager = Manager::new(kind.clone(), Some(Hub::new(root.join("hub"))));
    assert!(manager.list().is_empty(), "nothing installed yet");

    // Installing checks the handshake first
    let info = manager.install_from(&echo).unwrap();
    assert_eq!((info.name.as_str(), info.protocol), ("demo-ml-echo", 1));
    assert!(info.can("segment"));
    assert!(manager.install_from(&shelf.join("demo-ml-future")).unwrap_err().contains("update the app"));
    assert!(manager.install_from(&shelf.join("demo-ml-broken")).unwrap_err().contains("cannot find its runtime"));
    let list = manager.list();
    assert_eq!(list.len(), 1);
    assert!(list[0].removable && list[0].bytes > 0);

    // A server answers requests, reports progress, skips stray lines, and reports errors
    let mut p = PluginProcess::start(list[0].program.to_str().unwrap()).unwrap();
    let mut stages = Vec::new();
    let r = p.request(&json!({"request": "echo", "text": "hi"}), &mut |s, f| stages.push((s.to_string(), f))).unwrap();
    assert_eq!(r, json!({"request": "echo", "text": "hi"}));
    assert_eq!(stages, vec![("echoing".to_string(), Some(0.5))]);
    assert_eq!(p.request(&json!({"request": "fail"}), &mut |_, _| {}).unwrap_err(), "asked to fail");
    assert!(p.request(&CoreRequest::Install { id: "echo-tiny".into() }, &mut |_, _| {}).is_ok());
    assert!(p.alive());
    drop(p);

    // What it downloaded counts, and goes with it
    std::fs::create_dir_all(root.join("hub/echo")).unwrap();
    std::fs::write(root.join("hub/echo/weights.bin"), vec![0u8; 1000]).unwrap();
    assert!(manager.list()[0].bytes >= 1000);
    manager.remove("demo-ml-echo", true).unwrap();
    assert!(manager.list().is_empty() && !root.join("hub/echo").exists());
    assert!(manager.remove("demo-ml-echo", true).is_err(), "already gone");
    let _ = std::fs::remove_dir_all(&root);
}

#[gpui_kit::test]
fn the_settings_page_lists_and_removes_plugins(cx: &mut TestAppContext) {
    let root = scratch("page");
    let kind = PluginKind::new("demo", "demo-ml-", "KITCHEN_PLUGIN_TEST_UNSET").with_user_dir(root.join("plugins")).without_system();
    // Installed in place: the answering one, the newer one and the broken one
    demo_plugins::write_all(&kind.user_dir);
    let manager = Manager::new(kind, None);
    cx.executor().allow_parking();
    let (window, page) = cx.update(|cx| {
        gpui_kit::init(cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(900.), px(700.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|cx| PluginsPage::new(manager, cx))).unwrap()
    });
    for _ in 0..500 {
        cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
        if !cx.update(|cx| page.read(cx).loading()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let rows = cx.update(|cx| page.read(cx).rows().to_vec());
    assert_eq!(rows.len(), 3);
    let ok: Vec<&str> = rows.iter().filter(|r| r.info.is_ok()).map(|r| r.name.as_str()).collect();
    assert_eq!(ok, vec!["demo-ml-echo"], "the newer and the broken one are listed with their reason");
    let echo = rows.iter().position(|r| r.name == "demo-ml-echo").unwrap();
    cx.update_window(window, |_, window, cx| window.click(format!("plugin-remove-{echo}"), cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| page.read(cx).message().cloned()), Some((false, "Removed demo-ml-echo".into())));
    let _ = std::fs::remove_dir_all(&root);
}
