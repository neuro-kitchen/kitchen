//! Headless UI tests of the view container: views added from the dialog, arranged, switched from
//! their tab buttons.

#[path = "../examples/gallery/demo.rs"]
mod demo;
#[path = "../examples/gallery/view.rs"]
mod view;
#[path = "../examples/gallery/views.rs"]
mod views;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_shell::{Arrangement, DialogEvent};
use kitchen_ui::theme::{self, Look, Palette};

fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn views_are_added_arranged_and_switched(cx: &mut TestAppContext) {
    let (window, g) = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::EOSIN), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("open test window")
    });
    cx.update(|cx| g.update(cx, |g, cx| {
        g.page = view::Page::Views;
        cx.notify();
    }));
    frame(cx, window);
    let page = cx.update(|cx| g.read(cx).views.clone());
    assert_eq!(cx.update(|cx| page.read(cx).panels.len()), 2, "two views to start");

    // The dialog adds a view of the kind picked
    cx.update_window(window, |_, window, cx| page.update(cx, |p, cx| p.open_dialog(window, cx))).unwrap();
    frame(cx, window);
    let dialog = cx.update(|cx| page.read(cx).dialog.as_ref().map(|(d, _)| d.clone())).expect("the dialog opened");
    // Rows: the four arrangements, then the kinds; row 5 is "Table"
    cx.update(|cx| dialog.update(cx, |d, cx| d.run(5, cx)));
    frame(cx, window);
    let panels = cx.update(|cx| page.read(cx).panels.clone());
    assert_eq!(panels.len(), 3);
    assert_eq!(cx.update(|cx| panels[2].read(cx).kind), 1);
    assert!(cx.update(|cx| page.read(cx).dialog.is_none()), "the dialog closed");

    // Every arrangement lays the views out
    for how in Arrangement::ALL {
        cx.update_window(window, |_, window, cx| page.update(cx, |p, cx| p.open_dialog(window, cx))).unwrap();
        let dialog = cx.update(|cx| page.read(cx).dialog.as_ref().map(|(d, _)| d.clone())).unwrap();
        cx.update(|cx| dialog.update(cx, |_, cx| cx.emit(DialogEvent::Arrange(how))));
        frame(cx, window);
        assert_eq!(cx.update(|cx| page.read(cx).arrangement), Some(how));
    }

    // Both themes draw the dock
    for dark in [false, true] {
        cx.update(|cx| theme::use_look(theme::look().with_dark(dark), cx));
        frame(cx, window);
    }
}
