//! Ops: one operation each, described as data ([standard §2](../../../standard/spec/02-ops.md)).
//!
//! An op says what it reads and gives ([`PortSpec`]), what it is tuned with
//! ([`ParamSpec`](crate::ParamSpec)), how far it looks beyond a piece ([`Margin`]), where its code
//! is ([`OpSource`]: built in, a script, a tool, a recipe), its version, and whether it acts outside
//! the store (`effects`). Nothing here runs it: the runtime does.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::params::ParamSpec;
use crate::types::Type;

/// An input or an output of an op.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PortSpec {
    /// Its name, unique within the op's inputs, or its outputs (`image`, `mask`).
    pub name: String,
    /// What flows through it.
    #[serde(rename = "type")]
    pub ty: Type,
    /// An input that may stay unconnected.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// An input that takes a list (every recording of a session): batches and lists, no "for
    /// each" node (decision 0016).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub many: bool,
}

impl PortSpec {
    /// A required port `name` of type `ty`.
    pub fn new(name: impl Into<String>, ty: impl AsRef<str>) -> Self {
        Self { name: name.into(), ty: Type::new(ty), optional: false, many: false }
    }

    /// The same port, optional.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// The same port, taking a list.
    pub fn many(mut self) -> Self {
        self.many = true;
        self
    }
}

/// How far an op looks beyond its piece, per axis of the app's walk (lucem: z, y, x in pixels;
/// dsp-kitchen: samples before and after). Axes the app does not walk stay 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Margin {
    /// Before the piece, per axis.
    pub before: [usize; 3],
    /// After the piece, per axis.
    pub after: [usize; 3],
}

impl Margin {
    /// None: the op looks only at its piece.
    pub const NONE: Margin = Margin { before: [0; 3], after: [0; 3] };

    /// `n` on both sides of every axis the app walks (`axes` of them, from the last: one axis is
    /// time, two are y and x).
    pub fn around(n: usize, axes: usize) -> Self {
        let mut m = Margin::NONE;
        for a in (3 - axes.min(3))..3 {
            m.before[a] = n;
            m.after[a] = n;
        }
        m
    }

    /// `before` and `after` on the last axis (time).
    pub fn time(before: usize, after: usize) -> Self {
        Margin { before: [0, 0, before], after: [0, 0, after] }
    }

    /// The margin of two ops in a row: what each needs adds up.
    pub fn then(self, next: Margin) -> Margin {
        let mut m = self;
        for a in 0..3 {
            m.before[a] += next.before[a];
            m.after[a] += next.after[a];
        }
        m
    }

    /// The larger of two margins, per side and axis (two branches meeting).
    pub fn max(self, other: Margin) -> Margin {
        let mut m = self;
        for a in 0..3 {
            m.before[a] = m.before[a].max(other.before[a]);
            m.after[a] = m.after[a].max(other.after[a]);
        }
        m
    }

    /// Whether it needs nothing around the piece.
    pub fn is_none(&self) -> bool {
        *self == Margin::NONE
    }
}

/// The languages scripts are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// A `.py` file: a decorated function, run by the warm worker.
    Python,
    /// A `.rs` file: a `kitchen:` header and `pub fn run`, compiled once per version.
    Rust,
}

impl Language {
    /// The language of a file, by its extension.
    pub fn of(path: &std::path::Path) -> Option<Language> {
        match path.extension()?.to_str()? {
            "py" => Some(Language::Python),
            "rs" => Some(Language::Rust),
            _ => None,
        }
    }

    /// Its line-comment mark.
    pub fn comment(self) -> &'static str {
        match self {
            Language::Python => "#",
            Language::Rust => "//",
        }
    }
}

/// Where an op's code is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum OpSource {
    /// The app's own Rust code.
    Builtin,
    /// A script file; `hash` is its content's SHA-256 when it was read.
    Script {
        /// The file.
        path: PathBuf,
        /// Its language.
        language: Language,
        /// `sha256:<hex>` of its bytes.
        hash: String,
    },
    /// An operation of a tool program (run out of process, on whole results).
    Tool {
        /// The tool's name.
        tool: String,
        /// The operation's name in the tool.
        operation: String,
    },
    /// A recipe: a pipeline saved as an op; `hash` is the pipeline's content hash, which includes
    /// its scripts' hashes.
    Recipe {
        /// `sha256:<hex>` of the pipeline's canonical JSON.
        hash: String,
    },
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

fn is_one(v: &u32) -> bool {
    *v == 1
}

fn is_true(v: &bool) -> bool {
    *v
}

/// What an op is: everything a menu, an inspector, a pipeline check, an agent and the runtime need
/// to know, without running it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpSpec {
    /// Its id, unique in its namespace (`dspk:highpass`, `gaussian_blur`, `script:local_contrast`).
    pub id: String,
    /// Built-in and tool ops: a whole number from 1, bumped when the op's behaviour changes.
    /// Scripts and recipes are versioned by their hash ([`OpSpec::version_key`]).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub version: u32,
    /// Its name for people.
    pub title: String,
    /// Its group in the Operations menu (`Filter`, `Detect`).
    #[serde(default)]
    pub family: String,
    /// One line: what it does.
    #[serde(default)]
    pub summary: String,
    /// What it reads.
    pub inputs: Vec<PortSpec>,
    /// What it gives.
    pub outputs: Vec<PortSpec>,
    /// What it is tuned with.
    #[serde(default)]
    pub params: Vec<ParamSpec>,
    /// How far it looks beyond its piece.
    #[serde(default, alias = "context")]
    pub margin: Margin,
    /// Where its code is.
    pub source: OpSource,
    /// How its pieces' outputs are joined after the last piece (lucem's `stitch_labels`); `None`:
    /// in piece order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
    /// It acts outside the store (uploads, drives an instrument, sends a message, deletes): never
    /// reused from an earlier run, never previewed, asks before each run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub effects: bool,
    /// Whether it can run here; when not, [`OpSpec::unavailable`] says why.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub available: bool,
    /// Why it cannot run here ("weights not downloaded", "needs CUDA").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

impl OpSpec {
    /// A built-in operation `id` titled `title`, with nothing declared yet.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 1,
            title: title.into(),
            family: String::new(),
            summary: String::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Vec::new(),
            margin: Margin::NONE,
            source: OpSource::Builtin,
            finish: None,
            effects: false,
            available: true,
            unavailable: None,
        }
    }

    /// Sets its family.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.family = family.into();
        self
    }

    /// Sets its line.
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// Adds an input.
    pub fn input(mut self, port: PortSpec) -> Self {
        self.inputs.push(port);
        self
    }

    /// Adds an output.
    pub fn output(mut self, port: PortSpec) -> Self {
        self.outputs.push(port);
        self
    }

    /// Adds a parameter.
    pub fn param(mut self, param: ParamSpec) -> Self {
        self.params.push(param);
        self
    }

    /// Sets its margin.
    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }

    /// Sets its margin (the name kitchen-pipeline used before the standard).
    pub fn context(self, margin: Margin) -> Self {
        self.margin(margin)
    }

    /// Sets its version (built-in and tool ops).
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Sets how its pieces are joined.
    pub fn finish(mut self, finish: impl Into<String>) -> Self {
        self.finish = Some(finish.into());
        self
    }

    /// Marks it as acting outside the store.
    pub fn effects(mut self) -> Self {
        self.effects = true;
        self
    }

    /// Marks it as unable to run here, with why.
    pub fn unavailable(mut self, why: impl Into<String>) -> Self {
        self.available = false;
        self.unavailable = Some(why.into());
        self
    }

    /// What versions it for run keys: the hash of a script or a recipe, else `v<version>`.
    pub fn version_key(&self) -> String {
        match &self.source {
            OpSource::Script { hash, .. } | OpSource::Recipe { hash } => hash.clone(),
            OpSource::Builtin | OpSource::Tool { .. } => format!("v{}", self.version),
        }
    }

    /// The input `name`.
    pub fn input_port(&self, name: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|p| p.name == name)
    }

    /// The output `name`.
    pub fn output_port(&self, name: &str) -> Option<&PortSpec> {
        self.outputs.iter().find(|p| p.name == name)
    }

    /// Well formed: an id, at least one output, unique port and parameter names, defaults that
    /// fit their parameters.
    pub fn check(&self) -> Result<(), String> {
        let id = &self.id;
        if id.trim().is_empty() {
            return Err("an operation needs an id".into());
        }
        if self.outputs.is_empty() {
            return Err(format!("{id}: no outputs"));
        }
        if self.version == 0 {
            return Err(format!("{id}: versions start at 1"));
        }
        if !self.available && self.unavailable.is_none() {
            return Err(format!("{id}: unavailable without a reason"));
        }
        for (what, names) in [("input", self.inputs.iter().map(|p| &p.name).collect::<Vec<_>>()), ("output", self.outputs.iter().map(|p| &p.name).collect()), ("parameter", self.params.iter().map(|p| &p.name).collect())] {
            let mut seen = std::collections::BTreeSet::new();
            for n in names {
                if n.is_empty() || !n.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Err(format!("{id}: {what} name {n:?} (letters, digits and _ only)"));
                }
                if !seen.insert(n) {
                    return Err(format!("{id}: {what} {n} twice"));
                }
            }
        }
        for p in &self.params {
            p.check(&p.default).map_err(|e| format!("{id}: default of {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::params::ParamSpec;

    fn blur() -> OpSpec {
        OpSpec::new("gaussian_blur", "Gaussian blur")
            .family("Filter")
            .input(PortSpec::new("image", "image"))
            .output(PortSpec::new("image", "image"))
            .param(ParamSpec::number("sigma", 2.0).range(0.2, 20.0))
            .margin(Margin::around(6, 2))
    }

    #[test]
    fn a_well_formed_spec_checks() {
        blur().check().unwrap();
        // An input and an output may share a name; two inputs may not
        let twice = blur().input(PortSpec::new("image", "mask"));
        assert!(twice.check().unwrap_err().contains("input image twice"));
        let no_out = OpSpec::new("x", "X").input(PortSpec::new("a", "image"));
        assert!(no_out.check().unwrap_err().contains("no outputs"));
        let bad_default = blur().param(ParamSpec::number("gain", 50.0).range(0.0, 10.0));
        assert!(bad_default.check().unwrap_err().contains("default"));
        let bad_name = blur().param(ParamSpec::number("two words", 1.0));
        assert!(bad_name.check().is_err());
        assert!(blur().version(0).check().unwrap_err().contains("start at 1"));
    }

    #[test]
    fn versions_effects_and_availability() {
        let b = blur();
        assert_eq!(b.version, 1);
        assert_eq!(b.version_key(), "v1");
        assert_eq!(blur().version(3).version_key(), "v3");
        let mut script = blur();
        script.source = OpSource::Script { path: "a.py".into(), language: Language::Python, hash: "sha256:ab".into() };
        assert_eq!(script.version_key(), "sha256:ab", "a script is versioned by its hash");
        let upload = OpSpec::new("dandi_upload", "Upload to DANDI").input(PortSpec::new("x", "file")).output(PortSpec::new("receipt", "table")).effects();
        let json = serde_json::to_string(&upload).unwrap();
        assert!(json.contains(r#""effects":true"#) && !json.contains("version"), "{json}");
        let sam = blur().unavailable("weights not downloaded");
        assert!(!sam.available);
        sam.check().unwrap();
        let mut silent = blur();
        silent.available = false;
        assert!(silent.check().is_err());
        // The name before the standard still reads
        let old: OpSpec = serde_json::from_str(r#"{"id":"x","title":"X","inputs":[],"outputs":[{"name":"o","type":"image"}],"context":{"before":[0,0,3],"after":[0,0,3]},"source":{"from":"builtin"}}"#).unwrap();
        assert_eq!(old.margin, Margin::time(3, 3));
    }

    #[test]
    fn margins_add_along_a_pipeline_and_take_the_larger_at_a_join() {
        let blur = Margin::around(12, 2);
        assert_eq!(blur.before, [0, 12, 12]);
        let along = blur.then(Margin::around(3, 2));
        assert_eq!(along.after, [0, 15, 15]);
        let filter = Margin::time(300, 0);
        assert_eq!(filter.max(Margin::time(10, 40)), Margin::time(300, 40));
        assert!(Margin::NONE.is_none());
    }

    #[test]
    fn specs_read_and_write_json() {
        let spec = blur();
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains(r#""from":"builtin""#));
        let back: OpSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        assert_eq!(Language::of(std::path::Path::new("a/b.py")), Some(Language::Python));
    }
}
