//! The neuro-kitchen store ([standard §3](../../../standard/spec/03-results.md)): what runs read and
//! write, known by content, with the record of how each was made.
//!
//! - [`Workspace`]: the store on a person's machine, next to the data (`.<app>/runs/`,
//!   `.<app>/results/`): run records as JSON files, results kept by reference and checked when
//!   read, lineage back ([`Workspace::lineage`]) and forward ([`Workspace::made_from`]), export.
//!   A lab's orchestrator keeps the same records in SQLite.
//! - [`identify`]: content identity of a file or a store (a Zarr folder): the same bytes, the same
//!   id, wherever they are. The id is the contract's `ContentId` ([`ArtifactId`]).
//! - [`Provenance`]: a tool run's record written next to its output, and [`tool_run_key`]; they
//!   give way to the contract's `RunRecord` when the tools move to the contract's types (phase 2,
//!   step 3d).
//! - [`ArtifactKind`]: the tools' `family/format` kinds, read as the contract's type and format with
//!   `kitchen_contract::types::from_kind`; also until step 3d.
//!
//! This crate was kitchen-artifacts until phase 2, step 3b.

mod error;
pub mod identity;
pub mod kind;
pub mod provenance;
pub mod run_key;
pub mod workspace;

pub use error::{ArtifactError, ArtifactResult};
pub use identity::{ArtifactId, Identity, Manifest, ManifestEntry, identify};
pub use kind::{ArtifactKind, KindPattern};
pub use provenance::{ArtifactRef, PROVENANCE_VERSION, Provenance, ToolRef};
pub use run_key::{RunKey, canonical_json, recipe_id, tool_run_key};
pub use workspace::{Kept, Workspace};

/// The store's error (the name kitchen-artifacts gave it).
pub type StoreError = ArtifactError;

/// The provenance file at a store's root. Zarr readers ignore files they do not know, and the
/// store's identity leaves it out, so writing it does not change the id it records.
pub const STORE_PROVENANCE_FILE: &str = "kitchen-provenance.json";
