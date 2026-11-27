//! The neuro-kitchen store ([standard §3](../../../standard/spec/03-results.md)): what runs read and
//! write, known by content, with the record of how each was made.
//!
//! - [`Workspace`]: the store on a person's machine, next to the data (`.<app>/runs/`,
//!   `.<app>/results/`): run records as JSON files, results kept by reference and checked when
//!   read, lineage back ([`Workspace::lineage`]) and forward ([`Workspace::made_from`]), export.
//!   A lab's orchestrator keeps the same records in SQLite.
//! - [`identify`]: content identity of a file or a store (a Zarr folder): the same bytes, the same
//!   id, wherever they are. The id is the contract's `ContentId` ([`ArtifactId`]).
//!
//! A run's history is the contract's `RunRecord`, for the app's own ops and the tools' alike. The
//! tools' provenance files and artifact kinds (kitchen-artifacts) gave way to it in phase 2,
//! step 3d; old kinds still read as type and format with `kitchen_contract::types::from_kind`.
//!
//! This crate was kitchen-artifacts until phase 2, step 3b.

pub mod arrays;
mod error;
pub mod identity;
pub mod workspace;

pub use arrays::{ArrayInfo, array_info, read_region};
pub use error::{ArtifactError, ArtifactResult};
pub use identity::{ArtifactId, Identity, Manifest, ManifestEntry, identify};
pub use kitchen_contract::{RunKey, canonical_json};
pub use workspace::{Kept, Workspace};

/// The store's error (the name kitchen-artifacts gave it).
pub type StoreError = ArtifactError;

/// The provenance file kitchen-artifacts wrote at a store's root. A store's identity still leaves
/// it out, so stores written before the contract keep their ids.
pub const STORE_PROVENANCE_FILE: &str = "kitchen-provenance.json";
