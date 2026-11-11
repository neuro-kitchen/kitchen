//! Opens a window showing kitchen-shell's pieces on a small demo app.
//!
//! `cargo run -p kitchen-shell --example gallery` (add `-- --light` for the light theme).

mod demo;
mod view;
mod views;

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
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
        let (_, gallery) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("failed to open the gallery window");
        // Every command's key, from the registry
        let registry = gallery.read(cx).registry.clone();
        let bad = registry.bind_keys(None, cx);
        for b in bad {
            eprintln!("key not bound: {b}");
        }
        cx.activate(true);
    });
}
