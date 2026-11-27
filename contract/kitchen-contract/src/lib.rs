//! The neuro-kitchen pipeline contract, as Rust types: the waist every app, the runtime, a
//! generated interface and an agent agree on ([standard](../../../standard/README.md)).
//!
//! | Module | Standard | Holds |
//! |---|---|---|
//! | [`types`] | §1 | [`Type`] (shared and `app:name`), [`Format`], matching, the shared types |
//! | [`params`] | §2.1 | [`ParamSpec`], [`ParamValue`], [`check_params`] |
//! | [`ops`] | §2 | [`OpSpec`] (version, ports, params, [`Margin`], source, finish, effects, availability) |
//! | [`catalog`] | §2.2, §6.1 | [`Catalog`]: an app's ops, by family, by type, fillable by a screen's offers |
//! | [`pipelines`] | §4 | [`Pipeline`] and its check: every problem at once, or a [`Plan`] |
//! | [`documents`] | §2.2 | documents on disk as JSON or TOML, the same id either way |
//! | [`results`] | §3 | [`ContentId`], [`canonical_json`], [`ResultRecord`], [`RunRecord`], [`RunKey`] |
//!
//! Depends on nothing of ours, does no I/O and draws nothing: `cargo xtask check-layers` keeps it
//! so. Every document carries [`CONTRACT_VERSION`]; its JSON Schemas are generated into
//! `standard/schemas/` (a test fails when they are stale).

pub mod catalog;
pub mod documents;
pub mod ops;
pub mod params;
pub mod pipelines;
pub mod results;
pub mod types;

pub use catalog::Catalog;
pub use ops::{Language, Margin, OpSource, OpSpec, PortSpec};
pub use params::{ParamKind, ParamSpec, ParamValue, check_params};
pub use pipelines::{Edge, Endpoint, Node, PIPELINE_VERSION, Pipeline, Plan, Problem};
pub use results::{ContentId, OpRef, ResultRecord, RunKey, RunRecord, RunStatus, canonical_json};
pub use types::{Axis, Format, Type, TypeDescription};

/// The contract's version, `MAJOR.MINOR` ([standard §7](../../../standard/spec/07-versioning.md)):
/// `0.x` until the first public release, when anything may still change. Confirmation joins in
/// 0.2 (decision 0016).
pub const CONTRACT_VERSION: &str = "0.1";
