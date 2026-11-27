//! A script runs in a pipeline only once a person has confirmed it for what it is now.
//!
//! ```text
//!   read ──problems──► Unreadable            (the text does not declare a usable operation)
//!    │
//!    ▼
//!   Unchecked ──probe──► Failed / Mismatch    (does not load; runs differently from its text)
//!                 │
//!                 ▼
//!               Checked ──confirm──► Confirmed ──edit──► Checked (with what changed)
//! ```
//!
//! A confirmation records the file's hash and its interface, who confirmed it and when, in the
//! project's `approvals.json` (a file people can read and commit). By default it holds for that
//! content only: any edit asks again, showing whether the inputs, outputs or parameters changed
//! ([`ConfirmPolicy::Content`]). An app may choose to keep it while the interface stays the same
//! ([`ConfirmPolicy::Interface`]); the probe still runs on every change.
//!
//! A preview (the screen's part, not written) runs a Checked script; a run needs Confirmed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::interface::Interface;
use super::probe::ProbeResult;
use super::Script;

/// What a confirmation is bound to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmPolicy {
    /// The file's content: any edit asks again.
    #[default]
    Content,
    /// The interface: an edit inside the function keeps the confirmation (once checked again).
    Interface,
}

/// One confirmation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    /// The file's hash when confirmed.
    pub hash: String,
    /// What it declared.
    pub interface: Interface,
    /// Who confirmed it.
    pub by: String,
    /// When (seconds since 1970).
    pub at: u64,
}

/// Where a script stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Its text does not declare a usable operation: these problems.
    Unreadable(Vec<String>),
    /// Not checked for this version yet.
    Unchecked,
    /// It does not load (or compile): why, and where.
    Failed {
        /// The error.
        error: String,
        /// Its line in the script.
        line: Option<usize>,
    },
    /// It loads, but runs differently from what its text says.
    Mismatch(Vec<String>),
    /// Checked; a person has to confirm it. `changes`: what differs from the version last
    /// confirmed (`None`: never confirmed).
    Checked {
        /// What changed since the last confirmation.
        changes: Option<Vec<String>>,
    },
    /// Confirmed for what it is now.
    Confirmed,
}

impl Status {
    /// A pipeline may run it.
    pub fn can_run(&self) -> bool {
        matches!(self, Status::Confirmed)
    }

    /// A preview may run it.
    pub fn can_preview(&self) -> bool {
        matches!(self, Status::Confirmed | Status::Checked { .. })
    }

    /// One line for the Pipeline screen.
    pub fn line(&self) -> String {
        match self {
            Status::Unreadable(p) => format!("Does not declare an operation: {}", p.join("; ")),
            Status::Unchecked => "Not checked yet".into(),
            Status::Failed { error, line: Some(l) } => format!("Does not load (line {l}): {error}"),
            Status::Failed { error, line: None } => format!("Does not load: {error}"),
            Status::Mismatch(m) => format!("Runs differently from what it says: {}", m.join("; ")),
            Status::Checked { changes: None } => "Checked. Confirm it to use it in pipelines".into(),
            Status::Checked { changes: Some(c) } if c.is_empty() => "Edited since you confirmed it; its inputs and settings are the same. Confirm again".into(),
            Status::Checked { changes: Some(c) } => format!("Changed since you confirmed it: {}. Confirm again", c.join(", ")),
            Status::Confirmed => "Confirmed".into(),
        }
    }
}

/// The confirmations of a project, by script path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Approvals {
    path: Option<PathBuf>,
    /// What decides whether a confirmation still holds.
    pub policy: ConfirmPolicy,
    by_script: BTreeMap<String, Approval>,
}

#[derive(Serialize, Deserialize)]
struct File {
    #[serde(default)]
    policy: ConfirmPolicy,
    #[serde(default)]
    scripts: BTreeMap<String, Approval>,
}

impl Approvals {
    /// No confirmations, kept in memory only.
    pub fn new() -> Self {
        Self::default()
    }

    /// The confirmations saved at `path` (none if it does not exist yet).
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let file: File = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => File { policy: ConfirmPolicy::default(), scripts: BTreeMap::new() },
            Err(e) => return Err(e),
        };
        Ok(Self { path: Some(path.to_path_buf()), policy: file.policy, by_script: file.scripts })
    }

    /// Writes them back where they were loaded from (nothing for an in-memory set).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&File { policy: self.policy, scripts: self.by_script.clone() }).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }

    fn key(script: &Script) -> String {
        script.path.to_string_lossy().into_owned()
    }

    /// The confirmation of `script`'s file, whatever its version.
    pub fn get(&self, script: &Script) -> Option<&Approval> {
        self.by_script.get(&Self::key(script))
    }

    /// Where `script` stands, given the probe of its current version (if run).
    pub fn status(&self, script: &Script, probe: Option<&ProbeResult>) -> Status {
        let Some(interface) = &script.interface else { return Status::Unreadable(vec!["no operation: decorate a function with @op, or add `kitchen:` header lines".into()]) };
        if !script.problems.is_empty() {
            return Status::Unreadable(script.problems.clone());
        }
        let probe = match probe.filter(|p| p.hash == script.hash) {
            None => return Status::Unchecked,
            Some(p) => p,
        };
        if !probe.ok {
            return Status::Failed { error: probe.error.clone().unwrap_or_default(), line: probe.line };
        }
        if !probe.mismatches.is_empty() {
            return Status::Mismatch(probe.mismatches.clone());
        }
        match self.get(script) {
            Some(a) if a.hash == script.hash => Status::Confirmed,
            Some(a) if self.policy == ConfirmPolicy::Interface && &a.interface == interface => Status::Confirmed,
            Some(a) => Status::Checked { changes: Some(interface.changes_from(&a.interface)) },
            None => Status::Checked { changes: None },
        }
    }

    /// Confirms `script` as it is now, by `by`. Refused unless it is checked (a passing probe of
    /// this version).
    pub fn confirm(&mut self, script: &Script, probe: Option<&ProbeResult>, by: impl Into<String>) -> Result<(), String> {
        match self.status(script, probe) {
            Status::Checked { .. } | Status::Confirmed => {}
            other => return Err(other.line()),
        }
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let interface = script.interface.clone().ok_or("no operation")?;
        self.by_script.insert(Self::key(script), Approval { hash: script.hash.clone(), interface, by: by.into(), at });
        Ok(())
    }

    /// Takes the confirmation of `script` back; whether there was one.
    pub fn revoke(&mut self, script: &Script) -> bool {
        self.by_script.remove(&Self::key(script)).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::interface::Options;

    fn opts() -> Options {
        Options::new("lucem", &["image", "mask"], "image")
    }

    fn script(text: &str) -> Script {
        Script::from_text(Path::new("/project/scripts/sharpen.py"), text, &opts())
    }

    fn passed(s: &Script) -> ProbeResult {
        ProbeResult { hash: s.hash.clone(), ok: true, ..Default::default() }
    }

    const V1: &str = "@lucem.op\ndef sharpen(image, amount: float = 1.0):\n    return image\n";

    #[test]
    fn checked_then_confirmed_and_an_edit_asks_again() {
        let mut a = Approvals::new();
        let s = script(V1);
        assert_eq!(a.status(&s, None), Status::Unchecked);
        assert!(a.confirm(&s, None, "ana").is_err(), "not before it is checked");
        let p = passed(&s);
        assert_eq!(a.status(&s, Some(&p)), Status::Checked { changes: None });
        assert!(a.status(&s, Some(&p)).can_preview() && !a.status(&s, Some(&p)).can_run());
        a.confirm(&s, Some(&p), "ana").unwrap();
        assert!(a.status(&s, Some(&p)).can_run());
        assert_eq!(a.get(&s).unwrap().by, "ana");

        // A body edit: checked again, then the same interface, but the content changed
        let body = script(&V1.replace("return image", "return image * 1"));
        assert_eq!(a.status(&body, Some(&p)), Status::Unchecked, "the old probe is for the old version");
        assert_eq!(a.status(&body, Some(&passed(&body))), Status::Checked { changes: Some(vec![]) });
        // An interface edit says what changed
        let wider = script(&V1.replace("amount: float = 1.0", "amount: float = 1.0, radius: int = 2"));
        assert_eq!(a.status(&wider, Some(&passed(&wider))), Status::Checked { changes: Some(vec!["new parameter radius".into()]) });
    }

    #[test]
    fn the_interface_policy_keeps_a_confirmation_across_body_edits() {
        let mut a = Approvals { policy: ConfirmPolicy::Interface, ..Approvals::new() };
        let s = script(V1);
        a.confirm(&s, Some(&passed(&s)), "ana").unwrap();
        let body = script(&V1.replace("return image", "return image * 1"));
        assert_eq!(a.status(&body, Some(&passed(&body))), Status::Confirmed);
        let wider = script(&V1.replace("1.0", "2.0"));
        assert!(matches!(a.status(&wider, Some(&passed(&wider))), Status::Checked { .. }), "a new default is an interface change");
    }

    #[test]
    fn failures_and_mismatches_block_confirmation_and_saves_round_trip() {
        let mut a = Approvals::new();
        let s = script(V1);
        let failed = ProbeResult { hash: s.hash.clone(), ok: false, error: Some("ImportError: scipy".into()), line: Some(1), ..Default::default() };
        assert_eq!(a.status(&s, Some(&failed)).line(), "Does not load (line 1): ImportError: scipy");
        assert!(a.confirm(&s, Some(&failed), "ana").is_err());
        let differs = ProbeResult { mismatches: vec!["as it runs, parameter amount changed".into()], ..passed(&s) };
        assert!(matches!(a.status(&s, Some(&differs)), Status::Mismatch(_)));
        assert!(matches!(a.status(&script("print(1)\n"), None), Status::Unreadable(_)));

        let path = std::env::temp_dir().join(format!("kitchen-approvals-{}/approvals.json", std::process::id()));
        let mut saved = Approvals::load(&path).unwrap();
        saved.confirm(&s, Some(&passed(&s)), "ana").unwrap();
        saved.save().unwrap();
        let back = Approvals::load(&path).unwrap();
        assert_eq!(back.status(&s, Some(&passed(&s))), Status::Confirmed);
        assert!(saved.revoke(&s) && !saved.revoke(&s));
    }
}
