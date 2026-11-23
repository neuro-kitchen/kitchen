//! What the neuro-kitchen tools read and write, as the orchestrator sees it. Every step of a chain
//! reads and writes **artifacts**: a raw recording, an NWB-Zarr or OME-Zarr store in the middle, a
//! sorting or labels store, Parquet tables at the end. The orchestrator connects tools only by the
//! artifacts' kinds, so it never knows what a spike or a cell is.
//!
//! - [`ArtifactKind`]: `family/format` (`zarr/nwb`, `table/parquet`, `raw/spikeglx`), with the
//!   patterns a tool accepts (`raw/*`).
//!
//! Every type here is data (serde): the orchestrator's database, a tool's description and a
//! provenance file all hold the same JSON.

mod error;
pub mod kind;

pub use error::{ArtifactError, ArtifactResult};
pub use kind::{ArtifactKind, KindPattern};
