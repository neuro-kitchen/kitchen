//! What the Changes panel compares: the files that say what a project is and how its analysis
//! was done (`session.json`, `pipeline.json`, `project.json`), read and normalised, and what they
//! are compared against ([`Baseline`]). kitchen-shell's `ChangesPanel` draws the diffs.

use std::path::Path;
use std::time::SystemTime;

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

/// Whether `path` is JSON (compared by key, not by line order).
pub fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
}

/// The file's text (empty when it cannot be read) and when it was last modified.
pub fn read(path: &Path) -> (String, Option<SystemTime>) {
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

/// `value` with every object's keys sorted, whatever order serde_json keeps keys in (its
/// `preserve_order` feature, turned on by another crate in a larger build, keeps them as read).
fn sorted(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().map(|(k, v)| (k, sorted(v))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// `text` as pretty JSON with sorted keys when it is JSON, else as is (so a proposal and the file
/// compare line by line, and the accepted hunks make a valid file).
pub fn normalise(text: &str, json: bool) -> String {
    if json && let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        let mut s = serde_json::to_string_pretty(&sorted(v)).unwrap_or_default();
        s.push('\n');
        return s;
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_compares_by_key() {
        let a = normalise(r#"{"b":1,"a":2}"#, true);
        let b = normalise("{\n  \"a\": 2,\n  \"b\": 1\n}", true);
        assert_eq!(a, b, "reordered keys are not changes");
        assert_eq!(normalise("not json", true), "not json");
        assert!(is_json(Path::new("session.json")) && !is_json(Path::new("notes.md")));
        assert_eq!(Baseline::ALL.len(), 3);
    }
}
