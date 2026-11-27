//! Provenance: what an artifact was computed from, by what, how. Written next to every artifact
//! a run produces, so the record travels with the data (copied, archived, shared), and kept in
//! the orchestrator's database for lineage queries.
//!
//! - A **store**: [`STORE_PROVENANCE_FILE`] at its root (Zarr readers ignore it; its identity
//!   leaves it out).
//! - A **file**: `<file>.provenance.json` beside it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::run_key::{RunKey, recipe_id, tool_run_key};
use crate::{ArtifactError, ArtifactId, ArtifactKind, ArtifactResult, STORE_PROVENANCE_FILE};

/// The version of the provenance format this code writes. Bump it with every change an older
/// reader would misread; readers refuse a newer one.
pub const PROVENANCE_VERSION: u32 = 1;

/// An artifact as a run names it: its id, its kind, and the port it was read from or written to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    /// The tool's name for it (`recording`, `labels`); empty when it has none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// The tool that ran: its name and version, and the container image when it ran in one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRef {
    pub name: String,
    pub version: String,
    /// The image by digest (`ghcr.io/neuro-kitchen/dspk@sha256:…`), never by a moving tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

/// What an artifact was computed from, by what, how.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub schema_version: u32,
    /// The artifact this record is about.
    pub artifact: ArtifactRef,
    pub tool: ToolRef,
    /// The tool's operation (`sort`, `segment`).
    pub operation: String,
    /// The recipe's id (the SHA-256 of its canonical JSON): the recipe itself is an artifact of
    /// the project, kept by the orchestrator.
    pub recipe: ArtifactId,
    pub inputs: Vec<ArtifactRef>,
    /// The run's own parameters, beside the recipe.
    #[serde(default)]
    pub params: serde_json::Value,
    /// The run this artifact came from: the same key, the same result.
    pub run_key: RunKey,
    pub started_unix: u64,
    pub finished_unix: u64,
    /// Where it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl Provenance {
    /// The provenance of an output of a run, with its run key computed from the same fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact: ArtifactRef,
        tool: ToolRef,
        operation: &str,
        recipe: &serde_json::Value,
        inputs: Vec<ArtifactRef>,
        params: serde_json::Value,
        started_unix: u64,
        finished_unix: u64,
    ) -> Self {
        let run_key = tool_run_key(&tool, operation, recipe, &inputs, &params);
        Self {
            schema_version: PROVENANCE_VERSION,
            artifact,
            tool,
            operation: operation.to_string(),
            recipe: recipe_id(recipe),
            inputs,
            params,
            run_key,
            started_unix,
            finished_unix,
            host: None,
        }
    }

    /// Writes the record for the artifact at `artifact_path` (inside a store, or beside a file),
    /// whole and renamed into place so a reader never sees half of it.
    pub fn write(&self, artifact_path: &Path) -> ArtifactResult<PathBuf> {
        let path = provenance_path(artifact_path)?;
        let json = serde_json::to_vec_pretty(self).map_err(|e| ArtifactError::Invalid(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| ArtifactError::io(&tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| ArtifactError::io(&path, e))?;
        Ok(path)
    }

    /// Reads the record of the artifact at `artifact_path`; a newer format is refused.
    pub fn read(artifact_path: &Path) -> ArtifactResult<Self> {
        let path = provenance_path(artifact_path)?;
        let bytes = std::fs::read(&path).map_err(|e| ArtifactError::io(&path, e))?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| ArtifactError::Invalid(format!("{}: {e}", path.display())))?;
        let version = value.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version > u64::from(PROVENANCE_VERSION) {
            return Err(ArtifactError::Invalid(format!(
                "{}: provenance version {version} is newer than this reader ({PROVENANCE_VERSION})",
                path.display()
            )));
        }
        serde_json::from_value(value).map_err(|e| ArtifactError::Invalid(format!("{}: {e}", path.display())))
    }
}

/// Where the record of the artifact at `artifact_path` lives.
pub fn provenance_path(artifact_path: &Path) -> ArtifactResult<PathBuf> {
    let meta = std::fs::metadata(artifact_path).map_err(|e| ArtifactError::io(artifact_path, e))?;
    Ok(if meta.is_dir() {
        artifact_path.join(STORE_PROVENANCE_FILE)
    } else {
        let mut name = artifact_path.as_os_str().to_owned();
        name.push(".provenance.json");
        PathBuf::from(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{identify, kind};

    #[test]
    fn written_beside_a_file_and_inside_a_store_without_changing_ids() {
        let dir = std::env::temp_dir().join(format!("kitchen-artifacts-prov-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = dir.join("labels.zarr");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("zarr.json"), b"{}").unwrap();
        let table = dir.join("cells.parquet");
        std::fs::write(&table, b"PAR1").unwrap();

        let input = ArtifactRef {
            id: identify(&table).unwrap().id,
            kind: ArtifactKind::parse(kind::ZARR_OME).unwrap(),
            name: "image".into(),
        };
        let tool = ToolRef { name: "lucem".into(), version: "0.0.6".into(), image: None };
        for (path, k) in [(&store, kind::ZARR_LABELS), (&table, kind::TABLE_PARQUET)] {
            let before = identify(path).unwrap().id;
            let artifact =
                ArtifactRef { id: before.clone(), kind: ArtifactKind::parse(k).unwrap(), name: "out".into() };
            let record = Provenance::new(
                artifact,
                tool.clone(),
                "segment",
                &serde_json::json!({"b": 1, "a": 2}),
                vec![input.clone()],
                serde_json::Value::Null,
                1,
                2,
            );
            record.write(path).unwrap();
            assert_eq!(identify(path).unwrap().id, before, "the record does not change the artifact's id");
            assert_eq!(Provenance::read(path).unwrap(), record);
        }
        assert!(store.join(STORE_PROVENANCE_FILE).exists());
        assert!(dir.join("cells.parquet.provenance.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
