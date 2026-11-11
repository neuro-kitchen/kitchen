//! Opens a window showing kitchen-shell's pieces on a small demo app, in the shared frame.
//!
//! `cargo run -p kitchen-shell --example gallery` (add `-- --light` for the light theme).

mod app;
mod demo;
mod views;

use gpui_kit::{Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use kitchen_ui::icons::AppAssets;
use kitchen_ui::theme::{self, Look, Palette};

fn main() {
    let light = std::env::args().any(|a| a == "--light");
    gpui_kit::application().with_assets(AppAssets::new().all_icons()).run(move |cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::LIME).with_dark(!light), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("kitchen-shell gallery".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(800.)), cx))),
            window_min_size: Some(size(px(800.), px(500.))),
            ..Default::default()
        };
        app::open(options, true, cx);
        cx.activate(true);
    });
}
