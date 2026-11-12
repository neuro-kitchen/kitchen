//! The frame every app shares ([`Shell`]): an app hands it its registry, its workspaces, its
//! menus and its Settings sections ([`ShellSpec`]), and the frame draws them the same way in
//! every app (Affinity's layout):
//!
//! - **Title bar**: the app's menu (named after the app: files, Settings) and Help at the left,
//!   then the workspace switcher; at the right the project, the remote button (where the app
//!   runs), the layout toggles (commands, as icon buttons) and the theme.
//! - **Menu row**: the current workspace's menus, built from the registry.
//! - **Body**: the current workspace's view, a card on the chrome.
//! - **Status bar**: a status line, the running jobs ([`crate::jobs`]) with a cancel button each,
//!   and this computer's load (CPU, memory).
//! - **Settings** (Ctrl+,): sections on the left (Appearance, built in, then the app's), the
//!   section's settings on the right.
//! - **Search** (Ctrl+K, F3) over the registry.
//! - **Window frame**: where the app draws its own frame (client-side decorations on Linux), the
//!   window has rounded corners while it floats and resizes from its own edge; no shadow margin
//!   (compositors that do not draw it showed what was behind the window there).
//! - **Overlay**: one view of the app's over everything ([`Shell::set_overlay`]: a help sheet, a
//!   dialog).
//!
//! The frame adds its own commands to the registry (`shell.search`, `shell.settings`,
//! `shell.theme`, `shell.workspace.<id>`), so they are in the search and an agent's tools too.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity,
    Window, div, px,
};
use kitchen_ui::theme::{self, Look, color};
use kitchen_ui::widgets::{AccentChanged, AccentPicker, MenuSelect, icon_button, row, stepper};
use serde_json::Value;

use crate::command::{CommandSpec, MenuItem, MenuSpec, Registry, RunCommand};
use crate::jobs;
use crate::search::{Search, SearchEvent};

/// Height of the title bar (px).
pub const TITLE_BAR_H: f32 = 36.;
/// Height of the menu row (px).
pub const MENU_ROW_H: f32 = 32.;
/// Height of the status bar (px): one 24 px target.
pub const STATUS_BAR_H: f32 = 24.;
/// The band at the window's own edge that resizes it while it floats (px).
pub const RESIZE_BAND: f32 = 6.;
/// Rounded corners of the window while it floats (px).
pub const WINDOW_RADIUS: f32 = 10.;

/// A workspace of the app (Explore, Pipeline, Analyze; Sorting…).
pub struct WorkspaceSpec {
    /// Its id (`explore`): the command `shell.workspace.<id>` switches to it.
    pub id: SharedString,
    /// Its name on the switcher.
    pub title: SharedString,
    /// Its icon on the switcher.
    pub icon: IconName,
    /// What it is for (the switcher's tooltip).
    pub hint: SharedString,
    /// Its menu row.
    pub menus: Vec<MenuSpec>,
    /// What it shows.
    pub view: AnyView,
    /// A count shown on its switcher button when above zero (changed lines, unread messages).
    pub badge: Option<Rc<dyn Fn(&App) -> usize>>,
}

impl WorkspaceSpec {
    /// A workspace `id` titled `title`, showing `view`.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, icon: IconName, view: impl Into<AnyView>) -> Self {
        Self { id: id.into(), title: title.into(), icon, hint: SharedString::default(), menus: Vec::new(), view: view.into(), badge: None }
    }

    /// Sets what it is for.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Sets its menu row.
    pub fn menus(mut self, menus: Vec<MenuSpec>) -> Self {
        self.menus = menus;
        self
    }

    /// Shows `count` on its switcher button when above zero.
    pub fn badge(mut self, count: impl Fn(&App) -> usize + 'static) -> Self {
        self.badge = Some(Rc::new(count));
        self
    }
}

/// Draws a Settings section's body.
pub type SectionBody = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// A section of Settings an app registers (Compute, Models, Plugins…).
#[derive(Clone)]
pub struct SettingsSection {
    /// Its name in the list.
    pub title: SharedString,
    /// Its icon in the list.
    pub icon: IconName,
    /// Its settings.
    pub body: SectionBody,
}

impl SettingsSection {
    /// A section `title` drawing `body`.
    pub fn new(title: impl Into<SharedString>, icon: IconName, body: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        Self { title: title.into(), icon, body: Rc::new(body) }
    }
}

/// Where the app runs, at the right of the title bar (as VS Code's remote indicator): this
/// computer, or a server. Its menu lists the places.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteSpec {
    /// Where it runs now ("This computer").
    pub label: SharedString,
    /// Its icon.
    pub icon: IconName,
    /// The places to choose from (commands).
    pub menu: Vec<MenuItem>,
}

/// What an app hands the frame.
pub struct ShellSpec {
    /// The app's name: its menu's title.
    pub app_name: SharedString,
    /// Its commands.
    pub registry: Registry,
    /// The app menu's entries (files, projects…; Settings is added at its end).
    pub app_menu: Vec<MenuItem>,
    /// The Help menu's entries (the search and Settings are added).
    pub help_menu: Vec<MenuItem>,
    /// Its workspaces, in the switcher's order.
    pub workspaces: Vec<WorkspaceSpec>,
    /// Its Settings sections (after the built-in Appearance).
    pub settings: Vec<SettingsSection>,
    /// Commands shown as icon buttons at the right of the title bar (side panels…).
    pub layout_toggles: Vec<SharedString>,
    /// Where the app runs.
    pub remote: Option<RemoteSpec>,
    /// Whether the status bar shows this computer's load (off in tests).
    pub meters: bool,
    /// The app's own readings in the status bar, before the jobs (the frame's numbers, a device).
    pub status_items: Option<StatusItems>,
}

/// Draws the app's readings in the status bar.
pub type StatusItems = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

impl ShellSpec {
    /// An app named `app_name` with `registry` and `workspaces`, the rest empty.
    pub fn new(app_name: impl Into<SharedString>, registry: Registry, workspaces: Vec<WorkspaceSpec>) -> Self {
        Self {
            app_name: app_name.into(),
            registry,
            app_menu: Vec::new(),
            help_menu: Vec::new(),
            workspaces,
            settings: Vec::new(),
            layout_toggles: Vec::new(),
            remote: None,
            meters: true,
            status_items: None,
        }
    }
}

/// What the frame tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum ShellEvent {
    /// The user changed the look in the title bar or Settings: store it in the preferences.
    LookChanged(Look),
    /// The user switched workspace.
    WorkspaceChanged(SharedString),
}

/// The frame.
pub struct Shell {
    app_name: SharedString,
    registry: Registry,
    app_menu: MenuSpec,
    help_menu: MenuSpec,
    workspaces: Vec<WorkspaceSpec>,
    sections: Vec<SettingsSection>,
    layout_toggles: Vec<SharedString>,
    remote: Option<RemoteSpec>,
    status_items: Option<StatusItems>,
    overlay: Option<AnyView>,
    current: usize,
    search: Option<(Entity<Search>, Subscription)>,
    /// The Settings section shown, when Settings is open.
    settings: Option<usize>,
    accent: Entity<AccentPicker>,
    status: SharedString,
    project: Option<SharedString>,
    /// This computer's load: CPU %, memory used and installed (bytes).
    meters: Option<(f32, u64, u64)>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ShellEvent> for Shell {}

impl Shell {
    /// The frame for `spec`; adds the frame's commands to the registry and focuses the frame.
    /// Bind the keys after ([`Registry::bind_keys`]), once every command is registered.
    pub fn new(spec: ShellSpec, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let me = cx.entity().downgrade();
        spec.registry.extend(frame_commands(&me, &spec.workspaces));
        let mut app_items = spec.app_menu;
        if !app_items.is_empty() {
            app_items.push(MenuItem::Separator);
        }
        app_items.push(MenuItem::command("shell.settings"));
        let mut help_items = spec.help_menu;
        if !help_items.is_empty() {
            help_items.push(MenuItem::Separator);
        }
        help_items.push(MenuItem::command("shell.search"));
        let accent = cx.new(|cx| AccentPicker::new(theme::look().accent, window, cx));
        let mut subs = vec![cx.subscribe(&accent, |_, _, e: &AccentChanged, cx| {
            let look = theme::look().with_accent(e.0);
            theme::use_look(look, cx);
            cx.emit(ShellEvent::LookChanged(look));
            cx.notify();
        })];
        subs.push(cx.observe_window_appearance(window, |_, _, cx| cx.notify()));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let shell = Self {
            app_name: spec.app_name.clone(),
            registry: spec.registry,
            app_menu: MenuSpec::new(spec.app_name, app_items),
            help_menu: MenuSpec::new("Help", help_items),
            workspaces: spec.workspaces,
            sections: spec.settings,
            layout_toggles: spec.layout_toggles,
            remote: spec.remote,
            status_items: spec.status_items,
            overlay: None,
            current: 0,
            search: None,
            settings: None,
            accent,
            status: "Ready".into(),
            project: None,
            meters: None,
            focus,
            _subs: subs,
        };
        if let Some(w) = shell.workspaces.first() {
            crate::activity::focus(&w.title);
        }
        shell.watch_jobs(cx);
        if spec.meters {
            shell.watch_meters(cx);
        }
        shell
    }

    /// Redraws when a job starts, moves or ends.
    fn watch_jobs(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut seen = jobs::generation();
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                let now = jobs::generation();
                if now != seen {
                    seen = now;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    /// CPU and memory meters (as VS Code's resource monitor), read off the UI thread every 2 s.
    fn watch_meters(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            // No meters where the system cannot be read
            let Ok(mut sys) = sysinfo::System::new() else { return };
            loop {
                let (s, m) = cx
                    .background_executor()
                    .spawn(async move {
                        sys.refresh_cpu_usage();
                        sys.refresh_memory();
                        let m = (sys.global_cpu_usage(), sys.used_memory(), sys.total_memory());
                        (sys, m)
                    })
                    .await;
                sys = s;
                let alive = this.update(cx, |s, cx| {
                    s.meters = Some(m);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
            }
        })
        .detach();
    }

    /// The registry (the app's commands and the frame's).
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The workspace shown.
    pub fn workspace(&self) -> &WorkspaceSpec {
        &self.workspaces[self.current]
    }

    /// Shows the workspace `id`; whether there is one.
    pub fn set_workspace(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(i) = self.workspaces.iter().position(|w| w.id == id) else { return false };
        if i != self.current {
            self.current = i;
            crate::activity::focus(&self.workspaces[i].title);
            cx.emit(ShellEvent::WorkspaceChanged(self.workspaces[i].id.clone()));
        }
        cx.notify();
        true
    }

    /// The id of the workspace shown.
    pub fn workspace_id(&self) -> SharedString {
        self.workspaces[self.current].id.clone()
    }

    /// Replaces the menu row of the workspace `id`; whether there is one.
    pub fn set_menus(&mut self, id: &str, menus: Vec<MenuSpec>, cx: &mut Context<Self>) -> bool {
        let Some(w) = self.workspaces.iter_mut().find(|w| w.id == id) else { return false };
        w.menus = menus;
        cx.notify();
        true
    }

    /// Shows `view` over everything (a help sheet, a dialog), or takes it away (`None`).
    pub fn set_overlay(&mut self, view: Option<AnyView>, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = view;
        if self.overlay.is_none() {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Whether an overlay is shown.
    pub fn has_overlay(&self) -> bool {
        self.overlay.is_some()
    }

    /// Whether the Settings section `title` exists (0 is Appearance, then the app's).
    pub fn settings_index(&self, title: &str) -> Option<usize> {
        if title == "Appearance" {
            return Some(0);
        }
        self.sections.iter().position(|s| s.title == title).map(|i| i + 1)
    }

    /// Sets the status line.
    pub fn set_status(&mut self, status: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }

    /// The status line.
    pub fn status(&self) -> &SharedString {
        &self.status
    }

    /// Sets the project named in the title bar (`None`: none open).
    pub fn set_project(&mut self, project: Option<SharedString>, cx: &mut Context<Self>) {
        self.project = project;
        cx.notify();
    }

    /// The search, when open.
    pub fn search(&self) -> Option<Entity<Search>> {
        self.search.as_ref().map(|(s, _)| s.clone())
    }

    /// Opens the search over every command.
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_search_with(None, window, cx);
    }

    /// Opens the search over the commands `filter` accepts, titled `title` (Process ▸ "Find an
    /// operation…": one kind of command); every command when `None`. Open already: closes it.
    pub fn open_search_with(&mut self, filter: Option<(SharedString, crate::search::Filter)>, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        let registry = self.registry.clone();
        let search = cx.new(|cx| {
            let s = Search::new(registry, window, cx);
            match filter {
                Some((title, f)) => s.title(title).filter(move |c| f(c)),
                None => s,
            }
        });
        let sub = cx.subscribe_in(&search, window, |this, _, e: &SearchEvent, window, cx| {
            this.search = None;
            window.focus(&this.focus, cx);
            if let SearchEvent::Run(id) = e {
                // Run once this update is over: the command may change the frame itself
                let (registry, id) = (this.registry.clone(), id.clone());
                let me = cx.entity().downgrade();
                window.defer(cx, move |window, cx| {
                    if let Err(e) = registry.run_as(crate::activity::Actor::Person, "search", &id, Value::Null, window, cx) {
                        let _ = me.update(cx, |s, cx| s.set_status(format!("{id}: {e}"), cx));
                    }
                });
            }
            cx.notify();
        });
        self.search = Some((search, sub));
        cx.notify();
    }

    /// The Settings section shown, when Settings is open (0 is Appearance).
    pub fn settings_section(&self) -> Option<usize> {
        self.settings
    }

    /// Opens Settings at `section` (0: Appearance, then the app's in order), or closes it.
    pub fn show_settings(&mut self, section: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = section.map(|s| s.min(self.sections.len()));
        if self.settings.is_none() {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Sets the look and tells the app.
    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        theme::use_look(look, cx);
        cx.emit(ShellEvent::LookChanged(look));
        cx.notify();
    }

    fn title_bar(&self, rounded: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.registry.menu_button("menu-app", &MenuSpec { hint: "Files, projects, Settings".into(), ..self.app_menu.clone() });
        let help = self.registry.menu_button("menu-help", &self.help_menu);
        let switcher = h_flex().gap(px(2.)).p(px(2.)).rounded(px(theme::RADIUS)).bg(color::control().opacity(0.6)).children(self.workspaces.iter().enumerate().map(|(i, w)| {
            let id = w.id.clone();
            let count = w.badge.as_ref().map_or(0, |f| f(cx));
            let pill = (count > 0).then(|| {
                div().px(px(theme::XS)).rounded(px(theme::RADIUS_SM)).bg(color::accent()).text_color(color::on_accent()).text_xs().font_weight(FontWeight::SEMIBOLD).child(if count > 999 { "999+".to_string() } else { count.to_string() })
            });
            Button::new(SharedString::from(format!("workspace-{}", w.id)))
                .small()
                .ghost()
                .icon(w.icon)
                .label(w.title.clone())
                .children(pill)
                .tooltip(w.hint.clone())
                .selected(i == self.current)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_workspace(&id, cx);
                }))
        }));
        let toggles = self.layout_toggles.iter().filter_map(|id| self.registry.get(id)).map(|c| {
            let (registry, id) = (self.registry.clone(), c.id.clone());
            let tip = match c.key_label() {
                Some(k) => format!("{} ({k})", c.title),
                None => c.title.to_string(),
            };
            icon_button(SharedString::from(format!("toggle-{}", c.id)), c.icon, tip).selected(c.checked.as_ref().is_some_and(|f| f(cx))).on_click(move |_, window, cx| {
                let _ = registry.run_as(crate::activity::Actor::Person, "title bar", &id, Value::Null, window, cx);
            })
        });
        let remote = self.remote.clone().map(|r| {
            let (registry, items) = (self.registry.clone(), Rc::new(r.menu.clone()));
            Button::new("remote")
                .ghost()
                .small()
                .icon(r.icon)
                .label(r.label.clone())
                .tooltip("Where the app runs: this computer or a server")
                .dropdown_menu(move |m, window, cx| registry.fill_menu(m.label("Run on"), &items, window, cx))
        });
        let dark = theme::is_dark();
        let theme_button = icon_button("title-theme", if dark { IconName::Sun } else { IconName::Moon }, if dark { "Light theme" } else { "Dark theme" }).on_click(cx.listener(|this, _, _, cx| {
            let l = theme::look();
            this.set_look(l.with_dark(!l.dark), cx);
        }));
        let project = self.project.clone().unwrap_or_else(|| "No project".into());
        // GPUI clips to rectangles: the bars round their own corners inside a rounded frame
        TitleBar::new().when(rounded, |t| t.rounded_t(px(WINDOW_RADIUS - 1.0))).child(
            h_flex()
                .w_full()
                .h(px(TITLE_BAR_H))
                .gap(px(theme::MD))
                .pr(px(theme::SM))
                .items_center()
                .child(h_flex().gap(px(2.)).child(div().font_weight(FontWeight::SEMIBOLD).child(app)).child(help))
                .child(switcher)
                .child(div().flex_1())
                .child(div().min_w_0().text_sm().text_color(color::text_muted()).truncate().child(project))
                .children(remote)
                .children(toggles)
                .child(theme_button),
        )
    }

    fn menu_row(&self) -> impl IntoElement {
        let w = self.workspace();
        h_flex()
            .flex_none()
            .w_full()
            .h(px(MENU_ROW_H))
            .px(px(theme::XS))
            .gap(px(2.))
            .items_center()
            .bg(color::panel())
            .border_b_1()
            .border_color(color::border())
            .children(w.menus.iter().enumerate().map(|(i, m)| self.registry.menu_button(format!("menu-{}-{i}", w.id), m)))
    }

    fn status_bar(&self, rounded: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pct = |f: Option<f32>| f.map_or(String::new(), |f| format!(" {:.0} %", f * 100.0));
        let items = self.status_items.clone().map(|f| f(window, cx));
        let jobs = jobs::running().into_iter().map(|j| {
            let id = j.id;
            h_flex()
                .gap(px(theme::XS))
                .items_center()
                .child(div().max_w(px(320.)).truncate().text_color(color::accent_mark()).child(format!("{}{}", j.name, pct(j.progress))))
                .child(icon_button(SharedString::from(format!("job-cancel-{id}")), IconName::Close, format!("Stop: {}", j.name)).xsmall().on_click(cx.listener(move |_, _, _, cx| {
                    jobs::cancel_id(id);
                    cx.notify();
                })))
        });
        let meter = |icon: IconName, text: String, tip: &'static str| {
            h_flex().id(tip).gap(px(theme::XS)).items_center().text_color(color::text_muted()).child(Icon::new(icon).small()).child(text).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
        };
        h_flex()
            .flex_none()
            .h(px(STATUS_BAR_H))
            .px(px(theme::MD))
            .gap(px(theme::LG))
            .items_center()
            .bg(color::chrome())
            .when(rounded, |d| d.rounded_b(px(WINDOW_RADIUS - 1.0)))
            .text_xs()
            .child(div().min_w_0().flex_1().truncate().child(self.status.clone()))
            .children(items)
            .children(jobs)
            .children(self.meters.map(|(cpu, used, total)| {
                let gb = |b: u64| b as f64 / 1e9;
                h_flex()
                    .gap(px(theme::MD))
                    .child(meter(IconName::Cpu, format!("{cpu:.0} %"), "CPU in use, all cores"))
                    .child(meter(IconName::MemoryStick, format!("{:.1} / {:.1} GB", gb(used), gb(total)), "Memory in use / installed"))
            }))
    }

    fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let look = theme::look();
        let me = cx.entity().downgrade();
        let mode = MenuSelect::new("settings-theme", vec!["Dark".into(), "Light".into()], Some(if look.dark { 0 } else { 1 }), move |i, _, cx| {
            let _ = me.update(cx, |s, cx| s.set_look(theme::look().with_dark(i == 0), cx));
        });
        let me = cx.entity().downgrade();
        let size = stepper("settings-text", format!("{} px", look.font_size), move |step, _, cx| {
            let _ = me.update(cx, |s, cx| {
                let l = theme::look();
                s.set_look(l.with_font_size(l.font_size + step), cx)
            });
        });
        v_flex()
            .gap(px(theme::MD))
            .child(row("Theme", div().w_full().child(mode)))
            .child(row("Accent", self.accent.clone()))
            .child(row("Text size", size))
            .child(div().text_xs().text_color(color::text_muted()).child(format!("The accent defaults to {}'s; any colour is adjusted until text on it and its marks pass contrast.", self.app_name)))
            .into_any_element()
    }

    fn settings_view(&self, section: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut nav: Vec<(SharedString, IconName)> = vec![("Appearance".into(), IconName::Palette)];
        nav.extend(self.sections.iter().map(|s| (s.title.clone(), s.icon)));
        let body = if section == 0 { self.appearance(cx) } else { (self.sections[section - 1].body)(window, cx) };
        let close = cx.entity().downgrade();
        let list = v_flex().w(px(200.)).flex_none().gap(px(2.)).p(px(theme::SM)).border_r_1().border_color(color::border()).children(nav.iter().enumerate().map(|(i, (title, icon))| {
            Button::new(SharedString::from(format!("settings-section-{i}"))).ghost().small().w_full().justify_start().icon(*icon).label(title.clone()).selected(i == section).on_click(cx.listener(move |this, _, window, cx| this.show_settings(Some(i), window, cx)))
        }));
        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p(px(theme::XL))
            .bg(gpui_kit::black().opacity(0.35))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.show_settings(None, window, cx)))
            .child(
                v_flex()
                    .id("settings")
                    .occlude()
                    .w_full()
                    .max_w(px(820.))
                    .h_full()
                    .max_h(px(600.))
                    .rounded(px(theme::RADIUS_LG))
                    .border_1()
                    .border_color(color::border())
                    .bg(color::raised())
                    .text_color(color::text())
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .flex_none()
                            .px(px(theme::LG))
                            .py(px(theme::SM))
                            .border_b_1()
                            .border_color(color::border())
                            .child(div().flex_1().text_base().font_weight(FontWeight::SEMIBOLD).child("Settings"))
                            .child(icon_button("settings-close", IconName::Close, "Close (Esc)").on_click(move |_, window, cx| {
                                let _ = close.update(cx, |s, cx| s.show_settings(None, window, cx));
                            })),
                    )
                    .child(h_flex().flex_1().min_h_0().items_start().child(list).child(v_flex().id("settings-body").flex_1().min_w_0().h_full().overflow_y_scroll().p(px(theme::LG)).gap(px(theme::MD)).child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(nav[section].0.clone())).child(body))),
            )
            .into_any_element()
    }
}

/// The frame's own commands.
fn frame_commands(me: &WeakEntity<Shell>, workspaces: &[WorkspaceSpec]) -> Vec<CommandSpec> {
    let (m1, m2, m3) = (me.clone(), me.clone(), me.clone());
    let mut out = vec![
        CommandSpec::new("shell.search", "Search…", IconName::Search, move |window, cx| {
            let _ = m1.update(cx, |s, cx| s.open_search(window, cx));
        })
        .group("App")
        .hint("Find any command by name")
        .key("secondary-k")
        .read_only(),
        CommandSpec::new("shell.settings", "Settings…", IconName::Settings, move |window, cx| {
            let _ = m2.update(cx, |s, cx| {
                let open = if s.settings.is_some() { None } else { Some(0) };
                s.show_settings(open, window, cx)
            });
        })
        .group("App")
        .hint("Theme, accent, text size and the app's settings")
        .key("secondary-,")
        .read_only(),
        CommandSpec::new("shell.theme", "Dark theme", IconName::Moon, move |_, cx| {
            let _ = m3.update(cx, |s, cx| {
                let l = theme::look();
                s.set_look(l.with_dark(!l.dark), cx)
            });
        })
        .group("View")
        .hint("Switch between the dark and the light theme")
        .checked(|_| theme::is_dark()),
    ];
    for (i, w) in workspaces.iter().enumerate() {
        let (m, id) = (me.clone(), w.id.clone());
        let mut c = CommandSpec::new(format!("shell.workspace.{}", w.id), format!("Go to {}", w.title), w.icon, move |_, cx| {
            let _ = m.update(cx, |s, cx| s.set_workspace(&id, cx));
        })
        .group("Workspaces")
        .hint(if w.hint.is_empty() { format!("Show the {} workspace", w.title).into() } else { w.hint.clone() })
        .read_only();
        if i < 9 {
            c = c.key(format!("secondary-{}", i + 1));
        }
        out.push(c);
    }
    out
}

/// The edge a press at `p` (window px) resizes, within [`RESIZE_BAND`] of a `size` window's edge.
pub fn resize_edge(p: gpui_kit::Point<gpui_kit::Pixels>, size: gpui_kit::Size<gpui_kit::Pixels>) -> Option<gpui_kit::ResizeEdge> {
    use gpui_kit::ResizeEdge as E;
    let band = RESIZE_BAND;
    let (x, y, w, h) = (p.x.as_f32(), p.y.as_f32(), size.width.as_f32(), size.height.as_f32());
    let (l, r, t, b) = (x < band, x > w - band, y < band, y > h - band);
    Some(match (l, r, t, b) {
        (true, _, true, _) => E::TopLeft,
        (_, true, true, _) => E::TopRight,
        (true, _, _, true) => E::BottomLeft,
        (_, true, _, true) => E::BottomRight,
        (true, ..) => E::Left,
        (_, true, ..) => E::Right,
        (_, _, true, _) => E::Top,
        (_, _, _, true) => E::Bottom,
        _ => return None,
    })
}

/// Whether the window draws its own rounded frame: client-side decorations on a floating window
/// (square when maximised, full screen or tiled).
pub fn frame_rounded(window: &Window) -> bool {
    match window.window_decorations() {
        gpui_kit::Decorations::Client { tiling } => !(tiling.top || tiling.left || tiling.right || tiling.bottom) && !window.is_maximized() && !window.is_fullscreen(),
        gpui_kit::Decorations::Server => false,
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = self.registry.clone();
        let settings = self.settings.map(|s| self.settings_view(s, window, cx));
        // Linux draws the frame itself (client-side decorations): no inset for a shadow margin
        if matches!(window.window_decorations(), gpui_kit::Decorations::Client { .. }) {
            window.set_client_inset(px(0.0));
        }
        let rounded = frame_rounded(window);
        let shell = v_flex()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            // The chrome under everything: the panes are cards on it
            .bg(color::chrome())
            .text_color(color::text())
            // Not a listener: a command may update the frame itself
            .on_action(move |a: &RunCommand, window, cx| {
                if let Err(e) = registry.run_as(crate::activity::Actor::Person, "key", &a.id, Value::Null, window, cx) {
                    eprintln!("{}: {e}", a.id);
                }
            })
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" && this.settings.is_some() {
                    this.show_settings(None, window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.title_bar(rounded, cx))
            .child(self.menu_row())
            .child(div().flex_1().min_h_0().p(px(theme::PANE_GAP)).child(div().size_full().rounded(px(theme::RADIUS_LG)).overflow_hidden().bg(color::panel()).child(self.workspace().view.clone())))
            .child(self.status_bar(rounded, window, cx))
            .children(self.overlay.clone())
            .children(settings)
            .children(self.search.as_ref().map(|(s, _)| s.clone()))
            // No outline: the compositor's shadow sets a floating window apart
            .when(rounded, |d| d.rounded(px(WINDOW_RADIUS)).overflow_hidden());
        div()
            .id("window-frame")
            .size_full()
            .when(rounded, |d| {
                d.on_mouse_down(MouseButton::Left, |e, window, _| {
                    if let Some(edge) = resize_edge(e.position, window.viewport_size()) {
                        window.start_window_resize(edge);
                    }
                })
            })
            .child(shell)
    }
}
