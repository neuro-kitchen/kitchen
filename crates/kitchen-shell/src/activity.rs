//! What happened in the app, as an episodic memory (kitchen-ui's [`Timeline`]): every command run
//! (from a menu, a key, the search, the title bar or an agent) with who ran it, the time spent in
//! each workspace, and every job from its start to its end. Attention, not content: a command's
//! arguments are kept in short, never the data it worked on.
//!
//! The registry and the frame record into it by themselves; an app adds what a step produced
//! ([`produced`]) so the timeline can say where a file came from. [`view`] is a timeline that
//! follows the log as it grows.
//!
//! Each activity is done for the person working ([`crate::people::who`]: their name, or `guest`).
//! A finished activity (an instant when recorded, a span when it ends) goes to the sinks
//! ([`on_finished`]): [`persist`] keeps them on disk, one JSON line each, a file per day (UTC),
//! and reads the last days back, so the timeline remembers past sessions; an app adds its own
//! (the history of the data it works on).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

/// Where finished activities go.
pub type Sink = Arc<dyn Fn(&Activity) + Send + Sync>;
static SINKS: Mutex<Vec<Sink>> = Mutex::new(Vec::new());

fn with<T>(f: impl FnOnce(&mut Log) -> T) -> T {
    let mut g = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let out = f(g.get_or_insert_with(Log::default));
    GENERATION.fetch_add(1, Ordering::Relaxed);
    out
}

/// Hands `a` to the sinks (outside the log's lock: a sink may record).
fn finished(a: Activity) {
    let sinks = SINKS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for sink in sinks {
        sink(&a);
    }
}

/// Adds a sink: it gets every activity once finished (an instant when recorded, a span when it
/// ends), on the thread that finished it.
pub fn on_finished(sink: impl Fn(&Activity) + Send + Sync + 'static) {
    SINKS.lock().unwrap_or_else(|e| e.into_inner()).push(Arc::new(sink));
}

/// `a`, done for the person working now (unless it says).
fn for_now(mut a: Activity) -> Activity {
    if a.by.is_empty() {
        a.by = crate::people::who().into();
    }
    a
}

/// Seconds since 1970 (UTC).
pub fn now() -> f64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// Bumped on every change (redraw when it moves).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Adds an activity (for the person working, unless it names one); one that lasts (`end` after
/// `start`) or an instant is finished as it is recorded.
pub fn record(a: Activity) {
    let a = for_now(a);
    with(|l| l.items.push(a.clone()));
    finished(a);
}

/// Opens a span `key` (a job, a workspace shown) that lasts until [`close`]; one open with the
/// same key is closed first.
pub fn open(key: impl Into<String>, mut a: Activity) {
    let key = key.into();
    let t = now();
    a = for_now(a);
    let ended = with(|l| {
        let ended = l.open.remove(&key).map(|i| {
            l.items[i].end = t;
            l.items[i].clone()
        });
        a.start = t;
        a.end = t;
        l.items.push(a);
        l.open.insert(key, l.items.len() - 1);
        ended
    });
    if let Some(e) = ended {
        finished(e);
    }
}

/// Closes the span `key` now.
pub fn close(key: &str) {
    let t = now();
    let ended = with(|l| {
        l.open.remove(key).map(|i| {
            l.items[i].end = t.max(l.items[i].start + 1.0);
            l.items[i].clone()
        })
    });
    if let Some(e) = ended {
        finished(e);
    }
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
    let by: SharedString = crate::people::who().into();
    let a = with(|l| {
        let source = if l.workspace.is_empty() { via.to_string() } else { format!("{} · {via}", l.workspace) };
        let a = Activity {
            start: t,
            end: t,
            lane: actor.lane().into(),
            kind: if group.is_empty() { "Commands".into() } else { SharedString::from(group.to_string()) },
            title: title.to_string().into(),
            detail: detail.join("  ").into(),
            source: source.into(),
            assets: Vec::new(),
            by,
        };
        l.items.push(a.clone());
        a
    });
    finished(a);
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
    // Kept with the activity it belongs to; the sinks get the files as an activity of their own
    let t = now();
    let assets: Vec<SharedString> = assets.into_iter().map(SharedString::from).collect();
    let out = for_now(Activity { start: t, end: t, lane: Actor::Person.lane().into(), kind: "Output".into(), title: title.to_string().into(), assets: assets.clone(), ..Default::default() });
    with(|l| match l.items.iter_mut().rev().find(|a| a.title == title) {
        Some(a) => a.assets.extend(assets),
        None => l.items.push(out.clone()),
    });
    finished(out);
}

/// The day of `unix` seconds (UTC) as `YYYY-MM-DD` (days to the civil date, Howard Hinnant's).
pub fn day(unix: f64) -> String {
    let z = (unix / 86_400.0).floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

/// Keeps every finished activity in `dir` (`YYYY-MM-DD.jsonl`, a line each, appended) and reads
/// back those of the last `days` days, before what this session records. Returns how many were
/// read. Not in a browser (no files): nothing is kept.
pub fn persist(dir: impl Into<PathBuf>, days: u32) -> usize {
    let dir: PathBuf = dir.into();
    let past = read_days(&dir, days);
    let n = past.len();
    with(|l| {
        let mut items = past;
        // The spans open now keep their place
        let shift = items.len();
        items.append(&mut l.items);
        l.items = items;
        for i in l.open.values_mut() {
            *i += shift;
        }
    });
    #[cfg(not(target_family = "wasm"))]
    on_finished(move |a| {
        use std::io::Write as _;
        let path = dir.join(format!("{}.jsonl", day(a.start)));
        let line = match serde_json::to_string(a) {
            Ok(l) => l,
            Err(_) => return,
        };
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{line}")) {
            eprintln!("activity log {}: {e}", path.display());
        }
    });
    n
}

/// The activities kept in `dir` for the last `days` days (UTC), oldest first; lines that do not
/// read are skipped.
pub fn read_days(dir: &std::path::Path, days: u32) -> Vec<Activity> {
    let today = now();
    let mut out = Vec::new();
    for back in (0..days).rev() {
        let path = dir.join(format!("{}.jsonl", day(today - back as f64 * 86_400.0)));
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        out.extend(text.lines().filter_map(|line| serde_json::from_str::<Activity>(line).ok()));
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
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

    /// The log is one for the process: its tests take turns.
    static TURN: Mutex<()> = Mutex::new(());

    /// Commands land on their actor's lane with their provenance; spans close; jobs end.
    #[test]
    fn commands_spans_and_jobs_are_recorded() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Finished activities reach the sinks, are kept a line each in a file per day and read back;
    /// each is done for the person working.
    #[test]
    fn finished_activities_are_kept_on_disk_and_read_back() {
        assert_eq!(day(0.0), "1970-01-01");
        assert_eq!(day(951_782_400.0 + 3600.0), "2000-02-29");
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("kitchen_activity_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        clear();
        persist(&dir, 2);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        on_finished(move |a| {
            if a.title == "Open" || a.title == "Sorting" {
                s.lock().unwrap().push(a.title.to_string())
            }
        });
        command(Actor::Person, "file.open", "Open", "File", "menu", &Value::Null, &Ok(Value::Null));
        job_started(1, "sort", "Sorting");
        job_ended(1);
        assert_eq!(*seen.lock().unwrap(), vec!["Open".to_string(), "Sorting".to_string()], "an instant when recorded, a span when it ends");
        let back = read_days(&dir, 1);
        let open = back.iter().find(|a| a.title == "Open").unwrap();
        assert_eq!(open.by.as_ref(), crate::people::who(), "done for the person working");
        assert!(back.iter().any(|a| a.title == "Sorting" && a.end > a.start));
        clear();
        assert_eq!(read_days(&dir, 1).len(), back.len(), "read back after a restart");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
