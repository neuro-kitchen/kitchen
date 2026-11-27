//! Headless UI tests of the Changes panel: watched files as diffs against a baseline, the badge,
//! an agent's proposal reviewed hunk by hunk and applied.

mod common;

use common::{frame, open};
use gpui_kit::AppContext as _;
use gpui_kit::TestAppContext;
use kitchen_shell::{Baseline, ChangesEvent};
use serde_json::Value;

#[gpui_kit::test]
fn changes_show_what_moved_and_proposals_apply_per_hunk(cx: &mut TestAppContext) {
    let (window, g) = open(cx);
    cx.update(|cx| g.shell.update(cx, |s, cx| assert!(s.set_workspace("changes", cx))));
    frame(cx, window);
    let panel = g.changes.clone();
    assert_eq!(cx.update(|cx| panel.read(cx).changed_lines()), 0, "nothing changed since opening");

    // A step runs: session.json gains lines; project.json's 2,539 cells become 2,712, one line
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let ev = events.clone();
    cx.update(|cx| cx.subscribe(&panel, move |_, e: &ChangesEvent, _| ev.borrow_mut().push(e.clone())).detach());
    cx.update_window(window, |_, window, cx| g.registry.run("project.run_step", Value::Null, window, cx)).unwrap().unwrap();
    frame(cx, window);
    let (session, project) = cx.update(|cx| (panel.read(cx).diff_of("session.json").cloned().unwrap(), panel.read(cx).diff_of("project.json").cloned().unwrap()));
    assert!(session.added() > 0 && session.json);
    let summary: Vec<String> = project.lines.iter().filter(|l| l.kind == kitchen_ui::diff::LineKind::Summary).map(|l| l.text.clone()).collect();
    assert_eq!(summary.len(), 1, "the long array is one summary line: {summary:?}");
    assert!(summary[0].contains("2,539") && summary[0].contains("2,712"), "{}", summary[0]);
    let count = cx.update(|cx| panel.read(cx).changed_lines());
    assert!(count > 0);
    assert!(events.borrow().contains(&ChangesEvent::Changed(count)), "the badge was told");

    // Saved: nothing differs from the last save; the session baseline still shows the step
    cx.update(|cx| panel.update(cx, |p, cx| p.mark_saved(cx)));
    assert_eq!(cx.update(|cx| panel.read(cx).changed_lines()), 0);
    cx.update(|cx| panel.update(cx, |p, cx| p.set_baseline(Baseline::SessionStart, cx))).unwrap();
    assert!(cx.update(|cx| panel.read(cx).changed_lines()) > 0);
    // The temp folder is not a repository: the last commit is refused with the reason
    let head = cx.update(|cx| panel.update(cx, |p, cx| p.set_baseline(Baseline::GitHead, cx)));
    assert!(head.unwrap_err().contains("git"));
    cx.update(|cx| panel.update(cx, |p, cx| p.set_baseline(Baseline::LastSave, cx))).unwrap();

    // An agent proposes a change to pipeline.json: hunks reviewed one by one
    cx.update_window(window, |_, window, cx| g.registry.run("project.propose", Value::Null, window, cx)).unwrap().unwrap();
    frame(cx, window);
    let view = cx.update(|cx| panel.read(cx).proposal()).expect("a proposal is under review");
    let hunks = cx.update(|cx| view.read(cx).decisions().len());
    assert!(hunks >= 2, "sigma and the new node are separate hunks: {hunks}");
    // Accept the first hunk only
    cx.update(|cx| view.update(cx, |v, cx| v.decide(0, true, cx)));
    for h in 1..hunks {
        cx.update(|cx| view.update(cx, |v, cx| v.decide(h, false, cx)));
    }
    let accepted = cx.update(|cx| panel.update(cx, |p, cx| p.apply_proposal(cx))).unwrap();
    assert_eq!(accepted, 1);
    let written: Value = serde_json::from_str(&std::fs::read_to_string(g.project.join("pipeline.json")).unwrap()).expect("the file is still valid JSON");
    assert_eq!(written["nodes"][0]["sigma"], 1.5, "the accepted hunk is in");
    assert_eq!(written["nodes"].as_array().unwrap().len(), 4, "the rejected node is not");
    assert!(cx.update(|cx| panel.read(cx).proposal().is_none()));
    frame(cx, window);
    let _ = std::fs::remove_dir_all(&g.project);
}
