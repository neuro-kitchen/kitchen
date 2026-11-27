//! Change tracking: what changed in the open script, marked in the editor's gutter git style
//! (added, modified, removed lines). When a space is dense (a long script grown over a session,
//! an agent's edits), the marks say at a glance what is new. A toggle turns them on and off
//! (a toolbar button and the `[code] toggle_changes` key); they compare against one baseline:
//!
//! - **the last save** (the default): what is not written yet;
//! - **when it was opened**: everything this session did to it;
//! - **the last commit**, when the script sits in a git repository: what is not committed.
//!
//! Plain data: the workspace owns one [`ChangeTracking`] per open script and hands its marks to
//! the code view.

use std::path::Path;

use kitchen_ui::diff::{ChangeMark, LineChange, line_changes};

/// What the marks compare against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeBase {
    /// The file as last saved.
    #[default]
    Saved,
    /// The file when it was opened.
    Opened,
    /// The file in the last git commit.
    Commit,
}

impl ChangeBase {
    /// Every base, in the order the toolbar offers them.
    pub const ALL: [ChangeBase; 3] = [ChangeBase::Saved, ChangeBase::Opened, ChangeBase::Commit];

    /// As the toolbar says it.
    pub fn title(self) -> &'static str {
        match self {
            ChangeBase::Saved => "Since save",
            ChangeBase::Opened => "Since opened",
            ChangeBase::Commit => "Since commit",
        }
    }
}

/// The baselines of one open script, and whether its marks show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeTracking {
    base: ChangeBase,
    opened: String,
    saved: String,
    /// The last commit's text; none outside a repository or before the first commit.
    commit: Option<String>,
}

impl ChangeTracking {
    /// The baselines of the script at `path` whose text is `text` now (opened and saved alike);
    /// the commit's text is read from git when there is one.
    pub fn open(path: &Path, text: &str, base: ChangeBase) -> Self {
        let commit = kitchen_shell::changes::git_head(path);
        let base = if base == ChangeBase::Commit && commit.is_none() { ChangeBase::Saved } else { base };
        Self { base, opened: text.to_string(), saved: text.to_string(), commit }
    }

    /// The text was written: it is the new last save; the commit is read again (a save in an
    /// external editor, or a commit made meanwhile, moves it).
    pub fn saved(&mut self, path: &Path, text: &str) {
        self.saved = text.to_string();
        self.commit = kitchen_shell::changes::git_head(path);
    }

    /// The text was read again from the file (changed outside): every baseline but "opened" moves.
    pub fn reloaded(&mut self, path: &Path, text: &str) {
        self.saved(path, text);
    }

    /// What the marks compare against.
    pub fn base(&self) -> ChangeBase {
        self.base
    }

    /// Compares against `base`; false (nothing changes) when it is the commit and there is none.
    pub fn set_base(&mut self, base: ChangeBase) -> bool {
        if base == ChangeBase::Commit && self.commit.is_none() {
            return false;
        }
        self.base = base;
        true
    }

    /// Whether the script is in a repository with a commit to compare against.
    pub fn has_commit(&self) -> bool {
        self.commit.is_some()
    }

    /// The baseline's text.
    pub fn baseline(&self) -> &str {
        match self.base {
            ChangeBase::Saved => &self.saved,
            ChangeBase::Opened => &self.opened,
            ChangeBase::Commit => self.commit.as_deref().unwrap_or(&self.saved),
        }
    }

    /// The marks of `text` against the baseline.
    pub fn marks(&self, text: &str) -> Vec<ChangeMark> {
        line_changes(self.baseline(), text)
    }
}

/// Lines added, modified and removed in `marks`, for the toolbar (`+3 ~1 −2`).
pub fn summary(marks: &[ChangeMark]) -> (usize, usize, usize) {
    marks.iter().fold((0, 0, 0), |(a, m, r), c| match c.change {
        LineChange::Added => (a + c.lines, m, r),
        LineChange::Modified => (a, m + c.lines, r),
        LineChange::Removed => (a, m, r + c.lines),
    })
}

/// `+3 ~1 −2`, leaving out what is zero; `no changes` when nothing is.
pub fn summary_text(marks: &[ChangeMark]) -> String {
    let (a, m, r) = summary(marks);
    let parts: Vec<String> = [(a, '+'), (m, '~'), (r, '−')].iter().filter(|(n, _)| *n > 0).map(|(n, s)| format!("{s}{n}")).collect();
    if parts.is_empty() { "no changes".into() } else { parts.join(" ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracking(text: &str) -> ChangeTracking {
        ChangeTracking { base: ChangeBase::Saved, opened: text.into(), saved: text.into(), commit: None }
    }

    #[test]
    fn marks_follow_the_chosen_baseline() {
        let mut t = tracking("a\nb\n");
        assert!(t.marks("a\nb\n").is_empty());
        assert_eq!(summary_text(&t.marks("a\nB\nc\n")), "+1 ~1");
        // Saved: the save is the new baseline, the opened text stays
        t.saved = "a\nB\nc\n".into();
        assert!(t.marks("a\nB\nc\n").is_empty());
        assert!(t.set_base(ChangeBase::Opened));
        assert_eq!(summary_text(&t.marks("a\nB\nc\n")), "+1 ~1");
        // No repository: the commit cannot be chosen
        assert!(!t.set_base(ChangeBase::Commit));
        assert_eq!(t.base(), ChangeBase::Opened);
        assert_eq!(summary_text(&t.marks("a\n")), "−1");
    }
}
