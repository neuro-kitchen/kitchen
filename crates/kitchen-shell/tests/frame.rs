//! Headless UI tests of the frame: workspaces, layout toggles, Settings, the status bar's jobs.

mod common;

use common::{frame, open};
use gpui_kit::AppContext as _;
use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
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
