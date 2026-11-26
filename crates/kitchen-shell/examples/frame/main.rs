//! The shared frame on a stand-in dsp-kitchen: four views, Pipeline and Code, File → Open….
//!
//! `cargo run -p kitchen-shell --example frame` (`-- --light` for the light theme, `-- --teal`
//! for lucem's accent). Resize the window to see it fold: 1100 px, 900 px, 560 px.

mod dspk;

use gpui_kit::{Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use kitchen_ui::icons::AppAssets;
use kitchen_ui::theme::{self, Look, Palette};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let light = args.iter().any(|a| a == "--light");
    let palette = if args.iter().any(|a| a == "--teal") { Palette::TEAL } else { Palette::LIME };
    gpui_kit::application().with_assets(AppAssets::new()).run(move |cx| {
        gpui_kit::init(cx);
        theme::fonts::load(cx);
        theme::use_look(Look::new(palette).with_dark(!light), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("dsp-kitchen (frame)".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(800.)), cx))),
            window_min_size: Some(size(px(360.), px(480.))),
            ..Default::default()
        };
        dspk::open(options, true, cx);
        cx.activate(true);
    });
}
