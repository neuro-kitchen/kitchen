//! The Design System's frame on the stand-in dsp-kitchen (`examples/frame`), headless, at a
//! desktop (1280 px), a tablet held upright (820 px) and a phone (390 px) width.

#[path = "../examples/frame/dspk.rs"]
mod dspk;

use gpui_kit::AppContext as _;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_ui::theme::{self, Look, Palette};

fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

fn open(cx: &mut TestAppContext, width: f32) -> (AnyWindowHandle, dspk::Dspk) {
    let (window, d) = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::LIME), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(width), px(800.)) };
        dspk::open(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, false, cx)
    });
    frame(cx, window);
    (window, d)
}

/// Whether the element `id` is drawn (after one more frame: what a key or a click opened
/// appears on the next one).
fn has(cx: &mut TestAppContext, window: AnyWindowHandle, id: &str) -> bool {
    frame(cx, window);
    cx.update_window(window, |_, window, _| window.try_find(id.to_string()).is_some()).unwrap()
}

fn press(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &str) {
    cx.update_window(window, |_, window, cx| window.press(keys, cx)).unwrap();
    frame(cx, window);
}

fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &str) {
    cx.update_window(window, |_, window, cx| window.click(id.to_string(), cx)).unwrap();
    frame(cx, window);
}

fn current(cx: &mut TestAppContext, d: &dspk::Dspk) -> (String, Option<String>) {
    cx.update(|cx| {
        let s = d.shell.read(cx);
        (s.workspace_id().to_string(), s.current_view().map(|v| v.to_string()))
    })
}

/// Left to right: mark, File Edit View Help, the views; at the right search, Pipeline, Code,
/// theme.
#[gpui_kit::test]
fn the_title_bar_holds_the_menus_the_views_and_the_shared_workspaces(cx: &mut TestAppContext) {
    let (window, _d) = open(cx, 1280.);
    for id in [
        "menu-file",
        "menu-edit",
        "menu-view",
        "menu-help",
        "title-views",
        "workspace-explore",
        "workspace-sorting",
        "workspace-curation",
        "workspace-analysis",
        "title-search",
        "workspace-pipeline",
        "workspace-code",
        "title-theme",
    ] {
        assert!(has(cx, window, id), "{id} in the title bar at 1280 px");
    }
    let x = |cx: &mut TestAppContext, id: &str| cx.update_window(window, |_, window, _| window.try_find(id.to_string()).map(|b| b.bounds().origin.x.as_f32())).unwrap().unwrap();
    let order = ["menu-file", "menu-help", "title-views", "title-search", "workspace-pipeline", "workspace-code", "title-theme"];
    let xs: Vec<f32> = order.iter().map(|id| x(cx, id)).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "left to right: {order:?} at {xs:?}");
    assert!(!has(cx, window, "title-menu"), "not folded");
    assert!(!has(cx, window, "title-panels"), "no drawer button when the side pane fits");
}

/// Ctrl+1…4 the views; Ctrl+Alt+P and Ctrl+Alt+C the shared workspaces, during which no view is
/// current; each screen has its own menu row.
#[gpui_kit::test]
fn keys_switch_views_and_shared_workspaces(cx: &mut TestAppContext) {
    let (window, d) = open(cx, 1280.);
    assert_eq!(current(cx, &d), ("explore".into(), Some("explore".into())));
    assert!(has(cx, window, "menu-explore-0"), "Explore's Signal menu");
    for (key, id) in [("ctrl-2", "sorting"), ("ctrl-3", "curation"), ("ctrl-4", "analysis"), ("ctrl-1", "explore")] {
        press(cx, window, key);
        assert_eq!(current(cx, &d), (id.to_string(), Some(id.to_string())), "{key}");
        assert!(has(cx, window, &format!("menu-{id}-0")), "{id}'s menu row");
    }
    press(cx, window, "ctrl-alt-p");
    assert_eq!(current(cx, &d), ("pipeline".into(), None), "Pipeline: no view is current");
    assert!(has(cx, window, "menu-pipeline-0") && has(cx, window, "menu-pipeline-1"), "Operations and Plugins");
    press(cx, window, "ctrl-alt-c");
    assert_eq!(current(cx, &d), ("code".into(), None));
    // The switcher's buttons and the corner icons do the same
    click(cx, window, "workspace-curation");
    assert_eq!(current(cx, &d).0, "curation");
    click(cx, window, "workspace-pipeline");
    assert_eq!(current(cx, &d).0, "pipeline");
}

/// Ctrl+O brings up the picker, which lists what dsp-kitchen opens; a double click opens.
#[gpui_kit::test]
fn file_open_brings_up_the_picker_and_opens_what_is_picked(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("kitchen-dspk-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("probe.zarr")).unwrap();
    std::fs::write(dir.join("rec.bin"), b"0123").unwrap();
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    let (window, d) = open(cx, 1280.);
    press(cx, window, "ctrl-o");
    let picker = cx.update(|cx| d.shell.read(cx).picker()).expect("the picker is open");
    assert!(has(cx, window, "picker"));
    cx.update(|cx| picker.update(cx, |p, cx| {
        p.picker_mut().enter(&dir).unwrap();
        cx.notify();
    }));
    frame(cx, window);
    let names: Vec<String> = cx.update(|cx| picker.read(cx).picker().entries().iter().map(|e| e.name.clone()).collect());
    assert_eq!(names, ["probe.zarr", "rec.bin"], "what dsp-kitchen opens; notes.txt hidden");
    // Select rec.bin, then Open
    click(cx, window, "picker-entry-1");
    click(cx, window, "picker-open");
    assert_eq!(*d.opened.borrow(), [dir.join("rec.bin")]);
    assert!(cx.update(|cx| d.shell.read(cx).picker()).is_none(), "closed after opening");
    // Esc cancels
    press(cx, window, "ctrl-o");
    press(cx, window, "escape");
    assert!(cx.update(|cx| d.shell.read(cx).picker()).is_none());
    assert_eq!(d.opened.borrow().len(), 1, "nothing more opened");
}

/// 820 px: views as icons, Explore's side pane in the Panels drawer.
#[gpui_kit::test]
fn a_tablet_width_puts_side_panes_in_the_drawer(cx: &mut TestAppContext) {
    let (window, d) = open(cx, 820.);
    assert!(has(cx, window, "menu-file"), "the menus are still there at 820 px");
    assert!(has(cx, window, "title-panels"), "the drawer's button");
    click(cx, window, "title-panels");
    assert!(cx.update(|cx| d.shell.read(cx).drawer_open()));
    assert!(has(cx, window, "panels-drawer-scrim"), "the drawer and the scrim beside it");
    // A click beside it closes it
    click(cx, window, "panels-drawer-scrim");
    assert!(!cx.update(|cx| d.shell.read(cx).drawer_open()));
    // Ctrl+\ opens it again; switching screens closes it; Sorting has no side panes
    press(cx, window, "ctrl-\\");
    assert!(cx.update(|cx| d.shell.read(cx).drawer_open()));
    press(cx, window, "ctrl-2");
    assert!(!cx.update(|cx| d.shell.read(cx).drawer_open()));
    assert!(!has(cx, window, "title-panels"), "no drawer button for a screen without side panes");
}

/// 390 px: the four menus in one button, the toggles and theme in More; everything in reach.
#[gpui_kit::test]
fn a_phone_width_folds_the_menus_and_keeps_everything_in_reach(cx: &mut TestAppContext) {
    let (window, d) = open(cx, 390.);
    assert!(has(cx, window, "title-menu"), "one menu button");
    for id in ["menu-file", "menu-edit", "menu-view", "menu-help", "title-theme"] {
        assert!(!has(cx, window, id), "{id} folded away");
    }
    for id in ["title-more", "workspace-explore", "workspace-analysis", "workspace-pipeline", "workspace-code", "title-search"] {
        assert!(has(cx, window, id), "{id} still in reach");
    }
    click(cx, window, "title-menu");
    // The keys still work
    press(cx, window, "escape");
    press(cx, window, "ctrl-alt-c");
    assert_eq!(current(cx, &d).0, "code");
}
