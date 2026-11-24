//! The pipeline the neuro-kitchen apps share.
//!
//! Every app (lucem, dsp-kitchen, garnish, the next one) runs a graph of operations over its
//! data, takes operations written in Python and in Rust as well as its own, and walks its data its
//! own way: lucem in tiles of a slide with a margin around each, dsp-kitchen in windows of a
//! recording with a margin before and after. This crate is what they share; the walk is the app's.
//!
//! ```text
//!            the app (a Domain)                       kitchen-pipeline
//!   ┌───────────────────────────────┐   ┌──────────────────────────────────────────────┐
//!   │ port types (image, signal…)   │   │ Graph: nodes, edges, params (JSON, versioned)│
//!   │ pieces: tiles / windows       │◄──│ Catalog: OpSpecs from Rust, scripts, plugins │
//!   │ read, trim, write a piece     │   │ check: types, ports, params, loops, context  │
//!   │ its Rust operations (Operation)│──►│ scripts: decorators → OpSpec; check, confirm │
//!   │ arrays ↔ exchange files       │   │ Engine: piece by piece, progress, cancel     │
//!   └───────────────────────────────┘   └──────────────────────────────────────────────┘
//! ```
//!
//! - [`types`]: what flows ([`PortType`], declared by the app), parameters ([`ParamSpec`],
//!   [`ParamValue`]) and how much context an operation needs around a piece ([`Context`]).
//! - [`op`]: an operation's description ([`OpSpec`]), where it comes from ([`OpSource`]) and
//!   how an app's Rust operation runs ([`Operation`]).
//! - [`catalog`]: every operation an app offers, by id.
//! - [`graph`]: a pipeline as data ([`Graph`]) and its check ([`Graph::check`]), the order to run
//!   it in and the context a piece needs.
//! - [`script`]: operations written as scripts. A decorated Python function
//!   (`@kitchen.op`, or the app's own name for it: `@lucem.op`, `@dspk.op`) or a Rust file with a
//!   `kitchen:` header: its signature becomes ports and parameters ([`script::interface`]), a
//!   probe checks it imports and declares what the text says ([`script::probe`]), and a person
//!   confirms it before it runs in a pipeline ([`script::review`]). [`script::worker`] runs it, out
//!   of process.
//! - [`domain`]: what an app provides ([`Domain`]), and the two walks the apps use today
//!   ([`domain::windows`], [`domain::tiles`]).
//! - [`engine`]: runs a checked graph over a domain, piece by piece.
//!
//! Python users get the decorators from `python/kitchen_pipeline.py` (shipped in each app's
//! Python package, and given to every script by the worker, so a script runs without it
//! installed).

pub mod catalog;
pub mod domain;
pub mod engine;
pub mod graph;
pub mod op;
pub mod script;
pub mod types;

pub use catalog::Catalog;
pub use domain::{Domain, Piece};
pub use engine::{Cancel, Engine, Progress, RunReport};
pub use graph::{Edge, Graph, GRAPH_VERSION, Node, Problem};
pub use op::{OpSource, OpSpec, Operation};
pub use types::{Context, ParamKind, ParamSpec, ParamValue, PortSpec, PortType};

/// Why a pipeline could not be read, checked or run.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// The graph's JSON does not read.
    #[error("pipeline: {0}")]
    Parse(String),
    /// The graph does not check: every problem found.
    #[error("the pipeline has {} problem(s): {}", .0.len(), .0.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Problem>),
    /// A script was not checked and confirmed for what it is now.
    #[error("{0}")]
    Unconfirmed(String),
    /// An operation failed on a piece.
    #[error("{node}: {message}")]
    Op {
        /// The node that failed.
        node: String,
        /// Why.
        message: String,
    },
    /// Reading or writing the app's data.
    #[error("{0}")]
    Io(String),
    /// The run was cancelled.
    #[error("cancelled")]
    Cancelled,
}

/// A result of this crate.
pub type Result<T> = std::result::Result<T, PipelineError>;
