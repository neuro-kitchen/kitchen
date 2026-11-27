//! How an op runs ([`Operation`]): an app's own Rust code, or a script through the worker. What an
//! op *is* ([`OpSpec`], [`OpSource`], [`Language`]) is the contract's.

use std::collections::BTreeMap;

pub use kitchen_contract::{Language, OpSource, OpSpec};

use crate::domain::{Domain, Piece};
use crate::types::ParamValue;

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
