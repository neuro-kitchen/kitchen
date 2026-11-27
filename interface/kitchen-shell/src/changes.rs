//! The Changes panel: the files that say what a project is and how its analysis was done
//! (`session.json`, `pipeline.json`, `project.json`), watched, and what changed in them shown as
//! git-style diffs (kitchen-ui's `DiffView`): added lines green, removed red, compared by key
//! (reordered keys are not changes) with long arrays as one summary line (`cells: 2,539 →
//! 2,712`).
//!
//! - **Compared against** ([`Baseline`]): the last save (the default; the app calls
//!   [`ChangesPanel::mark_saved`] when it saves), the start of the agent session ("everything
//!   this session did", [`ChangesPanel::start_session`]), or the last git commit when the
//!   project folder is a repository.
//! - **Review**: an agent's proposed change ([`ChangesPanel::propose`]) shows as a diff with
//!   Accept / Reject per hunk; [`ChangesPanel::apply_proposal`] writes the accepted hunks.
//! - **Badge**: [`ChangesPanel::changed_lines`] counts the changed lines (the app shows it on the
//!   panel's icon); [`ChangesEvent`] tells it when the count moves.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use kitchen_ui::diff::{ARRAY_SUMMARY, CONTEXT, Diff};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{DiffView, EmptyState, MenuSelect};

/// What the watched files are compared against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    /// The files as last saved.
    LastSave,
    /// The files when the agent session started.
    SessionStart,
    /// The files in the last git commit (when the project is a repository).
    GitHead,
}

impl Baseline {
    /// Every baseline, in the menu's order.
    pub const ALL: [Baseline; 3] = [Baseline::LastSave, Baseline::SessionStart, Baseline::GitHead];

    /// As the menu says it.
    pub fn title(self) -> &'static str {
        match self {
            Baseline::LastSave => "Since the last save",
            Baseline::SessionStart => "Since the session started",
            Baseline::GitHead => "Since the last commit",
        }
    }
}

/// What the panel tells its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangesEvent {
    /// The count of changed lines moved (the badge).
    Changed(usize),
    /// A proposal was applied to `file` (its accepted hunks written).
    Applied {
        /// The file written.
        file: PathBuf,
        /// Hunks accepted.
        accepted: usize,
        /// Hunks rejected.
        rejected: usize,
    },
}

/// One watched file.
struct Watched {
    path: PathBuf,
    name: SharedString,
    current: String,
    modified: Option<SystemTime>,
    saved: String,
    session: String,
    head: Option<String>,
    diff: Diff,
    view: Entity<DiffView>,
}

/// An agent's proposed change to a file, under review.
struct Proposal {
    file: usize,
    /// The file's text and the proposal, both normalised (pretty JSON, sorted keys) when JSON.
    old: String,
    new: String,
    view: Entity<DiffView>,
}

/// The Changes panel.
pub struct ChangesPanel {
    files: Vec<Watched>,
    baseline: Baseline,
    proposal: Option<Proposal>,
    message: Option<(bool, String)>,
    last_count: usize,
}

impl EventEmitter<ChangesEvent> for ChangesPanel {}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
}

fn read(path: &Path) -> (String, Option<SystemTime>) {
    (std::fs::read_to_string(path).unwrap_or_default(), std::fs::metadata(path).and_then(|m| m.modified()).ok())
}

/// The file in the last commit of the repository it is in (`None` when it is not in one, or not
/// committed).
pub fn git_head(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    let name = path.file_name()?.to_str()?;
    let out = std::process::Command::new("git").arg("-C").arg(dir).arg("show").arg(format!("HEAD:./{name}")).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `text` as pretty JSON with sorted keys when it is JSON, else as is (so a proposal and the file
/// compare line by line, and the accepted hunks make a valid file).
fn normalise(text: &str, json: bool) -> String {
    if json && let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        let mut s = serde_json::to_string_pretty(&v).unwrap_or_default();
        s.push('\n');
        return s;
    }
    text.to_string()
}

impl ChangesPanel {
    /// Watches `files` (each read now: that is the last save and the session's start until the
    /// app says otherwise), checking them for changes every second.
    pub fn new(files: Vec<PathBuf>, cx: &mut Context<Self>) -> Self {
        let files = files
            .into_iter()
            .enumerate()
            .map(|(i, path)| {
                let (current, modified) = read(&path);
                let name: SharedString = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
                let view = cx.new(|_| DiffView::new(SharedString::from(format!("changes-{i}")), Diff::default()).title(name.clone()));
                Watched { saved: current.clone(), session: current.clone(), head: None, diff: Diff::default(), current, modified, path, name, view }
            })
            .collect();
        let mut panel = Self { files, baseline: Baseline::LastSave, proposal: None, message: None, last_count: 0 };
        panel.recompute(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                if this.update(cx, |p, cx| p.refresh(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        panel
    }

    /// Reads again the files that changed on disk.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let mut any = false;
        for f in &mut self.files {
            let (text, modified) = read(&f.path);
            if modified != f.modified || text != f.current {
                f.current = text;
                f.modified = modified;
                any = true;
            }
        }
        if any {
            self.recompute(cx);
        }
    }

    fn base<'a>(&self, f: &'a Watched) -> &'a str {
        match self.baseline {
            Baseline::LastSave => &f.saved,
            Baseline::SessionStart => &f.session,
            Baseline::GitHead => f.head.as_deref().unwrap_or(""),
        }
    }

    fn recompute(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.files.len() {
            let f = &self.files[i];
            let base = self.base(f).to_string();
            let diff = if is_json(&f.path) { Diff::json(&base, &f.current, CONTEXT, ARRAY_SUMMARY) } else { Diff::text(&base, &f.current, CONTEXT) };
            if diff != self.files[i].diff {
                self.files[i].view.update(cx, |v, cx| v.set_diff(diff.clone(), cx));
                self.files[i].diff = diff;
            }
        }
        let count = self.changed_lines();
        if count != self.last_count {
            self.last_count = count;
            cx.emit(ChangesEvent::Changed(count));
        }
        cx.notify();
    }

    /// Changed lines over every watched file (added and removed): the badge.
    pub fn changed_lines(&self) -> usize {
        self.files.iter().map(|f| f.diff.added() + f.diff.removed()).sum()
    }

    /// The diff of the watched file `name` against the baseline.
    pub fn diff_of(&self, name: &str) -> Option<&Diff> {
        self.files.iter().find(|f| f.name == name).map(|f| &f.diff)
    }

    /// What the files are compared against.
    pub fn baseline(&self) -> Baseline {
        self.baseline
    }

    /// Compares against `baseline`; the last commit is refused, with the reason, when no watched
    /// file is in a git repository.
    pub fn set_baseline(&mut self, baseline: Baseline, cx: &mut Context<Self>) -> Result<(), String> {
        if baseline == Baseline::GitHead {
            for f in &mut self.files {
                f.head = git_head(&f.path);
            }
            if self.files.iter().all(|f| f.head.is_none()) {
                let e = "The project folder is not a git repository (or its files are not committed)".to_string();
                self.message = Some((true, e.clone()));
                cx.notify();
                return Err(e);
            }
        }
        self.baseline = baseline;
        self.message = None;
        self.recompute(cx);
        Ok(())
    }

    /// The files as they are now are the last save (the app calls this when it saves).
    pub fn mark_saved(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
        for f in &mut self.files {
            f.saved = f.current.clone();
        }
        self.recompute(cx);
    }

    /// The files as they are now are the session's start (an agent session begins).
    pub fn start_session(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
        for f in &mut self.files {
            f.session = f.current.clone();
        }
        self.recompute(cx);
    }

    /// Shows `new_text` proposed for the watched file `name` as a diff with Accept / Reject per
    /// hunk. JSON is compared as pretty JSON with sorted keys, so the accepted hunks make a valid
    /// file.
    pub fn propose(&mut self, name: &str, new_text: &str, cx: &mut Context<Self>) -> Result<usize, String> {
        self.refresh(cx);
        let file = self.files.iter().position(|f| f.name == name).ok_or_else(|| format!("{name} is not watched"))?;
        let f = &self.files[file];
        let json = is_json(&f.path);
        if json {
            serde_json::from_str::<serde_json::Value>(new_text).map_err(|e| format!("the proposed {name} is not JSON: {e}"))?;
        }
        let (old, new) = (normalise(&f.current, json), normalise(new_text, json));
        let diff = Diff::text(&old, &new, CONTEXT);
        let hunks = diff.hunks.len();
        let view = cx.new(|_| DiffView::new("proposal", diff).title(SharedString::from(format!("Proposed: {name}"))).reviewable(true));
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        self.proposal = Some(Proposal { file, old, new, view });
        self.message = None;
        cx.notify();
        Ok(hunks)
    }

    /// The proposal's diff view, while one is under review.
    pub fn proposal(&self) -> Option<Entity<DiffView>> {
        self.proposal.as_ref().map(|p| p.view.clone())
    }

    /// Writes the proposal's accepted hunks to its file (undecided hunks count as rejected) and
    /// closes it; returns how many were accepted.
    pub fn apply_proposal(&mut self, cx: &mut Context<Self>) -> Result<usize, String> {
        let p = self.proposal.take().ok_or("no proposal under review")?;
        let decisions: Vec<bool> = p.view.read(cx).decisions().iter().map(|d| d.unwrap_or(false)).collect();
        let accepted = decisions.iter().filter(|d| **d).count();
        let path = self.files[p.file].path.clone();
        let diff = Diff::text(&p.old, &p.new, CONTEXT);
        let text = diff.apply(&decisions).ok_or("the proposal cannot be applied")?;
        if accepted > 0 {
            std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        let rejected = decisions.len() - accepted;
        self.message = Some((false, format!("{}: {accepted} change(s) applied, {rejected} rejected", self.files[p.file].name)));
        cx.emit(ChangesEvent::Applied { file: path, accepted, rejected });
        self.refresh(cx);
        cx.notify();
        Ok(accepted)
    }

    /// Drops the proposal without writing anything.
    pub fn discard_proposal(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.proposal.take() {
            self.message = Some((false, format!("Proposal to {} discarded", self.files[p.file].name)));
        }
        cx.notify();
    }
}

impl Render for ChangesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.changed_lines();
        let me = cx.entity().downgrade();
        let baseline = MenuSelect::new(
            "changes-baseline",
            Baseline::ALL.iter().map(|b| SharedString::from(b.title())).collect(),
            Baseline::ALL.iter().position(|b| *b == self.baseline),
            move |i, _, cx| {
                let _ = me.update(cx, |p, cx| {
                    let _ = p.set_baseline(Baseline::ALL[i], cx);
                });
            },
        );
        let badge = div().px(px(theme::SM)).rounded(px(theme::RADIUS_SM)).bg(if count > 0 { color::accent() } else { color::control() }).text_color(if count > 0 { color::on_accent() } else { color::text_muted() }).text_xs().font_weight(FontWeight::SEMIBOLD).child(count.to_string());
        let header = h_flex()
            .flex_none()
            .gap(px(theme::SM))
            .items_center()
            .flex_wrap()
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Changes"))
            .child(badge)
            .child(div().flex_1())
            .child(div().w(px(220.)).child(baseline));
        let proposal = self.proposal.as_ref().map(|p| {
            let decided = p.view.read(cx).decisions().iter().filter(|d| d.is_some()).count();
            let total = p.view.read(cx).decisions().len();
            v_flex()
                .gap(px(theme::SM))
                .p(px(theme::MD))
                .rounded(px(theme::RADIUS_LG))
                .border_1()
                .border_color(color::accent_mark())
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("An agent proposes a change to {}", self.files[p.file].name)))
                .child(div().text_xs().text_color(color::text_muted()).child(format!("{decided} of {total} hunks decided; undecided hunks are not applied.")))
                .child(p.view.clone())
                .child(
                    h_flex()
                        .gap(px(theme::SM))
                        .child(Button::new("proposal-apply").small().primary().icon(IconName::Check).label("Apply accepted").disabled(decided == 0).on_click(cx.listener(|p, _, _, cx| {
                            if let Err(e) = p.apply_proposal(cx) {
                                p.message = Some((true, e));
                            }
                        })))
                        .child(Button::new("proposal-discard").small().outline().label("Discard").on_click(cx.listener(|p, _, _, cx| p.discard_proposal(cx)))),
                )
        });
        let changed: Vec<_> = self.files.iter().filter(|f| !f.diff.is_empty()).map(|f| f.view.clone()).collect();
        let unchanged: Vec<SharedString> = self.files.iter().filter(|f| f.diff.is_empty()).map(|f| f.name.clone()).collect();
        let body = if changed.is_empty() && self.proposal.is_none() {
            div().h(px(220.)).child(EmptyState::new(IconName::GitCompare, "No changes", format!("{} match {}.", unchanged.join(", "), self.baseline.title().to_lowercase()))).into_any_element()
        } else {
            v_flex()
                .gap(px(theme::MD))
                .children(changed)
                .when(!unchanged.is_empty(), |d| d.child(div().text_xs().text_color(color::text_muted()).child(format!("Unchanged: {}", unchanged.join(", ")))))
                .into_any_element()
        };
        v_flex()
            .id("changes")
            .size_full()
            .overflow_y_scroll()
            .p(px(theme::LG))
            .gap(px(theme::MD))
            .child(header)
            .children(self.message.clone().map(|(err, m)| div().text_sm().text_color(if err { color::danger() } else { color::text_muted() }).child(m)))
            .children(proposal)
            .child(body)
    }
}
