//! A stand-in dsp-kitchen in the shared frame: the worked example of the Design System's frame.
//! Four views (Explore, Sorting, Curation, Analysis), the shared Pipeline and Code, File → Open…
//! with the kinds dsp-kitchen reads, a menu row per screen, and Explore's side pane that moves
//! into the Panels drawer on a narrow window. The screens are placeholders: the frame is what is
//! shown.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, WindowOptions, div, px};
use kitchen_shell::frame::fit;
use kitchen_shell::{CommandSpec, FileKind, MenuItem, MenuSpec, OpenSpec, Registry, Shell, ShellSpec, WorkspaceSpec};
use kitchen_ui::icons::standard;
use kitchen_ui::theme::{self, color};

/// The stand-in's parts, for tests.
pub struct Dspk {
    pub shell: Entity<Shell>,
    pub registry: Registry,
    /// What File → Open… opened, in order.
    pub opened: Rc<RefCell<Vec<PathBuf>>>,
    /// The commands the menus ran, in order.
    pub ran: Rc<RefCell<Vec<String>>>,
}

/// A placeholder screen: a title, what it will hold, and (Explore) a side pane that the drawer
/// takes over on a narrow window.
pub struct Screen {
    title: SharedString,
    lines: Vec<&'static str>,
    side: Option<&'static str>,
}

impl Screen {
    fn new(title: &'static str, lines: Vec<&'static str>, side: Option<&'static str>) -> Self {
        Self { title: title.into(), lines, side }
    }
}

fn pane(title: impl Into<SharedString>) -> gpui_kit::Div {
    v_flex().gap(px(theme::SM)).p(px(theme::LG)).child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(title.into()))
}

impl Render for Screen {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let f = fit(window);
        let body = pane(self.title.clone()).flex_1().min_w_0().children(self.lines.iter().map(|l| div().text_sm().text_color(color::text_muted()).child(*l)));
        // The side pane: beside the body when wide, in the Panels drawer when not
        let side = self.side.filter(|_| !f.drawer()).map(|s| pane("Channels").w(px(240.)).flex_none().border_r_1().border_color(color::border()).child(div().text_sm().text_color(color::text_muted()).child(s)));
        // Narrow: panes stack
        let row = if f.folded() { v_flex().size_full() } else { h_flex().size_full() };
        row.children(side).child(body)
    }
}

/// The drawer's copy of Explore's side pane.
pub struct Side;

impl Render for Side {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        pane("Channels").child(div().text_sm().text_color(color::text_muted()).child("384 channels, by shank"))
    }
}

/// A placeholder screen as an entity.
fn screen(cx: &mut App, title: &'static str, lines: Vec<&'static str>, side: Option<&'static str>) -> Entity<Screen> {
    cx.new(|_| Screen::new(title, lines, side))
}

/// A command that only records that it ran (the stand-in does nothing real).
fn noop(id: &'static str, title: &'static str, ran: &Rc<RefCell<Vec<String>>>) -> CommandSpec {
    let ran = ran.clone();
    CommandSpec::new(id, title, gpui_kit::assets::IconName::Dash, move |_, _| ran.borrow_mut().push(id.to_string())).group("dspk")
}

/// What dsp-kitchen opens.
pub fn kinds() -> Vec<FileKind> {
    vec![
        FileKind::new("Recording", vec!["bin", "dat", "nwb"]),
        FileKind::folder("Zarr store", &["zarr"]),
        FileKind::new("Probe", vec!["json", "prb"]),
        FileKind::new("Video", vec!["mp4", "avi"]).icon(standard::FILE_IMAGE),
    ]
}

/// Opens the stand-in in a window.
pub fn open(options: WindowOptions, meters: bool, cx: &mut App) -> (AnyWindowHandle, Dspk) {
    let opened = Rc::new(RefCell::new(Vec::new()));
    let ran = Rc::new(RefCell::new(Vec::new()));
    let mut registry_out = None;
    let (o, r) = (opened.clone(), ran.clone());
    let (window, shell) = gpui_kit::open_window(options, cx, |window, cx| {
        let registry = Registry::new(
            [
                ("file.recent.clear", "Clear recent"),
                ("file.save", "Save"),
                ("file.export", "Export…"),
                ("edit.undo", "Undo"),
                ("edit.redo", "Redo"),
                ("explore.filter", "Filter…"),
                ("explore.reference", "Reference…"),
                ("sorting.run", "Run sorter"),
                ("curation.accept", "Accept"),
                ("curation.reject", "Reject"),
                ("analysis.export", "Export table…"),
                ("pipeline.add", "Add operation…"),
                ("pipeline.plugins", "Manage plugins…"),
                ("code.new", "New script"),
            ]
            .into_iter()
            .map(|(id, t)| noop(id, t, &r))
            .collect(),
        );
        let explore = screen(cx, "Explore", vec!["Traces on the canvas, the timeline under them."], Some("384 channels, by shank"));
        let side = cx.new(|_| Side);
        let menus = |items: Vec<(&'static str, Vec<&'static str>)>| items.into_iter().map(|(t, ids)| MenuSpec::new(t, ids.into_iter().map(MenuItem::command).collect())).collect::<Vec<_>>();
        let workspaces = vec![
            WorkspaceSpec::new("explore", "Explore", standard::EXPLORE, explore).hint("Browse the recording").menus(menus(vec![("Signal", vec!["explore.filter", "explore.reference"])])).drawer(side),
            WorkspaceSpec::new("sorting", "Sorting", standard::SORTING, screen(cx, "Sorting", vec!["Sorters, their runs and results."], None)).hint("Find the units").menus(menus(vec![("Sort", vec!["sorting.run"])])),
            WorkspaceSpec::new("curation", "Curation", standard::CURATION, screen(cx, "Curation", vec!["Units one by one: accept or reject."], None)).hint("Keep the good units").menus(menus(vec![("Unit", vec!["curation.accept", "curation.reject"])])),
            WorkspaceSpec::new("analysis", "Analysis", standard::ANALYSIS, screen(cx, "Analysis", vec!["Tables and figures of the kept units."], None)).hint("Results").menus(menus(vec![("Table", vec!["analysis.export"])])),
            WorkspaceSpec::pipeline(screen(cx, "Pipeline", vec!["dsp-kitchen's graph editor goes here."], None)).menus(menus(vec![("Operations", vec!["pipeline.add"]), ("Plugins", vec!["pipeline.plugins"])])),
            WorkspaceSpec::code(screen(cx, "Code", vec!["kitchen-code's CodeWorkspace goes here."], None)).menus(menus(vec![("Script", vec!["code.new"])])),
        ];
        let mut spec = ShellSpec::new("dsp-kitchen", registry.clone(), workspaces);
        let opened = o.clone();
        spec.open = Some(OpenSpec::new(kinds(), move |path, _, _| opened.borrow_mut().push(path)));
        spec.file_menu = vec![MenuItem::submenu("Open recent", vec![MenuItem::command("file.recent.clear")]), MenuItem::command("file.save"), MenuItem::command("file.export")];
        spec.edit_menu = vec![MenuItem::command("edit.undo"), MenuItem::command("edit.redo")];
        spec.help_menu = vec![MenuItem::Label("dsp-kitchen (stand-in)".into())];
        spec.meters = meters;
        registry_out = Some(registry);
        cx.new(|cx| Shell::new(spec, window, cx))
    })
    .expect("failed to open the window");
    let registry = registry_out.expect("built");
    for bad in registry.bind_keys(None, cx) {
        eprintln!("key not bound: {bad}");
    }
    (window, Dspk { shell, registry, opened, ran })
}
