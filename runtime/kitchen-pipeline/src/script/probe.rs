//! The check before a script can be confirmed: run it once without data, in its own process.
//!
//! - **Python**: `kitchen_probe.py` installs the app's decorator names, imports the script and
//!   describes its operation from the function itself (`inspect`). Syntax and import errors come
//!   back with their line. The description goes through the same rules as the file's text
//!   ([`super::Interface::from_raw`]); any difference is reported, so what the person confirms
//!   is what will run.
//! - **Rust**: the file is compiled in the harness ([`super::worker::RustBuild`]); compiler
//!   errors come back as they are.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use super::interface::{Interface, Options, RawSignature};
use super::worker::{RustBuild, Tools, support_dir};
use super::Script;
use crate::op::Language;

/// What the check found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeResult {
    /// The file it checked (`sha256:<hex>`), so a result is not reused for another version.
    pub hash: String,
    /// It loads (or compiles).
    pub ok: bool,
    /// Why not.
    pub error: Option<String>,
    /// Where, in the script.
    pub line: Option<usize>,
    /// Where what the function declares differs from what its text says.
    pub mismatches: Vec<String>,
}

impl ProbeResult {
    /// Loads, and declares what its text says.
    pub fn passed(&self) -> bool {
        self.ok && self.mismatches.is_empty()
    }
}

#[derive(Deserialize)]
struct Answer {
    ok: bool,
    #[serde(default)]
    raw: Option<RawSignature>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    line: Option<usize>,
}

/// Checks `script` with `tools`.
pub fn probe(script: &Script, tools: &Tools, opts: &Options) -> ProbeResult {
    let mut out = ProbeResult { hash: script.hash.clone(), ..Default::default() };
    if !script.problems.is_empty() {
        out.error = Some(script.problems.join("; "));
        return out;
    }
    match script.language {
        Language::Python => {
            let support = match support_dir() {
                Ok(d) => d,
                Err(e) => {
                    out.error = Some(e);
                    return out;
                }
            };
            let answer = Command::new(&tools.python)
                .arg(support.join("kitchen_probe.py"))
                .arg(&script.path)
                .arg(opts.aliases.join(","))
                .env("PYTHONPATH", python_path(&support))
                .current_dir(script.path.parent().unwrap_or(Path::new(".")))
                .output();
            let answer = match answer {
                Ok(o) => o,
                Err(e) => {
                    out.error = Some(format!("{}: {e} (set the Python program in Settings › Tools)", tools.python.display()));
                    return out;
                }
            };
            let line = String::from_utf8_lossy(&answer.stdout).lines().last().unwrap_or("").to_string();
            match serde_json::from_str::<Answer>(&line) {
                Ok(a) if a.ok => {
                    out.ok = true;
                    if let (Some(raw), Some(text)) = (a.raw, &script.interface) {
                        out.mismatches = compare(text, &raw, opts);
                    }
                }
                Ok(a) => {
                    out.error = a.error;
                    out.line = a.line;
                }
                Err(_) => out.error = Some(format!("the check did not answer: {}", String::from_utf8_lossy(&answer.stderr).trim())),
            }
        }
        Language::Rust => match RustBuild::new(tools).build(&script.path) {
            Ok(_) => out.ok = true,
            Err(e) => out.error = Some(e),
        },
    }
    out
}

/// Where the function's own description differs from what the text said.
pub fn compare(text: &Interface, probed: &RawSignature, opts: &Options) -> Vec<String> {
    let (real, problems) = Interface::from_raw(probed, opts);
    let mut out: Vec<String> = problems.into_iter().map(|p| format!("as it runs: {p}")).collect();
    out.extend(real.changes_from(text).into_iter().map(|c| format!("as it runs, {c}")));
    out
}

/// `dir` before what `PYTHONPATH` already holds.
pub(crate) fn python_path(dir: &Path) -> std::ffi::OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).unwrap_or_else(|_| dir.as_os_str().to_os_string())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::interface::RawArg;

    #[test]
    fn a_function_that_differs_from_its_text_is_reported() {
        let opts = Options::new("lucem", &["image", "mask"], "image");
        let text = "@lucem.op\ndef f(image, sigma: float = 2.0):\n    return image\n";
        let s = Script::from_text(Path::new("f.py"), text, &opts);
        // As it runs (a decorator built `sigma` differently, say), sigma defaults to 3
        let probed = RawSignature {
            function: "f".into(),
            op: Some(Default::default()),
            args: vec![RawArg { name: "image".into(), annotation: None, default: None }, RawArg { name: "sigma".into(), annotation: Some("float".into()), default: Some("3.0".into()) }],
            ..Default::default()
        };
        let m = compare(s.interface.as_ref().unwrap(), &probed, &opts);
        assert_eq!(m, ["as it runs, parameter sigma changed"]);
        let same = RawSignature { args: vec![probed.args[0].clone(), RawArg { default: Some("2.0".into()), ..probed.args[1].clone() }], ..probed };
        assert!(compare(s.interface.as_ref().unwrap(), &same, &opts).is_empty());
    }

    /// The real probe, when Python is here (skipped otherwise).
    #[test]
    fn the_python_probe_reads_the_function() {
        let tools = Tools::detect();
        if Command::new(&tools.python).arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("kitchen-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let opts = Options::new("lucem", &["image", "mask"], "image");
        let good = dir.join("boost.py");
        std::fs::write(&good, "import lucem\n\n@lucem.op(family=\"Filter\", halo=12)\n@lucem.param(\"strength\", min=0.0, max=5.0)\ndef boost(image, sigma: float = 3.0, strength: float = 1.8, mask: lucem.Mask | None = None) -> lucem.Image:\n    \"\"\"Boost.\"\"\"\n    return image\n").unwrap();
        let r = probe(&Script::read(&good, &opts).unwrap(), &tools, &opts);
        assert!(r.passed(), "{r:?}");
        let bad = dir.join("bad.py");
        std::fs::write(&bad, "import lucem\n\n@lucem.op\ndef f(image):\n    return image\n\nraise RuntimeError('at import')\n").unwrap();
        let r = probe(&Script::read(&bad, &opts).unwrap(), &tools, &opts);
        assert!(!r.ok && r.error.as_deref().unwrap_or("").contains("at import"), "{r:?}");
        assert_eq!(r.line, Some(7));
    }
}
