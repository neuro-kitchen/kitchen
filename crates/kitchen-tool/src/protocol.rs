//! The requests a tool answers beside the plugin protocol's own (`info`, `install`, `remove`,
//! `quit`). Each is one JSON line with a `"request"` tag; the answer is the plugin protocol's
//! `progress` events, then one `done` (its `result` as below) or `error`.

use std::path::PathBuf;

use kitchen_artifacts::{ArtifactId, ArtifactKind, ArtifactRef, RunKey, ToolRef};
use serde::{Deserialize, Serialize};

/// The tool protocol version this crate speaks: in [`crate::ToolDescription::tool_protocol`],
/// checked by the orchestrator. The plugin protocol under it has its own version.
pub const TOOL_PROTOCOL: u32 = 1;

/// A request to a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum ToolRequest {
    /// Answered with a [`crate::ToolDescription`].
    Describe,
    /// Answered with a [`Validation`].
    Validate { operation: String, recipe: serde_json::Value },
    /// Answered with a [`crate::RunReport`].
    Run(RunRequest),
}

/// The answer to `validate`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Validation {
    /// What is wrong with the recipe, in words a person can act on; empty when it can run.
    pub problems: Vec<String>,
}

impl Validation {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Run an operation on input artifacts, writing its outputs into `output_dir` (which exists and
/// is empty; the tool writes nothing elsewhere).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRequest {
    pub operation: String,
    pub recipe: serde_json::Value,
    pub inputs: Vec<RunInput>,
    pub output_dir: PathBuf,
    /// The run's own parameters beside the recipe (a channel range, a region).
    #[serde(default)]
    pub params: serde_json::Value,
}

impl RunRequest {
    /// This run's key: what decides its result, so a finished one is reused.
    pub fn run_key(&self, tool: &ToolRef) -> RunKey {
        RunKey::new(tool, &self.operation, &self.recipe, &self.input_refs(), &self.params)
    }

    /// The inputs as provenance names them.
    pub fn input_refs(&self) -> Vec<ArtifactRef> {
        self.inputs
            .iter()
            .map(|i| ArtifactRef { id: i.id.clone(), kind: i.kind.clone(), name: i.name.clone() })
            .collect()
    }
}

/// An input artifact, on a path the tool can read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunInput {
    /// The input port it is for.
    pub name: String,
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub id: ArtifactId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_tagged_json_lines() {
        assert_eq!(serde_json::to_string(&ToolRequest::Describe).unwrap(), r#"{"request":"describe"}"#);
        let run = ToolRequest::Run(RunRequest {
            operation: "sort".into(),
            recipe: serde_json::json!({"sorter": "ks4"}),
            inputs: vec![],
            output_dir: "/out".into(),
            params: serde_json::Value::Null,
        });
        let line = serde_json::to_string(&run).unwrap();
        assert!(line.starts_with(r#"{"request":"run","operation":"sort""#), "{line}");
        assert_eq!(serde_json::from_str::<ToolRequest>(&line).unwrap(), run);
    }
}
