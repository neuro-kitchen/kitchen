//! What happened in the app, as an episodic memory: every command run (from a menu, a key, the
//! search, the title bar or an agent) with who ran it, the time spent in each workspace, and every
//! job from its start to its end. Attention, not content: a command's arguments are kept in short,
//! never the data it worked on.
//!
//! The command registry and the frame record into it by themselves; an app adds what a step
//! produced ([`produced`]) so the timeline can say where a file came from. kitchen-shell draws it
//! (its `activity::view`, on kitchen-ui's timeline).
//!
//! The log lives in memory for the app's session and is never written anywhere: it is the
//! session's story, not a record of the person. Each entry names who it was done for
//! ([`crate::people::who`]: a person's name, or `guest`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

/// One thing that happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// When it started (seconds since 1970, UTC).
    pub start: f64,
    /// When it ended (the same as `start` for an instant).
    pub end: f64,
    /// Who or what did it: its row ("You", "Agent", "Jobs", "Focus").
    pub lane: String,
    /// Its kind: the colour and the episode it joins ("Analyze", "Pipeline", "histology").
    pub kind: String,
    /// What it was ("Find cells").
    pub title: String,
    /// A short line about it (its arguments, its result).
    pub detail: String,
    /// Where it came from ("Explore · menu", "agent · lumen MCP").
    pub source: String,
    /// What it produced or used (files, ids).
    pub assets: Vec<String>,
    /// The person it was done for, by name (`guest` for no one named).
    pub by: String,
}

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
    items: Vec<Entry>,
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
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// Bumped on every change (redraw when it moves).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// `a`, done for the person working now (unless it says).
fn for_now(mut a: Entry) -> Entry {
    if a.by.is_empty() {
        a.by = crate::people::who().into();
    }
    a
}

/// Adds an activity (for the person working, unless it names one).
pub fn record(a: Entry) {
    let a = for_now(a);
    with(|l| l.items.push(a));
}

/// Opens a span `key` (a job, a workspace shown) that lasts until [`close`]; one open with the
/// same key is closed first.
pub fn open(key: impl Into<String>, mut a: Entry) {
    let key = key.into();
    let t = now();
    a = for_now(a);
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
    let by: String = crate::people::who();
    with(|l| {
        let source = if l.workspace.is_empty() { via.to_string() } else { format!("{} · {via}", l.workspace) };
        l.items.push(Entry {
            start: t,
            end: t,
            lane: actor.lane().into(),
            kind: if group.is_empty() { "Commands".into() } else { String::from(group.to_string()) },
            title: title.to_string().into(),
            detail: detail.join("  ").into(),
            source: source.into(),
            assets: Vec::new(),
            by,
        })
    });
}

/// The workspace shown now: a span on the Focus lane, until the next one.
pub fn focus(workspace: &str) {
    let name = workspace.to_string();
    with(|l| l.workspace = name.clone());
    open("focus", Entry { lane: "Focus".into(), kind: String::from(name.clone()), title: format!("In {name}").into(), ..Default::default() });
}

/// A job started (its span on the Jobs lane, until [`job_ended`]).
pub fn job_started(id: u64, kind: &str, name: &str) {
    open(format!("job-{id}"), Entry { lane: "Jobs".into(), kind: kind.to_string().into(), title: name.to_string().into(), source: "job".into(), ..Default::default() });
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
        Some(a) => a.assets.extend(assets.into_iter()),
        None => l.items.push(for_now(Entry { start: t, end: t, lane: Actor::Person.lane().into(), kind: "Output".into(), title: title.to_string().into(), assets: assets.into_iter().collect(), ..Default::default() })),
    });
}

/// Everything so far; spans still open end now.
pub fn all() -> Vec<Entry> {
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

/// What was made: a job, or an entry that produced something.
pub fn made(a: &Entry) -> bool {
    a.lane == "Jobs" || !a.assets.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_jobs_and_outputs_are_recorded() {
        clear();
        command(Actor::Agent, "process.blur", "Blur", "Process", "agent", &serde_json::json!({"sigma": 2}), &Ok(Value::Null));
        job_started(7, "export", "Exporting tables");
        produced("Exporting tables", vec!["cells.parquet".into()]);
        job_ended(7);
        let all = all();
        let blur = all.iter().find(|e| e.title == "Blur").unwrap();
        assert_eq!((blur.lane.as_str(), blur.kind.as_str()), ("Agent", "Process"));
        assert!(blur.detail.contains("sigma"));
        let job = all.iter().find(|e| e.lane == "Jobs").unwrap();
        assert_eq!(job.assets, ["cells.parquet"]);
        assert!(job.end >= job.start);
        assert!(made(job) && !made(blur));
    }
}
