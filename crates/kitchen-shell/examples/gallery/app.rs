//! The gallery app: the demo's commands and pages in the shared frame.

#![allow(dead_code)]

use gpui_kit::assets::IconName;
use gpui_kit::component::v_flex;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, IntoElement, ParentElement as _, Styled as _, WindowOptions, div, px};
use kitchen_shell::{MenuItem, MenuSpec, Registry, RemoteSpec, SettingsSection, Shell, ShellSpec, WorkspaceSpec};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::row;

use super::demo::{self, DemoStateHolder, LogPage};
use super::project;
use super::views::ViewsPage;

/// The gallery's parts, for tests.
pub struct Gallery {
    pub shell: Entity<Shell>,
    pub state: Entity<DemoStateHolder>,
    pub views: Entity<ViewsPage>,
    pub changes: Entity<kitchen_shell::ChangesPanel>,
    pub project: std::path::PathBuf,
    pub registry: Registry,
}

/// Opens the gallery in a window; `meters` turns the status bar's CPU and memory on.
pub fn open(options: WindowOptions, meters: bool, cx: &mut App) -> (AnyWindowHandle, Gallery) {
    let mut parts = None;
    let (window, shell) = gpui_kit::open_window(options, cx, |window, cx| {
        let state = demo::state(cx);
        let registry = Registry::new(demo::commands(&state));
        let log = cx.new(|cx| LogPage::new(state.clone(), cx));
        let views = cx.new(|cx| ViewsPage::new(window, cx));
        let dir = project::dir();
        let changes = project::panel(&dir, cx);
        registry.extend(project::commands(dir.clone(), &changes));
        let badge = changes.clone();
        let workspaces = vec![
            WorkspaceSpec::new("commands", "Commands", IconName::SquareTerminal, log).hint("Menus, search and keys from one list").menus(demo::menus()),
            WorkspaceSpec::new("views", "Views", IconName::LayoutGrid, views.clone()).hint("The view container").menus(vec![MenuSpec::new("View", vec![MenuItem::command("view.side_panel")])]),
            WorkspaceSpec::new("changes", "Changes", IconName::GitCompare, changes.clone()).hint("What changed in the project's files").menus(project::menus()).badge(move |cx| badge.read(cx).changed_lines()),
        ];
        let mut spec = ShellSpec::new("Gallery", registry.clone(), workspaces);
        spec.app_menu = vec![MenuItem::command("file.open"), MenuItem::command("file.save")];
        spec.help_menu = vec![MenuItem::Label("kitchen-shell gallery".into())];
        spec.layout_toggles = vec!["view.side_panel".into()];
        spec.remote = Some(RemoteSpec { label: "This computer".into(), icon: IconName::Monitor, menu: vec![MenuItem::Label("Servers are added in Settings".into())] });
        spec.meters = meters;
        spec.settings = vec![SettingsSection::new("Compute", IconName::Cpu, |_, _| {
            v_flex()
                .gap(px(theme::MD))
                .child(row("Device", div().text_sm().child("This computer's GPU")))
                .child(div().text_xs().text_color(color::text_muted()).child("An app's own section: it draws what it needs."))
                .into_any_element()
        })];
        let shell = cx.new(|cx| Shell::new(spec, window, cx));
        parts = Some((state, views, changes, dir, registry));
        shell
    })
    .expect("failed to open the gallery window");
    let (state, views, changes, project, registry) = parts.expect("the window was built");
    let bad = registry.bind_keys(None, cx);
    for b in bad {
        eprintln!("key not bound: {b}");
    }
    (window, Gallery { shell, state, views, changes, project, registry })
}
