//! What a run produced.

use std::path::PathBuf;

use kitchen_store::ArtifactKind;
use serde::{Deserialize, Serialize};

use crate::Operation;

/// A tool's answer to `run`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RunReport {
    pub outputs: Vec<RunOutput>,
    /// What a person should look at. The tool's own checks (a recording with dead channels, a
    /// segmentation with almost no cells); later also a model's. Advisory: it never changes a
    /// result.
    #[serde(default)]
    pub review: Vec<ReviewFlag>,
    /// A few numbers worth showing in the orchestrator's list of runs (units found, cells).
    #[serde(default)]
    pub summary: serde_json::Value,
}

impl RunReport {
    /// Every output a port of `operation`, of that port's kind, under `output_dir`; every output
    /// port written unless the tool says why not in `review`.
    pub fn check(&self, operation: &Operation, output_dir: &std::path::Path) -> Result<(), String> {
        for out in &self.outputs {
            let Some(port) = operation.outputs.iter().find(|p| p.name == out.name) else {
                return Err(format!("{}: no output port {}", operation.name, out.name));
            };
            if port.kind != out.kind {
                return Err(format!("{}.{}: wrote {}, declared {}", operation.name, out.name, out.kind, port.kind));
            }
            if !out.path.starts_with(output_dir) {
                return Err(format!(
                    "{}.{}: {} is outside the run's folder",
                    operation.name,
                    out.name,
                    out.path.display()
                ));
            }
        }
        Ok(())
    }
}

/// One artifact a run wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutput {
    /// Its output port.
    pub name: String,
    pub kind: ArtifactKind,
    pub path: PathBuf,
}

/// Something a person should look at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewFlag {
    /// Why, in words a person can act on.
    pub reason: String,
    /// The output it is about, when it is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}
