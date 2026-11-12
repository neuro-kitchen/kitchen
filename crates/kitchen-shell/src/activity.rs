//! What happened in the app, as an episodic memory (kitchen-ui's [`Timeline`]): every command run
//! (from a menu, a key, the search, the title bar or an agent) with who ran it, the time spent in
//! each workspace, and every job from its start to its end. Attention, not content: a command's
//! arguments are kept in short, never the data it worked on.
//!
//! The registry and the frame record into it by themselves; an app adds what a step produced
//! ([`produced`]) so the timeline can say where a file came from. [`view`] is a timeline that
//! follows the log as it grows.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::{App, AppContext as _, Entity, SharedString};
use kitchen_ui::timeline::{Activity, Timeline};
use serde_json::Value;

/// Who did it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    /// The person at the app.
    Person,
    /// An agent (through MCP or the chat).
    Agent,
}

impl Actor {
    /// Its lane on the timeline.
    pub fn lane(self) -> &'static str {
        match self {
            Actor::Person => "You",
            Actor::Agent => "Agent",
        }
    }
}

#[derive(Default)]
struct Log {
    items: Vec<Activity>,
    /// Spans still open (a job, the workspace shown): their index; they end at "now".
    open: HashMap<String, usize>,
    workspace: String,
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn with<T>(f: impl FnOnce(&mut Log) -> T) -> T {
    let mut g = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let out = f(g.get_or_insert_with(Log::default));
    GENERATION.fetch_add(1, Ordering::Relaxed);
    out
}

/// Seconds since 1970 (UTC).
pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// Bumped on every change (redraw when it moves).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Adds an activity as it is.
pub fn record(a: Activity) {
    with(|l| l.items.push(a));
}

/// Opens a span `key` (a job, a workspace shown) that lasts until [`close`]; one open with the
/// same key is closed first.
pub fn open(key: impl Into<String>, mut a: Activity) {
    let key = key.into();
    let t = now();
    with(|l| {
        if let Some(i) = l.open.remove(&key) {
            l.items[i].end = t;
        }
        a.start = t;
        a.end = t;
        l.items.push(a);
        l.open.insert(key, l.items.len() - 1);
    });
}

/// Closes the span `key` now.
pub fn close(key: &str) {
    let t = now();
    with(|l| {
        if let Some(i) = l.open.remove(key) {
            l.items[i].end = t.max(l.items[i].start + 1.0);
        }
    });
}

/// A short line of JSON: at most `max` characters.
fn short(v: &Value, max: usize) -> String {
    let s = v.to_string();
    if s.chars().count() <= max { s } else { format!("{}…", s.chars().take(max).collect::<String>()) }
}

/// Records a command run: by whom, how (`via`: menu, key, search, title bar, agent), with what
/// arguments and what came of it.
pub fn command(actor: Actor, id: &str, title: &str, group: &str, via: &str, args: &Value, result: &Result<Value, String>) {
    let mut detail = Vec::new();
    if !args.is_null() && args != &Value::Object(Default::default()) {
        detail.push(format!("{id} {}", short(args, 120)));
    } else {
        detail.push(id.to_string());
    }
    match result {
        Ok(Value::Null) => {}
        Ok(v) => detail.push(format!("→ {}", short(v, 120))),
        Err(e) => detail.push(format!("refused: {e}")),
    }
    let t = now();
    with(|l| {
        let source = if l.workspace.is_empty() { via.to_string() } else { format!("{} · {via}", l.workspace) };
        l.items.push(Activity {
            start: t,
            end: t,
            lane: actor.lane().into(),
            kind: if group.is_empty() { "Commands".into() } else { SharedString::from(group.to_string()) },
            title: title.to_string().into(),
            detail: detail.join("  ").into(),
            source: source.into(),
            assets: Vec::new(),
        })
    });
}

/// The workspace shown now: a span on the Focus lane, until the next one.
pub fn focus(workspace: &str) {
    let name = workspace.to_string();
    with(|l| l.workspace = name.clone());
    open("focus", Activity { lane: "Focus".into(), kind: SharedString::from(name.clone()), title: format!("In {name}").into(), ..Default::default() });
}

/// A job started (its span on the Jobs lane, until [`job_ended`]).
pub fn job_started(id: u64, kind: &str, name: &str) {
    open(format!("job-{id}"), Activity { lane: "Jobs".into(), kind: kind.to_string().into(), title: name.to_string().into(), source: "job".into(), ..Default::default() });
}

/// A job ended.
pub fn job_ended(id: u64) {
    close(&format!("job-{id}"));
}

/// What a step produced (files, ids): added to the last activity titled `title`, or as one of its
/// own on the person's lane.
pub fn produced(title: &str, assets: Vec<String>) {
    let t = now();
    with(|l| match l.items.iter_mut().rev().find(|a| a.title == title) {
        Some(a) => a.assets.extend(assets.into_iter().map(SharedString::from)),
        None => l.items.push(Activity { start: t, end: t, lane: Actor::Person.lane().into(), kind: "Output".into(), title: title.to_string().into(), assets: assets.into_iter().map(SharedString::from).collect(), ..Default::default() }),
    });
}

/// Everything so far; spans still open end now.
pub fn all() -> Vec<Activity> {
    let t = now();
    let g = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let Some(l) = g.as_ref() else { return Vec::new() };
    let mut out = l.items.clone();
    for &i in l.open.values() {
        out[i].end = t.max(out[i].start + 1.0);
    }
    out
}

/// Forgets everything (a new project).
pub fn clear() {
    with(|l| {
        let ws = std::mem::take(&mut l.workspace);
        *l = Log::default();
        l.workspace = ws;
    });
}

/// A timeline titled `title` that follows the log (read again when it changed, every second).
pub fn view(title: &str, cx: &mut App) -> Entity<Timeline> {
    let timeline = cx.new(|_| Timeline::new(title.to_string()));
    timeline.update(cx, |t, cx| t.set_activities(all(), now(), cx));
    let weak = timeline.downgrade();
    cx.spawn(async move |cx| {
        let mut seen = generation();
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            let g = generation();
            // Open spans grow with time: read again now and then even when nothing was added
            let stale = g != seen;
            seen = g;
            let alive = weak.update(cx, |t, cx| {
                if stale || !t.activities().is_empty() {
                    t.set_activities(all(), now(), cx)
                }
            });
            if alive.is_err() {
                break;
            }
        }
    })
    .detach();
    timeline
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Commands land on their actor's lane with their provenance; spans close; jobs end.
    #[test]
    fn commands_spans_and_jobs_are_recorded() {
        clear();
        focus("Explore");
        command(Actor::Person, "hist.find_cells", "Find cells", "Analyze", "menu", &Value::Null, &Ok(serde_json::json!({"cells": 12})));
        command(Actor::Agent, "process.op.blur", "Gaussian blur", "Filter", "agent", &serde_json::json!({"sigma": 2}), &Err("Open an image first".into()));
        job_started(7, "histology", "Finding cells");
        produced("Find cells", vec!["cells.parquet".into()]);
        let a = all();
        let find = a.iter().find(|x| x.title == "Find cells").unwrap();
        assert_eq!((find.lane.as_ref(), find.kind.as_ref(), find.source.as_ref()), ("You", "Analyze", "Explore · menu"));
        assert!(find.detail.contains("cells"));
        assert_eq!(find.assets, vec![SharedString::from("cells.parquet")]);
        let blur = a.iter().find(|x| x.title == "Gaussian blur").unwrap();
        assert_eq!(blur.lane.as_ref(), "Agent");
        assert!(blur.detail.contains("refused"));
        assert!(a.iter().any(|x| x.lane == "Jobs" && x.end > x.start), "a running job lasts until now");
        job_ended(7);
        focus("Analyze");
        let a = all();
        assert_eq!(a.iter().filter(|x| x.lane == "Focus").count(), 2);
        assert!(a.iter().filter(|x| x.lane == "Focus").all(|x| x.end > x.start));
    }
}
