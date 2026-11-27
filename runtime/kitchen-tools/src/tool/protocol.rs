//! The requests a tool answers beside the plugin protocol's own (`info`, `install`, `remove`,
//! `quit`). Each is one JSON line with a `"request"` tag; the answer is the plugin protocol's
//! `progress` events, then one `done` (its `result` as below) or `error`.

use std::path::PathBuf;

use kitchen_contract::{ContentId, Format, RunKey, Type};
use serde::{Deserialize, Serialize};

/// The tool protocol version this crate speaks: in [`crate::tool::ToolDescription::tool_protocol`],
/// checked by the orchestrator. The plugin protocol under it has its own version. Version 2 names
/// ports and results by the contract's type and format (version 1: artifact kinds).
pub const TOOL_PROTOCOL: u32 = 2;

/// A request to a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum ToolRequest {
    /// Answered with a [`crate::tool::ToolDescription`].
    Describe,
    /// Answered with a [`Validation`].
    Validate { operation: String, recipe: serde_json::Value },
    /// Answered with a [`crate::tool::RunReport`].
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
    /// This run's key: what decides its result, so a finished one is reused. The fields are the
    /// same as before the contract (tool, version, image, operation, recipe, named inputs,
    /// parameters), so earlier keys still match.
    pub fn run_key(&self, tool: &ToolRef) -> RunKey {
        let mut named: Vec<(&str, &str)> = self.inputs.iter().map(|i| (i.name.as_str(), i.id.as_str())).collect();
        named.sort();
        RunKey::from_fields(&serde_json::json!({
            "key": "kitchen-run-v1",
            "tool": tool.name,
            "version": tool.version,
            "image": tool.image,
            "operation": self.operation,
            "recipe": ContentId::of_json(&self.recipe).as_str(),
            "inputs": named,
            "params": self.params,
        }))
    }
}

/// An input result, on a path the tool can read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunInput {
    /// The input port it is for.
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub format: Format,
    pub path: PathBuf,
    pub id: ContentId,
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

    #[test]
    fn the_same_run_has_the_same_key() {
        let tool = ToolRef { name: "dspk".into(), version: "0.0.13".into(), image: None };
        let input = |name: &str, c: char| RunInput {
            name: name.into(),
            ty: Type::new("signal"),
            format: Format::parse("nwb-zarr").unwrap(),
            path: "/data/x".into(),
            id: ContentId::parse(&format!("sha256:{}", c.to_string().repeat(64))).unwrap(),
        };
        let reordered: serde_json::Value = serde_json::from_str(r#"{"threshold": 6, "sorter": "ks4"}"#).unwrap();
        let run = |inputs: Vec<RunInput>, recipe: serde_json::Value| RunRequest { operation: "sort".into(), recipe, inputs, output_dir: "/out".into(), params: serde_json::Value::Null };
        let key = run(vec![input("recording", 'a'), input("probe", 'b')], serde_json::json!({"sorter": "ks4", "threshold": 6})).run_key(&tool);
        assert_eq!(key, run(vec![input("probe", 'b'), input("recording", 'a')], reordered.clone()).run_key(&tool), "order does not count");
        assert_ne!(key, run(vec![input("recording", 'a'), input("probe", 'c')], reordered.clone()).run_key(&tool));
        let newer = ToolRef { version: "0.0.14".into(), ..tool.clone() };
        assert_ne!(key, run(vec![input("recording", 'a'), input("probe", 'b')], reordered).run_key(&newer));
    }
}
