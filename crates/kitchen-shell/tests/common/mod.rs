//! What the headless tests share: the gallery app in a test window.

#![allow(dead_code)]

use gpui_kit::AppContext as _;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use kitchen_ui::theme::{self, Look, Palette};

#[path = "../../examples/gallery/app.rs"]
pub mod app;
#[path = "../../examples/gallery/demo.rs"]
pub mod demo;
#[path = "../../examples/gallery/project.rs"]
pub mod project;
#[path = "../../examples/gallery/views.rs"]
pub mod views;

/// Draws a frame and lets the UI settle.
pub fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

/// The gallery in a 1280 × 800 test window (no meters).
pub fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, app::Gallery) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::LIME), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1280.), px(800.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        app::open(options, false, cx)
    })
}
