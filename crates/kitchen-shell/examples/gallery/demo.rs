//! A small app for the gallery: its state, its commands and its menus, registered once.

use gpui_kit::assets::IconName;
use gpui_kit::{App, Entity, WeakEntity};
use kitchen_shell::{CommandSpec, MenuItem, MenuSpec, Registry};
use kitchen_ui::theme;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

/// What the demo app holds: a log of what ran and whether something changed since the save.
#[derive(Default)]
pub struct DemoState {
    pub log: Vec<String>,
    pub dirty: bool,
    pub search_open: bool,
}

impl DemoState {
    pub fn note(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }
}

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

/// The demo's commands; each acts on `state` (and tells `notify` to redraw).
pub fn commands(state: &Entity<DemoStateHolder>) -> Vec<CommandSpec> {
    let s = state.downgrade();
    let edit = move |s: &WeakEntity<DemoStateHolder>, cx: &mut App, f: &dyn Fn(&mut DemoState)| {
        let _ = s.update(cx, |h, cx| {
            f(&mut h.0);
            cx.notify();
        });
    };
    let (s1, s2, s3, s4, s5, s6, s7) = (s.clone(), s.clone(), s.clone(), s.clone(), s.clone(), s.clone(), s.clone());
    let (r1, r2) = (s.clone(), s.clone());
    vec![
        CommandSpec::new("file.open", "Open image…", IconName::FolderOpen, move |_, cx| {
            edit(&s1, cx, &|d| {
                d.note("Opened sample.ome.zarr");
                d.dirty = true;
            })
        })
        .group("File")
        .hint("Open an image or an OME-Zarr container")
        .key("secondary-o"),
        CommandSpec::new("file.save", "Save project", IconName::Save, move |_, cx| {
            edit(&s2, cx, &|d| {
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
            edit(&s3, cx, &|d| {
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
        CommandSpec::new("view.theme", "Dark theme", IconName::Moon, |_, cx| {
            let l = theme::look();
            theme::use_look(l.with_dark(!l.dark), cx);
        })
        .group("View")
        .hint("Switch between the dark and the light theme")
        .key("secondary-shift-t")
        .checked(|_| theme::is_dark())
        .read_only(),
        CommandSpec::with_args::<ThresholdArgs>("process.threshold", "Threshold", IconName::SquareSplitHorizontal, move |a, _, cx| {
            if !(0.0..=1.0).contains(&a.level) {
                return Err(format!("level {} is outside 0–1", a.level));
            }
            edit(&s4, cx, &|d| {
                d.note(format!("Threshold at {}", a.level));
                d.dirty = true;
            });
            Ok(json!({ "kept_fraction": 1.0 - a.level }))
        })
        .group("Process")
        .hint("Keep the pixels above a level"),
        CommandSpec::with_args::<BlurArgs>("process.blur", "Gaussian blur", IconName::Droplet, move |a, _, cx| {
            let sigma = a.sigma.unwrap_or(2.0);
            edit(&s5, cx, &|d| {
                d.note(format!("Blur, sigma {sigma}"));
                d.dirty = true;
            });
            Ok(json!({ "sigma": sigma }))
        })
        .group("Process")
        .hint("Smooth the image with a Gaussian of standard deviation sigma")
        .key("secondary-shift-b"),
        CommandSpec::new("app.search", "Search…", IconName::Search, move |_, cx| {
            edit(&s6, cx, &|d| d.search_open = true);
        })
        .group("App")
        .hint("Find any command by name")
        .key("secondary-k")
        .read_only(),
        CommandSpec::new("help.keys", "Keyboard shortcuts", IconName::Keyboard, move |_, cx| edit(&s7, cx, &|d| d.note("Shortcuts: Ctrl+K search, Ctrl+O open, Ctrl+S save")))
            .group("Help")
            .hint("Every key and what it does")
            .key("f1")
            .read_only(),
    ]
}

/// The demo's menus, by command id.
pub fn menus() -> Vec<MenuSpec> {
    vec![
        MenuSpec::new("File", vec![MenuItem::command("file.open"), MenuItem::command("file.save")]).hint("Images and projects"),
        MenuSpec::new("Edit", vec![MenuItem::command("edit.undo")]),
        MenuSpec::new("View", vec![MenuItem::command("view.theme"), MenuItem::Separator, MenuItem::command("app.search")]),
        MenuSpec::new("Process", vec![MenuItem::Label("Filters".into()), MenuItem::submenu("Smooth", vec![MenuItem::command("process.blur")]), MenuItem::command("process.threshold")]),
    ]
}

/// The demo state as an entity (so commands can change it and views redraw).
pub struct DemoStateHolder(pub DemoState);

/// A registry of the demo's commands.
pub fn registry(state: &Entity<DemoStateHolder>) -> Registry {
    Registry::new(commands(state))
}
