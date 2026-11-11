//! Opens a window showing every kitchen-ui widget and token. The header switches dark and light,
//! the palette and the text size.
//!
//! `cargo run -p kitchen-ui --example gallery` (add `-- --light` to open in the light theme).

mod view;

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use kitchen_ui::icons::AppAssets;
use kitchen_ui::theme;

// The icons the gallery adds to gpui-kit's component set (the empty state's)
gpui_kit::assets::icon_assets!(GalleryIcons, [Microscope]);

fn main() {
    let light = std::env::args().any(|a| a == "--light");
    gpui_kit::application().with_assets(AppAssets::new().icons(GalleryIcons)).run(move |cx| {
        gpui_kit::init(cx);
        theme::use_look(view::initial_look(light), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("kitchen-ui gallery".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(860.)), cx))),
            window_min_size: Some(size(px(960.), px(560.))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("failed to open the gallery window");
        cx.activate(true);
    });
}
