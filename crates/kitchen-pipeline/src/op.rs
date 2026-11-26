//! An operation: what it is ([`OpSpec`]: ports, parameters, context), where it comes from
//! ([`OpSource`]), and how an app's own Rust operation runs ([`Operation`]).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::{Domain, Piece};
use crate::types::{Context, ParamSpec, ParamValue, PortSpec};

/// The languages scripts are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Where an operation comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// An operation of a kitchen-tool program (run out of process, a whole artifact at a time).
    Tool {
        /// The tool's name.
        tool: String,
        /// The operation's name in the tool.
        operation: String,
    },
}

/// What an operation is: everything the catalog, the Inspector, the graph check and the engine
/// need to know, without running it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpSpec {
    /// Its id, unique in the app (`gaussian_blur`, `script:local_contrast_boost`).
    pub id: String,
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
    #[serde(default)]
    pub context: Context,
    /// Where it comes from.
    pub source: OpSource,
}

impl OpSpec {
    /// A built-in operation `id` titled `title`, with nothing declared yet.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            family: String::new(),
            summary: String::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Vec::new(),
            context: Context::NONE,
            source: OpSource::Builtin,
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

    /// Sets its context.
    pub fn context(mut self, context: Context) -> Self {
        self.context = context;
        self
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

/// What an operation gets for one piece.
pub struct Call<'a, D: Domain> {
    /// Its connected inputs, by port name (an optional input left unconnected is absent).
    pub inputs: BTreeMap<String, D::Data>,
    /// Every parameter, checked, defaults filled in.
    pub params: &'a BTreeMap<String, ParamValue>,
    /// The piece: what it owns and what was read around it.
    pub piece: &'a Piece,
    /// The app's domain (to reach its device, its exchange for scripts).
    pub domain: &'a D,
    /// The node it runs as (for messages).
    pub node: &'a str,
}

impl<D: Domain> Call<'_, D> {
    /// The input `name`, or why it is missing.
    pub fn take(&mut self, name: &str) -> Result<D::Data, String> {
        self.inputs.remove(name).ok_or_else(|| format!("no input {name}"))
    }

    /// The number parameter `name` (whole numbers too).
    pub fn number(&self, name: &str) -> Result<f64, String> {
        self.params.get(name).and_then(ParamValue::as_f64).ok_or_else(|| format!("no number {name}"))
    }
}

/// An operation an app runs: its own Rust code, or a script through the worker
/// ([`crate::script::ScriptOp`]).
pub trait Operation<D: Domain> {
    /// What it is.
    fn spec(&self) -> &OpSpec;

    /// Runs it on one piece; returns every declared output, by port name. Its data covers the
    /// piece and the context read around it, as its inputs did.
    fn run(&self, call: Call<'_, D>) -> Result<BTreeMap<String, D::Data>, String>;
}

/// An operation from a closure, for an app's simple operations and for tests.
pub struct FnOp<D: Domain, F> {
    spec: OpSpec,
    run: F,
    _domain: std::marker::PhantomData<fn(&D)>,
}

impl<D: Domain, F> FnOp<D, F>
where
    F: Fn(Call<'_, D>) -> Result<BTreeMap<String, D::Data>, String>,
{
    /// `spec` run by `run`.
    pub fn new(spec: OpSpec, run: F) -> Self {
        Self { spec, run, _domain: std::marker::PhantomData }
    }
}

impl<D: Domain, F> Operation<D> for FnOp<D, F>
where
    F: Fn(Call<'_, D>) -> Result<BTreeMap<String, D::Data>, String>,
{
    fn spec(&self) -> &OpSpec {
        &self.spec
    }

    fn run(&self, call: Call<'_, D>) -> Result<BTreeMap<String, D::Data>, String> {
        (self.run)(call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blur() -> OpSpec {
        OpSpec::new("gaussian_blur", "Gaussian blur")
            .family("Filter")
            .input(PortSpec::new("image", "image"))
            .output(PortSpec::new("image", "image"))
            .param(ParamSpec::number("sigma", 2.0).range(0.2, 20.0))
            .context(Context::around(6, 2))
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
