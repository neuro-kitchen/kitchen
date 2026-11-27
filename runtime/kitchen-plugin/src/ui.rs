//! The plugin manager's Settings page: every plugin found, its version, licence, capabilities,
//! whether its runtime is ready and the room it takes; install one from a file, refresh, remove
//! one (with what it downloaded). Discovery and installs run off the UI thread. An app shows the
//! page as a section of its Settings (`kitchen-shell`'s `SettingsSection`).

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    Context, EventEmitter, FontWeight, IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Window, div, px,
};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{EmptyState, Section};

use crate::manager::{Installed, Manager};

/// The page tells its owner when plugins were installed or removed (to look again at what they
/// can do).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginsChanged;

/// The plugin manager's page.
pub struct PluginsPage {
    manager: Manager,
    rows: Vec<Installed>,
    loading: bool,
    message: Option<(bool, String)>,
}

impl EventEmitter<PluginsChanged> for PluginsPage {}

impl PluginsPage {
    /// The page over `manager`; starts looking for plugins.
    pub fn new(manager: Manager, cx: &mut Context<Self>) -> Self {
        let mut page = Self { manager, rows: Vec::new(), loading: false, message: None };
        page.refresh(cx);
        page
    }

    /// The plugins found (after the last refresh).
    pub fn rows(&self) -> &[Installed] {
        &self.rows
    }

    /// Whether it is looking for plugins.
    pub fn loading(&self) -> bool {
        self.loading
    }

    /// The last message (`true`: an error).
    pub fn message(&self) -> Option<&(bool, String)> {
        self.message.as_ref()
    }

    /// Looks for plugins again (off the UI thread).
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let manager = self.manager.clone();
        cx.spawn(async move |this, cx| {
            let rows = cx.background_executor().spawn(async move { manager.list() }).await;
            let _ = this.update(cx, |p, cx| {
                p.rows = rows;
                p.loading = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Installs the plugin program at `path` (off the UI thread), then looks again.
    pub fn install(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let manager = self.manager.clone();
        self.message = Some((false, format!("Installing {}…", path.display())));
        cx.spawn(async move |this, cx| {
            let r = cx.background_executor().spawn(async move { manager.install_from(&path) }).await;
            let _ = this.update(cx, |p, cx| {
                p.message = Some(match r {
                    Ok(info) => (false, format!("Installed {} {}", info.name, info.version)),
                    Err(e) => (true, e),
                });
                cx.emit(PluginsChanged);
                p.refresh(cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// Removes the plugin `name` and what it downloaded.
    pub fn remove(&mut self, name: &str, cx: &mut Context<Self>) {
        self.message = Some(match self.manager.remove(name, true) {
            Ok(()) => (false, format!("Removed {name}")),
            Err(e) => (true, e),
        });
        cx.emit(PluginsChanged);
        self.refresh(cx);
    }

    fn pick_file(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Install plugin".into()) });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |p, cx| p.install(path, cx));
            }
        })
        .detach();
    }

    fn row(&self, i: usize, r: &Installed, cx: &mut Context<Self>) -> impl IntoElement {
        let remove = r.removable.then(|| {
            let name = r.name.clone();
            Button::new(SharedString::from(format!("plugin-remove-{i}"))).small().outline().icon(IconName::Trash).label("Remove").tooltip("Remove the plugin and what it downloaded").on_click(cx.listener(move |p, _, _, cx| p.remove(&name, cx)))
        });
        let head = |title: String, version: String| {
            h_flex()
                .gap(px(theme::SM))
                .items_center()
                .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_xs().text_color(color::text_muted()).child(version))
                .child(div().text_xs().font_family("monospace").text_color(color::text_muted()).child(human_bytes(r.bytes)))
        };
        let body = match &r.info {
            Ok(info) => {
                let runtime = info.runtime.as_ref().map(|rt| div().text_xs().text_color(if rt.ready { color::text_muted() } else { color::warning() }).child(if rt.ready { format!("Ready: {}", rt.detail) } else { format!("Not ready: {}", rt.detail) }));
                v_flex()
                    .gap(px(theme::XS))
                    .child(head(info.name.clone(), format!("{} · {}", info.version, if info.licence.is_empty() { "licence not stated" } else { &info.licence })))
                    .child(h_flex().gap(px(theme::XS)).flex_wrap().children(info.capabilities.iter().map(|c| div().px(px(theme::SM)).py(px(2.)).rounded(px(theme::RADIUS_SM)).bg(color::control()).text_xs().child(c.clone()))))
                    .children(runtime)
                    .child(div().text_xs().text_color(color::text_muted()).truncate().child(r.program.display().to_string()))
            }
            Err(e) => v_flex().gap(px(theme::XS)).child(head(r.name.clone(), "does not answer".into())).child(div().text_xs().text_color(color::danger()).child(e.clone())),
        };
        h_flex().w_full().gap(px(theme::MD)).items_start().p(px(theme::MD)).rounded(px(theme::RADIUS_LG)).border_1().border_color(color::border()).bg(color::raised()).child(div().flex_1().min_w_0().child(body)).children(remove)
    }
}

/// `1 234 567` bytes as "1.2 MB".
pub fn human_bytes(b: u64) -> String {
    let b = b as f64;
    match b {
        b if b >= 1e9 => format!("{:.1} GB", b / 1e9),
        b if b >= 1e6 => format!("{:.1} MB", b / 1e6),
        b if b >= 1e3 => format!("{:.0} kB", b / 1e3),
        b => format!("{b:.0} B"),
    }
}

impl Render for PluginsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total: u64 = self.rows.iter().map(|r| r.bytes).sum();
        let summary = if self.loading { "Looking for plugins…".to_string() } else { format!("{} found · {} with what they downloaded", self.rows.len(), human_bytes(total)) };
        let actions = h_flex()
            .gap(px(theme::SM))
            .child(Button::new("plugins-install").small().primary().icon(IconName::Plus).label("Install from file…").on_click(cx.listener(|p, _, _, cx| p.pick_file(cx))))
            .child(Button::new("plugins-refresh").small().outline().icon(IconName::RefreshCw).label("Refresh").disabled(self.loading).on_click(cx.listener(|p, _, _, cx| p.refresh(cx))));
        let rows: Vec<_> = self.rows.clone().iter().enumerate().map(|(i, r)| self.row(i, r, cx).into_any_element()).collect();
        let list = if rows.is_empty() && !self.loading {
            div()
                .h(px(220.))
                .child(EmptyState::new(IconName::Blocks, "No plugins", format!("Plugins are programs named {}…; install one from a file, or put it in {}.", self.manager.kind.prefix, self.manager.kind.user_dir.display())))
                .into_any_element()
        } else {
            v_flex().gap(px(theme::SM)).children(rows).into_any_element()
        };
        v_flex()
            .w_full()
            .gap(px(theme::MD))
            .child(h_flex().gap(px(theme::SM)).items_center().flex_wrap().child(div().flex_1().min_w_0().text_sm().text_color(color::text_muted()).child(summary)).child(actions))
            .children(self.message.clone().map(|(err, m)| div().text_sm().text_color(if err { color::danger() } else { color::text_muted() }).child(m)))
            .child(Section::new("Installed").child(list))
    }
}
