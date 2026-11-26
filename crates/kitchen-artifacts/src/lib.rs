//! What the neuro-kitchen tools read and write, as the orchestrator sees it. Every step of a chain
//! reads and writes **artifacts**: a raw recording, an NWB-Zarr or OME-Zarr store in the middle, a
//! sorting or labels store, Parquet tables at the end. The orchestrator connects tools only by the
//! artifacts' kinds, so it never knows what a spike or a cell is.
//!
//! - [`ArtifactKind`]: `family/format` (`zarr/nwb`, `table/parquet`, `raw/spikeglx`), with the
//!   patterns a tool accepts (`raw/*`).
//! - [`identify`] / [`ArtifactId`]: content identity of a file or a store (a Zarr folder): the
//!   same bytes, the same id, wherever they are.
//! - [`Provenance`]: what an artifact was computed from, by what, how; written next to it.
//! - [`RunKey`]: the hash of everything that decides a run's result, so a finished run is reused.
//!
//! Every type here is data (serde): the orchestrator's database, a tool's description and a
//! provenance file all hold the same JSON.

mod error;
pub mod identity;
pub mod kind;
pub mod provenance;
pub mod run_key;

pub use error::{ArtifactError, ArtifactResult};
pub use identity::{ArtifactId, Identity, Manifest, ManifestEntry, identify};
pub use kind::{ArtifactKind, KindPattern};
pub use provenance::{ArtifactRef, PROVENANCE_VERSION, Provenance, ToolRef};
pub use run_key::{RunKey, canonical_json, recipe_id};

/// The provenance file at a store's root. Zarr readers ignore files they do not know, and the
/// store's identity leaves it out, so writing it does not change the id it records.
pub const STORE_PROVENANCE_FILE: &str = "kitchen-provenance.json";
