//! The engine the neuro-kitchen apps share: runs a checked pipeline over an app's data, piece by
//! piece ([standard §4.3](../../../standard/spec/04-pipelines.md)).
//!
//! Every app (lucem, dsp-kitchen, garnish, the next one) walks its data its own way: lucem in tiles
//! of a slide with a margin around each, dsp-kitchen in windows of a recording with a margin before
//! and after. The walk is the app's ([`Domain`]); this crate runs the ops over it.
//!
//! ```text
//!            the app (a Domain)                        kitchen-engine
//!   ┌────────────────────────────────┐   ┌─────────────────────────────────────────────┐
//!   │ its types (image, signal…)     │   │ the contract's Pipeline, Catalog and check   │
//!   │ pieces: tiles / windows        │◄──│ Engine: piece by piece, progress, cancel     │
//!   │ read, trim, write a piece      │   │ preview: one piece, returned, not written    │
//!   │ its Rust ops (Operation)       │──►│                                              │
//!   │ arrays ↔ exchange files        │   │ (script ops: kitchen-scripts)                │
//!   └────────────────────────────────┘   └─────────────────────────────────────────────┘
//! ```
//!
//! - [`domain`]: what an app provides ([`Domain`]), and the two walks the apps use today
//!   ([`domain::windows`], [`domain::tiles`]).
//! - [`op`]: how an op runs ([`Operation`], [`FnOp`], [`Call`]).
//! - [`engine`]: runs a checked pipeline over a domain, piece by piece.
//! - [`types`], [`graph`], [`catalog`]: the contract's types under the names this code used
//!   before the standard ([`PortType`] = `Type`, [`Context`] = `Margin`, [`Graph`] = `Pipeline`).
//!
//! Script ops (Python and Rust files) are kitchen-scripts': it implements [`Operation`] for them.

pub mod catalog;
pub mod domain;
pub mod engine;
pub mod graph;
pub mod op;
pub mod types;

pub use catalog::Catalog;
pub use domain::{Domain, Exchange, ExchangeFile, Piece, Region};
pub use engine::{Cancel, Engine, Progress, RunReport};
pub use graph::{Edge, Endpoint, GRAPH_VERSION, Graph, Node, Plan, Problem};
pub use op::{Call, FnOp, Language, OpSource, OpSpec, Operation};
pub use types::{Context, ParamKind, ParamSpec, ParamValue, PortSpec, PortType};

/// Why a pipeline could not be read, checked or run.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The graph's JSON does not read.
    #[error("pipeline: {0}")]
    Parse(String),
    /// The graph does not check: every problem found.
    #[error("the pipeline has {} problem(s): {}", .0.len(), .0.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Problem>),
    /// A script op is not checked for what it is now.
    #[error("{0}")]
    Unchecked(String),
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
pub type Result<T> = std::result::Result<T, EngineError>;

/// The name kitchen-pipeline used before the split.
pub type PipelineError = EngineError;
