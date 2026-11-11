//! Headless UI test: the gallery (every widget) drawn with each app palette and a few custom
//! accents, dark and light; the accent picker; a slider that follows a click.

#[path = "../examples/gallery/view.rs"]
mod view;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, point, px, size};
use kitchen_ui::theme::{self, Palette, Rgb};

/// Draws a frame and lets the UI settle.
fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn gallery_draws_in_every_look_and_the_slider_follows_a_click(cx: &mut TestAppContext) {
    let (window, gallery) = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(view::initial_look(false), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1440.), px(860.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("open test window")
    });
    frame(cx, window);
    let accents = [None, Some(Rgb(0xfffacd)), Some(Rgb(0x0a1030)), Some(Rgb(0xff0000))];
    for p in Palette::APPS {
        for accent in accents {
            for dark in [true, false] {
                cx.update(|cx| theme::use_look(theme::look().with_palette(p).with_accent(accent).with_dark(dark), cx));
                frame(cx, window);
            }
        }
    }

    // Picking a suggested accent applies it; "App" goes back to the app's palette
    cx.update_window(window, |_, window, cx| window.click("accent-2b7f86", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|_| theme::look().accent), Some(Rgb(0x2b7f86)));
    cx.update_window(window, |_, window, cx| window.click("accent-app", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|_| theme::look().accent), None);

    // A click at three quarters of the track sets the value to about 0.75
    cx.update_window(window, |_, window, cx| {
        let b = window.find("gallery-level-track").bounds();
        window.click_at("gallery-level-track", point(b.size.width * 0.75, b.size.height / 2.), cx);
    })
    .unwrap();
    frame(cx, window);
    let level = cx.update(|cx| gallery.read(cx).level);
    assert!((level - 0.75).abs() < 0.03, "slider value after the click: {level}");
}
