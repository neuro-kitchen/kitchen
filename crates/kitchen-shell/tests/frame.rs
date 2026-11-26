//! Headless UI tests of the frame: workspaces, layout toggles, Settings, the status bar's jobs.

mod common;

use common::{frame, open, open_sized};
use gpui_kit::AppContext as _;
use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
use kitchen_shell::frame::{PANELS_W, TITLE_COMPACT_W, TITLE_NARROW_W, TitleFit};
use kitchen_shell::jobs;
use kitchen_ui::theme;

#[gpui_kit::test]
fn the_frame_switches_workspaces_and_opens_settings(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).workspace().id.clone()).as_ref(), "commands");

    // Ctrl+2: the second workspace, from the frame's own command
    cx.update_window(window, |_, window, cx| window.press("ctrl-2", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).workspace().id.clone()).as_ref(), "views");
    // The switcher's button
    cx.update_window(window, |_, window, cx| window.click("workspace-commands", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).workspace().id.clone()).as_ref(), "commands");

    // A layout toggle in the title bar runs its command
    cx.update_window(window, |_, window, cx| window.click("toggle-view.side_panel", cx)).unwrap();
    frame(cx, window);
    assert!(cx.update(|cx| g.state.read(cx).0.side_panel));

    // Settings: Appearance first, then the app's sections; Esc closes
    cx.update_window(window, |_, window, cx| window.press("ctrl-,", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).settings_section()), Some(0));
    cx.update_window(window, |_, window, cx| window.click("settings-section-1", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).settings_section()), Some(1));
    cx.update_window(window, |_, window, cx| window.press("escape", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| g.shell.read(cx).settings_section()), None);

    // The theme button in the title bar
    let dark = theme::is_dark();
    cx.update_window(window, |_, window, cx| window.click("title-theme", cx)).unwrap();
    frame(cx, window);
    assert_eq!(theme::is_dark(), !dark);
}

#[gpui_kit::test]
fn the_status_bar_lists_jobs_and_stops_them(cx: &mut TestAppContext) {
    let (window, _g) = open(cx);
    let job = jobs::start("test", "Exporting tables", false).unwrap();
    job.set("Exporting tables: 3 of 8", Some(0.375));
    frame(cx, window);
    let id = job.id;
    cx.update_window(window, |_, window, cx| window.click(format!("job-cancel-{id}"), cx)).unwrap();
    frame(cx, window);
    assert!(job.cancelled(), "the cancel button asked the job to stop");
    drop(job);
    frame(cx, window);
}

#[test]
fn the_title_bar_fits_the_window_width() {
    assert_eq!(TitleFit::of(390.), TitleFit::Narrow, "a phone held upright");
    assert_eq!(TitleFit::of(TITLE_NARROW_W), TitleFit::Medium);
    assert_eq!(TitleFit::of(820.), TitleFit::Medium, "a tablet held upright");
    assert_eq!(TitleFit::of(PANELS_W), TitleFit::Compact);
    assert_eq!(TitleFit::of(1024.), TitleFit::Compact, "a small laptop window");
    assert_eq!(TitleFit::of(TITLE_COMPACT_W), TitleFit::Full);
    assert_eq!(TitleFit::of(1920.), TitleFit::Full);
    assert!(TitleFit::Narrow.folded() && !TitleFit::Medium.folded());
    assert!(TitleFit::Medium.drawer() && !TitleFit::Compact.drawer());
    assert!(TitleFit::Full.labels() && !TitleFit::Compact.labels());
}

#[gpui_kit::test]
fn the_general_menus_come_first_and_file_keeps_its_items(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    frame(cx, window);
    cx.update_window(window, |_, window, _| {
        for id in ["menu-file", "menu-view", "menu-help", "title-search", "title-theme"] {
            assert!(window.try_find(id).is_some(), "{id} in the title bar");
        }
        assert!(window.try_find("title-menu").is_none(), "the menus are not folded at 1280 px");
    })
    .unwrap();
    // File: Open… (the picker), the app's items, Settings, Quit
    let ids: Vec<String> = g.registry.all().into_iter().map(|c| c.id.to_string()).collect();
    assert!(!ids.iter().any(|i| i == "shell.open"), "no Open… for an app that opens no files");
    for id in ["shell.quit", "shell.settings", "shell.panels"] {
        assert!(ids.iter().any(|i| i == id), "{id} is a command");
    }
}

#[gpui_kit::test]
fn a_narrow_window_keeps_every_title_bar_control_in_reach(cx: &mut TestAppContext) {
    let (window, _g) = open_sized(cx, 390., 800.);
    frame(cx, window);
    cx.update_window(window, |_, window, _| {
        // The toggles and the theme live in the "More" menu; the workspaces stay, as icons
        assert!(window.try_find("title-more").is_some(), "a More menu");
        assert!(window.try_find("toggle-view.side_panel").is_none(), "no toggle in the bar");
        assert!(window.try_find("title-theme").is_none(), "no theme button in the bar");
        assert!(window.try_find("workspace-commands").is_some(), "the workspaces stay");
    })
    .unwrap();
    // Its menu offers the toggles and the theme, as the bar's buttons did
    cx.update_window(window, |_, window, cx| window.click("title-more", cx)).unwrap();
    frame(cx, window);
}
