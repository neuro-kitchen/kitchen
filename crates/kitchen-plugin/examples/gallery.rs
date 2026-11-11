//! Opens the plugin manager's Settings page over a few demo plugins (shell scripts written to a
//! temporary folder; Unix).
//!
//! `cargo run -p kitchen-plugin --example plugin-gallery`

#[path = "support/demo_plugins.rs"]
mod demo_plugins;

use gpui_kit::component::v_flex;
use gpui_kit::{AppContext as _, Bounds, Entity, InteractiveElement as _, IntoElement, StatefulInteractiveElement as _, ParentElement as _, Render, Styled as _, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use kitchen_hub::Hub;
use kitchen_plugin::ui::PluginsPage;
use kitchen_plugin::{Manager, PluginKind};
use kitchen_ui::icons::AppAssets;
use kitchen_ui::theme::{self, Look, Palette, color};

struct Page(Entity<PluginsPage>);

impl Render for Page {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        v_flex().id("page").size_full().overflow_y_scroll().p(px(theme::XL)).bg(color::panel()).text_color(color::text()).child(self.0.clone())
    }
}

fn main() {
    let root = std::env::temp_dir().join("kitchen-plugin-gallery");
    let kind = PluginKind::new("demo", "demo-ml-", "DEMO_PLUGINS").with_user_dir(root.join("plugins")).without_system();
    demo_plugins::write_all(&kind.user_dir);
    let manager = Manager::new(kind, Some(Hub::new(root.join("hub"))));
    gpui_kit::application().with_assets(AppAssets::new().all_icons()).run(move |cx| {
        gpui_kit::init(cx);
        theme::use_look(Look::new(Palette::EOSIN), cx);
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("kitchen-plugin gallery".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(820.), px(640.)), cx))),
            ..Default::default()
        };
        let m = manager.clone();
        gpui_kit::open_window(options, cx, |_, cx| {
            let page = cx.new(|cx| PluginsPage::new(m, cx));
            cx.new(|_| Page(page))
        })
        .expect("failed to open the gallery window");
        cx.activate(true);
    });
}
