//! What happened in the app, drawn: kitchen-services' activity log ([`kitchen_services::activity`])
//! as kitchen-ui's [`Timeline`]. The log itself (commands, focus, jobs, what was produced) is the
//! runtime's; this module re-exports it, so the registry and the frame record into it as before,
//! and adds the views: [`view`] follows the whole log, [`view_of`] what a filter keeps.

use gpui_kit::{App, AppContext as _, Entity};
pub use kitchen_services::activity::{Actor, Entry, clear, close, command, focus, generation, job_ended, job_started, now, open, produced};
use kitchen_ui::timeline::{Activity, Timeline};

/// An entry as the timeline draws it.
pub fn to_activity(e: &Entry) -> Activity {
    Activity {
        start: e.start,
        end: e.end,
        lane: e.lane.clone().into(),
        kind: e.kind.clone().into(),
        title: e.title.clone().into(),
        detail: e.detail.clone().into(),
        source: e.source.clone().into(),
        assets: e.assets.iter().cloned().map(Into::into).collect(),
        by: e.by.clone().into(),
    }
}

/// Adds an activity (for the person working, unless it names one).
pub fn record(a: Activity) {
    kitchen_services::activity::record(Entry {
        start: a.start,
        end: a.end,
        lane: a.lane.to_string(),
        kind: a.kind.to_string(),
        title: a.title.to_string(),
        detail: a.detail.to_string(),
        source: a.source.to_string(),
        assets: a.assets.iter().map(|s| s.to_string()).collect(),
        by: a.by.to_string(),
    });
}

/// Everything so far, as the timeline draws it; spans still open end now.
pub fn all() -> Vec<Activity> {
    kitchen_services::activity::all().iter().map(to_activity).collect()
}

/// What was made: a job, or an activity that produced something (a [`view_of`] filter).
pub fn made(a: &Activity) -> bool {
    a.lane == "Jobs" || !a.assets.is_empty()
}

/// A timeline titled `title` that follows the log (read again when it changed, every second).
pub fn view(title: &str, cx: &mut App) -> Entity<Timeline> {
    view_of(title, |_| true, cx)
}

/// A timeline of the activities `keep` keeps (e.g. [`made`]: what the session produced, not
/// every command), following the log.
pub fn view_of(title: &str, keep: impl Fn(&Activity) -> bool + 'static, cx: &mut App) -> Entity<Timeline> {
    let keep = std::rc::Rc::new(keep);
    let shown = { let keep = keep.clone(); move || all().into_iter().filter(|a| keep(a)).collect::<Vec<_>>() };
    let timeline = cx.new(|_| Timeline::new(title.to_string()));
    timeline.update(cx, |t, cx| t.set_activities(shown(), now(), cx));
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
                    t.set_activities(shown(), now(), cx)
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
    use gpui_kit::SharedString;
    use serde_json::Value;

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
        assert_eq!(find.by.as_ref(), crate::people::who(), "done for the person working");
        assert!(made(find), "it produced a file: made");
        assert!(!made(a.iter().find(|x| x.title == "Gaussian blur").unwrap()), "a command that made nothing is not");
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
