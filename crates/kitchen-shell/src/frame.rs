//! The frame every app shares ([`Shell`]): an app hands it its registry, its workspaces, its
//! menus and its Settings sections ([`ShellSpec`]), and the frame draws them the same way in
//! every app (Design System, "The frame"):
//!
//! - **Title bar**, left to right: the app's mark, the general menus (File · Edit · View · Help),
//!   a divider, then the app's views in one switcher (Explore, Sorting, Curation, Analysis…).
//!   At the right: search, the project, who works, where the app runs, the shared workspaces as
//!   icons (Pipeline, Code: the same in every app, [`Placement::Shared`]), the layout toggles and
//!   the theme. While a shared workspace is open the switcher shows no view as current.
//! - **File** keeps its usual items: Open… (the file picker, [`crate::picker_view`], only when the
//!   app says what it opens: [`OpenSpec`]), the app's own (Open recent, Save, Export…), Settings
//!   and Quit.
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
//! The frame adapts to the window's own width ([`TitleFit`]): under [`TITLE_COMPACT_W`] the
//! views show as icons; under [`PANELS_W`] a workspace's side panes move into the Panels drawer
//! ([`WorkspaceSpec::drawer`]; the app asks [`fit`] and leaves them out of its own layout); under
//! [`TITLE_NARROW_W`] the four general menus fold into one menu button, and the layout toggles and
//! the theme into a "More" menu.
//!
//! The frame adds its own commands to the registry (`shell.search`, `shell.settings`,
//! `shell.theme`, `shell.open`, `shell.quit`, `shell.panels`, `shell.workspace.<id>`), so they are
//! in the search and an agent's tools too. Keys: Ctrl/⌘+1…9 the views in order, Ctrl/⌘+Alt+P
//! Pipeline, Ctrl/⌘+Alt+C Code, Ctrl/⌘+O Open…, Ctrl/⌘+\ the Panels drawer.

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
use kitchen_ui::icons::standard;
use kitchen_ui::theme::{self, Look, color};
use kitchen_ui::widgets::{AccentChanged, AccentPicker, MenuSelect, icon_button, row, stepper};
use serde_json::Value;

use crate::command::{CommandSpec, MenuItem, MenuSpec, Registry, RunCommand};
use crate::jobs;
use crate::picker::{FileKind, Picker};
use crate::picker_view::{FilePickerView, PickerEvent};
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
/// Below this window width (px) the views show only their icons, the title bar drops the
/// project name and shows the remote as an icon (a small laptop window, a tablet held upright).
pub const TITLE_COMPACT_W: f32 = 1100.;
/// Below this window width (px) a workspace's side panes move into the Panels drawer.
pub const PANELS_W: f32 = 900.;
/// Below this window width (px) the general menus fold into one menu button, and the layout
/// toggles and the theme button into one "More" menu (a phone held upright).
pub const TITLE_NARROW_W: f32 = 560.;

/// How much of the frame fits a window `width` px wide (see [`TITLE_COMPACT_W`], [`PANELS_W`],
/// [`TITLE_NARROW_W`]). Ordered: `Narrow < Medium < Compact < Full`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TitleFit {
    /// Under [`TITLE_NARROW_W`]: one menu button, icons, a "More" menu; panes stack.
    Narrow,
    /// Under [`PANELS_W`]: side panes in the Panels drawer.
    Medium,
    /// Under [`TITLE_COMPACT_W`]: views as icons, no project name, the remote as an icon.
    Compact,
    /// Everything with its label.
    Full,
}

impl TitleFit {
    /// The fit for a window `width` px wide.
    pub fn of(width: f32) -> Self {
        if width < TITLE_NARROW_W {
            TitleFit::Narrow
        } else if width < PANELS_W {
            TitleFit::Medium
        } else if width < TITLE_COMPACT_W {
            TitleFit::Compact
        } else {
            TitleFit::Full
        }
    }

    /// Whether labels show next to the icons (views, the remote).
    pub fn labels(self) -> bool {
        self == TitleFit::Full
    }

    /// Whether side panes go into the Panels drawer.
    pub fn drawer(self) -> bool {
        self <= TitleFit::Medium
    }

    /// Whether the general menus fold into one button.
    pub fn folded(self) -> bool {
        self == TitleFit::Narrow
    }
}

/// The fit of `window` ([`TitleFit::of`] its width): a workspace's view asks it to leave its side
/// panes out (they are in the drawer) and to stack its panes when narrow.
pub fn fit(window: &Window) -> TitleFit {
    TitleFit::of(window.viewport_size().width.as_f32())
}

/// Where a workspace sits in the title bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// One of the app's views, in the switcher (Explore, Sorting, Curation, Analysis).
    #[default]
    View,
    /// A workspace every app has, as an icon at the right (Pipeline, Code).
    Shared,
}

/// The shared Pipeline workspace's id.
pub const PIPELINE_ID: &str = "pipeline";
/// The shared Code workspace's id.
pub const CODE_ID: &str = "code";
/// Pipeline's key: Ctrl/⌘+Alt+P.
pub const PIPELINE_KEY: &str = "secondary-alt-p";
/// Code's key: Ctrl/⌘+Alt+C.
pub const CODE_KEY: &str = "secondary-alt-c";

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
    /// Where it sits: a view of the app's, or shared.
    pub placement: Placement,
    /// Its key (views get Ctrl/⌘+1…9 in order without one).
    pub key: Option<SharedString>,
    /// Its side panes, shown in the Panels drawer when the window is narrow ([`TitleFit::drawer`]).
    pub drawer: Option<AnyView>,
}

impl WorkspaceSpec {
    /// A workspace `id` titled `title`, showing `view`.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, icon: IconName, view: impl Into<AnyView>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon,
            hint: SharedString::default(),
            menus: Vec::new(),
            view: view.into(),
            badge: None,
            placement: Placement::View,
            key: None,
            drawer: None,
        }
    }

    /// The shared Pipeline workspace: the app's pipeline screen in `view`. The app fills its
    /// Operations and Plugins menus ([`WorkspaceSpec::menus`]).
    pub fn pipeline(view: impl Into<AnyView>) -> Self {
        WorkspaceSpec::new(PIPELINE_ID, "Pipeline", standard::PIPELINE, view).hint("The steps from raw data to results, as a graph").shared().key(PIPELINE_KEY)
    }

    /// The shared Code workspace (kitchen-code's `CodeWorkspace` in `view`).
    pub fn code(view: impl Into<AnyView>) -> Self {
        WorkspaceSpec::new(CODE_ID, "Code", standard::CODE, view).hint("Scripts: write them, run them, add them to the pipeline").shared().key(CODE_KEY)
    }

    /// Makes it shared: an icon at the right of the title bar, not a view in the switcher.
    pub fn shared(mut self) -> Self {
        self.placement = Placement::Shared;
        self
    }

    /// Gives it a key (`secondary-alt-p`).
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Its side panes, for the Panels drawer of a narrow window. The workspace's view leaves them
    /// out of its own layout when [`fit`] says [`TitleFit::drawer`].
    pub fn drawer(mut self, view: impl Into<AnyView>) -> Self {
        self.drawer = Some(view.into());
        self
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

/// What File → Open… opens: the kinds the app reads, where to start, and what to do with the
/// file picked.
#[derive(Clone)]
pub struct OpenSpec {
    /// What the app opens (an empty list: any file).
    pub kinds: Vec<FileKind>,
    /// The folder the picker starts in (home when `None` or unreadable).
    pub start: Option<std::path::PathBuf>,
    /// The app's recent files, newest first, listed under Places.
    pub recent: Rc<dyn Fn(&App) -> Vec<std::path::PathBuf>>,
    /// Opens what was picked.
    pub open: Rc<dyn Fn(std::path::PathBuf, &mut Window, &mut App)>,
}

impl OpenSpec {
    /// Opens `kinds` with `open`; no recent files, starting at home.
    pub fn new(kinds: Vec<FileKind>, open: impl Fn(std::path::PathBuf, &mut Window, &mut App) + 'static) -> Self {
        Self { kinds, start: None, recent: Rc::new(|_| Vec::new()), open: Rc::new(open) }
    }

    /// Lists the app's recent files under Places.
    pub fn recent(mut self, recent: impl Fn(&App) -> Vec<std::path::PathBuf> + 'static) -> Self {
        self.recent = Rc::new(recent);
        self
    }

    /// Starts the picker in `dir`.
    pub fn start(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.start = Some(dir.into());
        self
    }
}

/// What an app hands the frame.
pub struct ShellSpec {
    /// The app's name: its mark at the left of the title bar, and the window's.
    pub app_name: SharedString,
    /// Its commands.
    pub registry: Registry,
    /// What File → Open… opens; no Open… item without it.
    pub open: Option<OpenSpec>,
    /// File's entries after Open… (Open recent, Save, Export…; Settings and Quit are added).
    pub file_menu: Vec<MenuItem>,
    /// Older name of [`ShellSpec::file_menu`]: its entries follow `file_menu`'s.
    pub app_menu: Vec<MenuItem>,
    /// Edit's entries (Undo, Redo, Copy…); no Edit menu when empty.
    pub edit_menu: Vec<MenuItem>,
    /// View's entries (the layout toggles and the theme are added).
    pub view_menu: Vec<MenuItem>,
    /// The Help menu's entries (the search is added).
    pub help_menu: Vec<MenuItem>,
    /// Its workspaces, in the switcher's order.
    pub workspaces: Vec<WorkspaceSpec>,
    /// Its Settings sections (after the built-in Appearance).
    pub settings: Vec<SettingsSection>,
    /// Commands shown as icon buttons at the right of the title bar (side panels…).
    pub layout_toggles: Vec<SharedString>,
    /// Where the app runs.
    pub remote: Option<RemoteSpec>,
    /// Whether the app records who works ([`crate::people`]): the title bar's person button and a
    /// People section at the end of Settings. The app [`crate::people::load`]s the people first.
    pub people: bool,
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
            open: None,
            file_menu: Vec::new(),
            app_menu: Vec::new(),
            edit_menu: Vec::new(),
            view_menu: Vec::new(),
            help_menu: Vec::new(),
            workspaces,
            settings: Vec::new(),
            layout_toggles: Vec::new(),
            remote: None,
            people: false,
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
    file_menu: MenuSpec,
    edit_menu: Option<MenuSpec>,
    view_menu: MenuSpec,
    help_menu: MenuSpec,
    open: Option<OpenSpec>,
    picker: Option<(Entity<FilePickerView>, Subscription)>,
    /// Whether the Panels drawer is open (narrow windows).
    drawer_open: bool,
    workspaces: Vec<WorkspaceSpec>,
    sections: Vec<SettingsSection>,
    layout_toggles: Vec<SharedString>,
    remote: Option<RemoteSpec>,
    /// The People section, when the app records who works.
    people: Option<Entity<crate::people_view::PeopleSettings>>,
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
        spec.registry.extend(frame_commands(&me, &spec.workspaces, spec.open.is_some()));
        let people = spec.people.then(|| {
            spec.registry.extend(people_commands(&me));
            spec.registry.provide("people", |_| crate::people_view::menu_items());
            cx.new(|cx| crate::people_view::PeopleSettings::new(window, cx))
        });
        // File: Open…, the app's items, Settings, Quit
        let mut file_items = Vec::new();
        if spec.open.is_some() {
            file_items.push(MenuItem::command("shell.open"));
        }
        file_items.extend(spec.file_menu);
        file_items.extend(spec.app_menu);
        if !file_items.is_empty() {
            file_items.push(MenuItem::Separator);
        }
        file_items.push(MenuItem::command("shell.settings"));
        file_items.push(MenuItem::Separator);
        file_items.push(MenuItem::command("shell.quit"));
        let edit_menu = (!spec.edit_menu.is_empty()).then(|| MenuSpec::new("Edit", spec.edit_menu));
        // View: the app's items, then the layout toggles, the Panels drawer and the theme
        let mut view_items = spec.view_menu;
        if !view_items.is_empty() {
            view_items.push(MenuItem::Separator);
        }
        view_items.extend(spec.layout_toggles.iter().map(|id| MenuItem::command(id.clone())));
        if spec.workspaces.iter().any(|w| w.drawer.is_some()) {
            view_items.push(MenuItem::command("shell.panels"));
        }
        view_items.push(MenuItem::command("shell.theme"));
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
            file_menu: MenuSpec::new("File", file_items).hint("Open, save, export; Settings; Quit"),
            edit_menu,
            view_menu: MenuSpec::new("View", view_items),
            help_menu: MenuSpec::new("Help", help_items),
            open: spec.open,
            picker: None,
            drawer_open: false,
            workspaces: spec.workspaces,
            sections: spec.settings,
            layout_toggles: spec.layout_toggles,
            remote: spec.remote,
            people,
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
            self.drawer_open = false;
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

    /// The view of the app's shown, if the workspace shown is one (`None` while a shared one, Pipeline
    /// or Code, is open).
    pub fn current_view(&self) -> Option<SharedString> {
        let w = &self.workspaces[self.current];
        (w.placement == Placement::View).then(|| w.id.clone())
    }

    /// Opens the file picker (File → Open…); nothing when the app opens no files.
    pub fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.open.clone() else { return };
        let start = open.start.clone().or_else(crate::picker::home_dir).unwrap_or_else(|| std::path::PathBuf::from("/"));
        let picker = Picker::new(open.kinds.clone(), start).with_recent((open.recent)(cx));
        let view = cx.new(|cx| FilePickerView::new(picker, window, cx));
        let sub = cx.subscribe_in(&view, window, move |this, _, e: &PickerEvent, window, cx| {
            this.picker = None;
            window.focus(&this.focus, cx);
            if let PickerEvent::Open(path) = e {
                (open.open)(path.clone(), window, cx);
            }
            cx.notify();
        });
        self.picker = Some((view, sub));
        cx.notify();
    }

    /// The file picker, while it is open.
    pub fn picker(&self) -> Option<Entity<FilePickerView>> {
        self.picker.as_ref().map(|(p, _)| p.clone())
    }

    /// Opens or closes the Panels drawer.
    pub fn toggle_drawer(&mut self, cx: &mut Context<Self>) {
        self.drawer_open = !self.drawer_open && self.workspace().drawer.is_some();
        cx.notify();
    }

    /// Whether the Panels drawer is open.
    pub fn drawer_open(&self) -> bool {
        self.drawer_open
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

    /// Whether the Settings section `title` exists (0 is Appearance, then the app's, then People
    /// when the app records who works).
    pub fn settings_index(&self, title: &str) -> Option<usize> {
        if title == "Appearance" {
            return Some(0);
        }
        if title == PEOPLE && self.people.is_some() {
            return Some(self.sections.len() + 1);
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
        self.settings = section.map(|s| s.min(self.sections.len() + usize::from(self.people.is_some())));
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

    fn title_bar(&self, rounded: bool, fit: TitleFit, cx: &mut Context<Self>) -> impl IntoElement {
        // The general menus; narrow: one button holding them as submenus
        let menus: Vec<&MenuSpec> = [Some(&self.file_menu), self.edit_menu.as_ref(), Some(&self.view_menu), Some(&self.help_menu)].into_iter().flatten().collect();
        let general = if fit.folded() {
            let registry = self.registry.clone();
            let items: Rc<Vec<MenuItem>> = Rc::new(menus.iter().map(|m| MenuItem::submenu(m.title.clone(), m.items.clone())).collect());
            icon_button("title-menu", standard::MENU, "File, Edit, View, Help").dropdown_menu(move |m, window, cx| registry.fill_menu(m, &items, window, cx)).into_any_element()
        } else {
            h_flex().gap(px(2.)).children(menus.iter().map(|m| self.registry.menu_button(format!("menu-{}", m.title.to_lowercase()), m))).into_any_element()
        };
        let mark = div().id("title-mark").flex_none().px(px(theme::SM)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(color::accent_mark()).child(if fit.folded() {
            self.app_name.chars().next().map(String::from).unwrap_or_default()
        } else {
            self.app_name.to_string()
        });
        let divider = div().flex_none().w(px(1.)).h(px(16.)).bg(color::border());
        let workspace_button = |i: usize, w: &WorkspaceSpec, labelled: bool, cx: &mut Context<Self>| {
            let id = w.id.clone();
            let count = w.badge.as_ref().map_or(0, |f| f(cx));
            let pill = (count > 0).then(|| {
                div().px(px(theme::XS)).rounded(px(theme::RADIUS_SM)).bg(color::accent()).text_color(color::on_accent()).text_xs().font_weight(FontWeight::SEMIBOLD).child(if count > 999 { "999+".to_string() } else { count.to_string() })
            });
            let key = self.registry.get(&format!("shell.workspace.{}", w.id)).and_then(|c| c.key_label());
            let tip = match (&key, w.hint.is_empty()) {
                (Some(k), false) => format!("{}: {} ({k})", w.title, w.hint),
                (Some(k), true) => format!("{} ({k})", w.title),
                (None, false) => format!("{}: {}", w.title, w.hint),
                (None, true) => w.title.to_string(),
            };
            let button = Button::new(SharedString::from(format!("workspace-{}", w.id))).small().ghost().icon(w.icon).tooltip(tip);
            let button = if labelled { button.label(w.title.clone()) } else { button };
            button.children(pill).selected(i == self.current).on_click(cx.listener(move |this, _, _, cx| {
                this.set_workspace(&id, cx);
            }))
        };
        // The app's views, one detented switcher; full width: with their names
        let views: Vec<_> = self.workspaces.iter().enumerate().filter(|(_, w)| w.placement == Placement::View).map(|(i, w)| workspace_button(i, w, fit.labels(), cx)).collect();
        let switcher = (!views.is_empty()).then(|| h_flex().id("title-views").min_w_0().gap(px(2.)).p(px(2.)).rounded(px(theme::RADIUS)).bg(color::control().opacity(0.6)).children(views));
        // The shared workspaces: icons in the right corner
        let shared: Vec<_> = self.workspaces.iter().enumerate().filter(|(_, w)| w.placement == Placement::Shared).map(|(i, w)| workspace_button(i, w, false, cx)).collect();
        let shared = (!shared.is_empty()).then(|| h_flex().gap(px(2.)).children(shared));
        let toggles = self.layout_toggles.iter().filter(|_| !fit.folded()).filter_map(|id| self.registry.get(id)).map(|c| {
            let (registry, id) = (self.registry.clone(), c.id.clone());
            let tip = match c.key_label() {
                Some(k) => format!("{} ({k})", c.title),
                None => c.title.to_string(),
            };
            icon_button(SharedString::from(format!("toggle-{}", c.id)), c.icon, tip).selected(c.checked.as_ref().is_some_and(|f| f(cx))).on_click(move |_, window, cx| {
                let _ = registry.run_as(crate::activity::Actor::Person, "title bar", &id, Value::Null, window, cx);
            })
        });
        // Narrow and medium: the Panels drawer's button, when the workspace has side panes
        let panels = (fit.drawer() && self.workspace().drawer.is_some()).then(|| {
            icon_button("title-panels", standard::PANELS, "Panels (Ctrl+\\)").selected(self.drawer_open).on_click(cx.listener(|this, _, _, cx| this.toggle_drawer(cx)))
        });
        let search = icon_button("title-search", IconName::Search, "Search commands (Ctrl+K)").on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx)));
        let remote = self.remote.clone().map(|r| {
            let (registry, items) = (self.registry.clone(), Rc::new(r.menu.clone()));
            let button = Button::new("remote").ghost().small().icon(r.icon);
            // Compact: the icon only, where it runs in the tooltip
            let button = if fit.labels() { button.label(r.label.clone()).tooltip("Where the app runs: this computer or a server") } else { button.tooltip(format!("Runs on {}: this computer or a server", r.label)) };
            button.dropdown_menu(move |m, window, cx| registry.fill_menu(m.label("Run on"), &items, window, cx))
        });
        // Narrow: the layout toggles and the theme in one menu, as commands (checked when on)
        let more = fit.folded().then(|| {
            let registry = self.registry.clone();
            let items: Rc<Vec<MenuItem>> = Rc::new(self.layout_toggles.iter().map(|id| MenuItem::command(id.clone())).chain([MenuItem::command("shell.theme")]).collect());
            icon_button("title-more", IconName::Ellipsis, "Panels and theme").dropdown_menu(move |m, window, cx| registry.fill_menu(m, &items, window, cx))
        });
        let dark = theme::is_dark();
        let theme_button = (!fit.folded()).then(|| icon_button("title-theme", if dark { IconName::Sun } else { IconName::Moon }, if dark { "Light theme" } else { "Dark theme" }).on_click(cx.listener(|this, _, _, cx| {
            let l = theme::look();
            this.set_look(l.with_dark(!l.dark), cx);
        })));
        // Who works: their name (compact: the icon, the name in the tooltip); a guest says so
        let person = self.people.as_ref().map(|_| {
            let who = crate::people::current().map(|p| p.name);
            let label: SharedString = who.clone().unwrap_or_else(|| "Guest".into()).into();
            let tip = match &who {
                Some(n) => format!("Working as {n}: what you do is recorded under this name"),
                None => "Working as a guest: no name is recorded".to_string(),
            };
            let button = Button::new("title-person").ghost().small().icon(IconName::CircleUser);
            let button = if fit.labels() { button.label(label) } else { button };
            let (registry, items) = (self.registry.clone(), Rc::new(vec![MenuItem::Provided("people".into())]));
            button.tooltip(tip).dropdown_menu(move |m, window, cx| registry.fill_menu(m.label("Who works"), &items, window, cx))
        });
        // Compact and narrower: the project's name is left to the app's status bar
        let project = fit.labels().then(|| div().min_w_0().text_sm().text_color(color::text_muted()).truncate().child(self.project.clone().unwrap_or_else(|| "No project".into())));
        // GPUI clips to rectangles: the bars round their own corners inside a rounded frame
        TitleBar::new().when(rounded, |t| t.rounded_t(px(WINDOW_RADIUS - 1.0))).child(
            h_flex()
                .w_full()
                .h(px(TITLE_BAR_H))
                .gap(px(if fit.folded() { theme::XS } else { theme::SM }))
                .pr(px(theme::SM))
                .items_center()
                .children(panels)
                .child(mark)
                .child(general)
                .when(switcher.is_some(), |d| d.child(divider))
                .children(switcher)
                .child(div().flex_1())
                .child(search)
                .children(project)
                .children(person)
                .children(remote)
                .children(shared)
                .children(toggles)
                .children(theme_button)
                .children(more),
        )
    }

    fn menu_row(&self, fit: TitleFit) -> impl IntoElement {
        let w = self.workspace();
        h_flex()
            .id("menu-row")
            .flex_none()
            .w_full()
            .h(px(MENU_ROW_H))
            .px(px(theme::XS))
            .gap(px(theme::XS))
            .items_center()
            .bg(color::panel())
            .border_b_1()
            .border_color(color::border())
            // Narrow: the menus scroll sideways rather than wrap
            .when(fit.folded(), |d| d.overflow_x_scroll())
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
        if self.people.is_some() {
            nav.push((PEOPLE.into(), IconName::CircleUser));
        }
        let body = match (&self.people, section) {
            (_, 0) => self.appearance(cx),
            (Some(people), s) if s == self.sections.len() + 1 => people.update(cx, |p, cx| p.body(cx)),
            (_, s) => (self.sections[s - 1].body)(window, cx),
        };
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

/// The People section's title.
const PEOPLE: &str = "People";

/// Who works: pick a person (or a guest), open People.
fn people_commands(me: &WeakEntity<Shell>) -> Vec<CommandSpec> {
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct UseArgs {
        /// The person's name; none: a guest.
        name: Option<String>,
    }
    let (m1, m2) = (me.clone(), me.clone());
    vec![
        CommandSpec::with_args("people.use", "Work as…", IconName::CircleUser, move |a: UseArgs, _, cx| {
            crate::people::use_person(a.name.as_deref())?;
            let _ = m1.update(cx, |_, cx| cx.notify());
            Ok(Value::Null)
        })
        .group("App")
        .hint("Who works now: what is done is recorded under their name (none: a guest)"),
        CommandSpec::new("people.manage", "People…", IconName::CircleUser, move |window, cx| {
            let _ = m2.update(cx, |s, cx| {
                let i = s.settings_index(PEOPLE);
                s.show_settings(i, window, cx)
            });
        })
        .group("App")
        .hint("Add, pick and remove the people who work in the app")
        .read_only(),
    ]
}

/// The frame's own commands.
fn frame_commands(me: &WeakEntity<Shell>, workspaces: &[WorkspaceSpec], opens_files: bool) -> Vec<CommandSpec> {
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
    let (m4, m5) = (me.clone(), me.clone());
    // Open… (and its key) only for an app that opens files: an app's own command keeps Ctrl+O otherwise
    if opens_files {
        out.push(
            CommandSpec::new("shell.open", "Open…", standard::OPEN, move |window, cx| {
                let _ = m4.update(cx, |s, cx| s.open_picker(window, cx));
            })
            .group("File")
            .hint("Pick a file to open")
            .key("secondary-o")
            .read_only(),
        );
    }
    out.push(
        CommandSpec::new("shell.quit", "Quit", IconName::WindowClose, |_, cx| cx.quit())
            .group("File")
            .hint("Close the app")
            .key("secondary-q")
            .read_only(),
    );
    out.push(
        CommandSpec::new("shell.panels", "Panels", standard::PANELS, move |_, cx| {
            let _ = m5.update(cx, |s, cx| s.toggle_drawer(cx));
        })
        .group("View")
        .hint("Show the side panes of a narrow window")
        .key("secondary-\\")
        .read_only(),
    );
    // Views: Ctrl/⌘+1…9 in order; shared workspaces: their own keys
    let mut n = 0;
    for w in workspaces {
        let (m, id) = (me.clone(), w.id.clone());
        let mut c = CommandSpec::new(format!("shell.workspace.{}", w.id), format!("Go to {}", w.title), w.icon, move |_, cx| {
            let _ = m.update(cx, |s, cx| s.set_workspace(&id, cx));
        })
        .group("Workspaces")
        .hint(if w.hint.is_empty() { format!("Show the {} workspace", w.title).into() } else { w.hint.clone() })
        .read_only();
        if let Some(k) = &w.key {
            c = c.key(k.clone());
        } else if w.placement == Placement::View && n < 9 {
            n += 1;
            c = c.key(format!("secondary-{n}"));
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
        let fit = fit(window);
        // A wider window shows the side panes itself: the drawer closes
        if !fit.drawer() {
            self.drawer_open = false;
        }
        // The Panels drawer: the workspace's side panes over the body's left, a scrim beside them
        let drawer = self.workspace().drawer.clone().filter(|_| self.drawer_open && fit.drawer()).map(|view| {
            let close = cx.listener(|this, _, _, cx| {
                this.drawer_open = false;
                cx.notify();
            });
            div()
                .id("panels-drawer")
                .absolute()
                .inset_0()
                .flex()
                .child(
                    div()
                        .id("panels-drawer-pane")
                        .occlude()
                        .h_full()
                        .w(px(320.))
                        .max_w(gpui_kit::relative(0.85))
                        .bg(color::panel())
                        .border_r_1()
                        .border_color(color::border())
                        .shadow_lg()
                        .overflow_hidden()
                        .child(view),
                )
                .child(gpui_kit::TestSupportExt::test_support(div().id("panels-drawer-scrim").role(gpui_kit::Role::Button).aria_label("Close the panels").flex_1().h_full().bg(gpui_kit::black().opacity(0.3)).on_click(close)))
        });
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
                } else if e.keystroke.key == "escape" && this.drawer_open {
                    this.drawer_open = false;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(self.title_bar(rounded, fit, cx))
            .child(self.menu_row(fit))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .p(px(theme::PANE_GAP))
                    .child(div().size_full().rounded(px(theme::RADIUS_LG)).overflow_hidden().bg(color::panel()).child(self.workspace().view.clone()))
                    .children(drawer),
            )
            .child(self.status_bar(rounded, window, cx))
            // Over everything, and nothing under it takes a click meant for it
            .children(self.overlay.clone().map(|o| div().absolute().inset_0().occlude().child(o)))
            .children(settings)
            .children(self.search.as_ref().map(|(s, _)| s.clone()))
            .children(self.picker.as_ref().map(|(p, _)| p.clone()))
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
