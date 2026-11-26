//! Operations written as scripts, in Python or Rust, the way lucem does it, for every app.
//!
//! 1. **Read** ([`interface`]): the file's text says what it is. A decorated Python function
//!    (`@lucem.op`, `@dspk.op`, `@kitchen.op`) gives its inputs, parameters and outputs from its
//!    signature; a Rust file says it in `kitchen:` header lines.
//! 2. **Check** ([`probe`]): the script is imported in a separate process, without data; it must
//!    load, and its function must declare what its text says.
//! 3. **Confirm** ([`review`]): a person confirms the script for what it is now. The confirmation
//!    is bound to the file's content (or, by choice, to its interface); an edit asks again,
//!    showing what changed.
//! 4. **Run** ([`worker`]): out of process, so a script never takes the app down: a warm Python
//!    worker, or the Rust file compiled once per version. Data travels as files in memory.
//!
//! [`ScriptOp`] is a confirmed script as an [`crate::Operation`] for the engine.

pub mod interface;
pub mod probe;
pub mod review;
pub mod worker;

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub use interface::{Interface, Options, RawSignature};
pub use probe::{ProbeResult, probe};
pub use review::{Approval, Approvals, ConfirmPolicy, Status};
pub use worker::{PythonWorker, RustBuild, ScriptOp, Tools};

use crate::op::{Language, OpSource, OpSpec};

/// A script file, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// The file.
    pub path: PathBuf,
    /// Its language.
    pub language: Language,
    /// `sha256:<hex>` of its bytes.
    pub hash: String,
    /// What its text says (`None`: it declares no operation).
    pub raw: Option<RawSignature>,
    /// Its interface, when it declares one.
    pub interface: Option<Interface>,
    /// What did not read; a script with problems is not usable.
    pub problems: Vec<String>,
}

impl Script {
    /// Reads the file at `path`.
    pub fn read(path: &Path, opts: &Options) -> std::io::Result<Script> {
        let text = std::fs::read_to_string(path)?;
        Ok(Script::from_text(path, &text, opts))
    }

    /// A script from its text (the editor's buffer, before it is saved).
    pub fn from_text(path: &Path, text: &str, opts: &Options) -> Script {
        let language = Language::of(path).unwrap_or(Language::Python);
        let raw = interface::read(text, language, opts);
        let (interface, problems) = match &raw {
            Some(r) => {
                let (i, p) = Interface::from_raw(r, opts);
                (Some(i), p)
            }
            None => (None, Vec::new()),
        };
        Script { path: path.to_path_buf(), language, hash: hash(text.as_bytes()), raw, interface, problems }
    }

    /// Whether it declares an operation.
    pub fn is_operation(&self) -> bool {
        self.interface.is_some()
    }

    /// Its operation id: `script:<file stem>`.
    pub fn id(&self) -> String {
        format!("script:{}", self.path.file_stem().and_then(|s| s.to_str()).unwrap_or("script"))
    }

    /// As an operation, when it declares one without problems.
    pub fn spec(&self) -> Option<OpSpec> {
        if !self.problems.is_empty() {
            return None;
        }
        let source = OpSource::Script { path: self.path.clone(), language: self.language, hash: self.hash.clone() };
        let mut spec = self.interface.as_ref()?.spec(self.id(), source);
        if spec.family.is_empty() {
            spec.family = "Scripts".into();
        }
        Some(spec)
    }
}

/// `sha256:<hex>` of `bytes`.
pub fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// The scripts in `dir` (not in subfolders) that declare an operation, by file name. The app's
/// plugin and project folders.
pub fn scan(dir: &Path, opts: &Options) -> Vec<Script> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Script> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| Language::of(p).is_some())
        .filter_map(|p| Script::read(&p, opts).ok())
        .filter(Script::is_operation)
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_read_hashed_and_described() {
        let opts = Options::new("lucem", &["image", "mask"], "image");
        let text = "import lucem\n\n@lucem.op(family=\"Filter\")\ndef sharpen(image, amount: float = 1.0):\n    return image\n";
        let s = Script::from_text(Path::new("/p/sharpen.py"), text, &opts);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert_eq!(s.id(), "script:sharpen");
        assert!(s.hash.starts_with("sha256:") && s.hash.len() == 71);
        let spec = s.spec().unwrap();
        assert_eq!(spec.family, "Filter");
        assert!(matches!(spec.source, OpSource::Script { language: Language::Python, .. }));
        let edited = Script::from_text(Path::new("/p/sharpen.py"), &text.replace("1.0", "2.0"), &opts);
        assert_ne!(edited.hash, s.hash);
        let plain = Script::from_text(Path::new("/p/notes.py"), "print(1)\n", &opts);
        assert!(!plain.is_operation() && plain.spec().is_none());
    }
}
