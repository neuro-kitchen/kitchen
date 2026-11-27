//! A small app for the gallery: its state, its commands and its menus, registered once.

use gpui_kit::assets::IconName;
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window, div, px};
use gpui_kit::component::v_flex;
use gpui_kit::FontWeight;
use kitchen_shell::{CommandSpec, MenuItem, MenuSpec, jobs};
use kitchen_ui::theme::{self, color};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

/// What the demo app holds: a log of what ran, whether something changed since the save, and
/// whether its side panel shows.
#[derive(Default)]
pub struct DemoState {
    pub log: Vec<String>,
    pub dirty: bool,
    pub side_panel: bool,
}

impl DemoState {
    pub fn note(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }
}

/// The demo state as an entity (commands change it; views redraw).
pub struct DemoStateHolder(pub DemoState);

#[derive(Deserialize, JsonSchema)]
pub struct ThresholdArgs {
    /// The level (0–1) above which pixels are kept.
    pub level: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct BlurArgs {
    /// The blur's standard deviation in pixels (default 2).
    #[serde(default)]
    pub sigma: Option<f64>,
}

fn edit(s: &WeakEntity<DemoStateHolder>, cx: &mut App, f: impl FnOnce(&mut DemoState)) {
    let _ = s.update(cx, |h, cx| {
        f(&mut h.0);
        cx.notify();
    });
}

/// The demo's commands; each acts on `state`.
pub fn commands(state: &Entity<DemoStateHolder>) -> Vec<CommandSpec> {
    let s = state.downgrade();
    let (s1, s2, s3, s4, s5, s6, s7) = (s.clone(), s.clone(), s.clone(), s.clone(), s.clone(), s.clone(), s.clone());
    let (r1, r2, r3) = (s.clone(), s.clone(), s.clone());
    vec![
        CommandSpec::new("file.open", "Open image…", IconName::FolderOpen, move |_, cx| {
            edit(&s1, cx, |d| {
                d.note("Opened sample.ome.zarr");
                d.dirty = true;
            })
        })
        .group("File")
        .hint("Open an image or an OME-Zarr container")
        .key("secondary-o"),
        CommandSpec::new("file.save", "Save project", IconName::Save, move |_, cx| {
            edit(&s2, cx, |d| {
                d.note("Saved the project");
                d.dirty = false;
            })
        })
        .group("File")
        .hint("Write project.json and pipeline.json")
        .key("secondary-s")
        .enabled(move |cx| match r1.upgrade().map(|h| h.read(cx).0.dirty) {
            Some(true) => Ok(()),
            _ => Err("Nothing changed since the last save".into()),
        }),
        CommandSpec::new("edit.undo", "Undo", IconName::Undo2, move |_, cx| {
            edit(&s3, cx, |d| {
                let last = d.log.pop();
                d.note(format!("Undid: {}", last.unwrap_or_default()));
            })
        })
        .group("Edit")
        .hint("Take back the last change")
        .key("secondary-z")
        .enabled(move |cx| match r2.upgrade().map(|h| h.read(cx).0.log.is_empty()) {
            Some(false) => Ok(()),
            _ => Err("Nothing to undo".into()),
        }),
        CommandSpec::new("view.side_panel", "Side panel", IconName::PanelRight, move |_, cx| edit(&s4, cx, |d| d.side_panel = !d.side_panel))
            .group("View")
            .hint("Show or hide the side panel")
            .key("secondary-]")
            .checked(move |cx| r3.upgrade().is_some_and(|h| h.read(cx).0.side_panel))
            .read_only(),
        CommandSpec::with_args::<ThresholdArgs>("process.threshold", "Threshold", IconName::SquareSplitHorizontal, move |a, _, cx| {
            if !(0.0..=1.0).contains(&a.level) {
                return Err(format!("level {} is outside 0–1", a.level));
            }
            edit(&s5, cx, |d| {
                d.note(format!("Threshold at {}", a.level));
                d.dirty = true;
            });
            Ok(json!({ "kept_fraction": 1.0 - a.level }))
        })
        .group("Process")
        .hint("Keep the pixels above a level"),
        CommandSpec::with_args::<BlurArgs>("process.blur", "Gaussian blur", IconName::Droplet, move |a, _, cx| {
            let sigma = a.sigma.unwrap_or(2.0);
            edit(&s6, cx, |d| {
                d.note(format!("Blur, sigma {sigma}"));
                d.dirty = true;
            });
            Ok(json!({ "sigma": sigma }))
        })
        .group("Process")
        .hint("Smooth the image with a Gaussian of standard deviation sigma")
        .key("secondary-shift-b"),
        CommandSpec::new("process.long", "Find cells (a long job)", IconName::Cpu, move |_, cx| {
            match jobs::start("demo", "Finding cells", false) {
                Ok(job) => {
                    edit(&s7, cx, |d| d.note("Started a job: watch the status bar"));
                    // The work runs off the UI thread and reports through its handle
                    std::thread::spawn(move || {
                        for tile in 1..=40 {
                            if job.cancelled() {
                                return;
                            }
                            job.set(&format!("Finding cells: tile {tile} of 40"), Some(tile as f32 / 40.0));
                            std::thread::sleep(std::time::Duration::from_millis(150));
                        }
                    });
                }
                Err(e) => edit(&s7, cx, |d| d.note(e)),
            }
        })
        .group("Process")
        .hint("A job with progress and a cancel button in the status bar"),
    ]
}

/// The menus of the Commands workspace, by command id.
pub fn menus() -> Vec<MenuSpec> {
    vec![
        MenuSpec::new("Edit", vec![MenuItem::command("edit.undo")]),
        MenuSpec::new("View", vec![MenuItem::command("view.side_panel"), MenuItem::command("shell.theme"), MenuItem::Separator, MenuItem::command("shell.search")]),
        MenuSpec::new(
            "Process",
            vec![MenuItem::Label("Filters".into()), MenuItem::submenu("Smooth", vec![MenuItem::command("process.blur")]), MenuItem::command("process.threshold"), MenuItem::Separator, MenuItem::command("process.long")],
        ),
    ]
}

/// The Commands workspace: what ran.
pub struct LogPage {
    pub state: Entity<DemoStateHolder>,
}

impl LogPage {
    pub fn new(state: Entity<DemoStateHolder>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for LogPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.state.read(cx).0;
        let lines = if s.log.is_empty() { vec!["Nothing ran yet. Try the menus, Ctrl+K, or Ctrl+O.".to_string()] } else { s.log.clone() };
        v_flex()
            .size_full()
            .p(px(theme::LG))
            .gap(px(theme::SM))
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Commands"))
            .child(div().text_sm().text_color(color::text_muted()).child("One registry: the menus, the search (Ctrl+K), the keys and the title bar toggles come from the same list."))
            .child(div().text_sm().text_color(color::text_muted()).child(if s.side_panel { "Side panel: shown" } else { "Side panel: hidden" }))
            .children(lines.into_iter().map(|l| div().text_sm().font_family("monospace").child(l)))
    }
}

/// A new demo state.
pub fn state(cx: &mut App) -> Entity<DemoStateHolder> {
    cx.new(|_| DemoStateHolder(DemoState::default()))
}
