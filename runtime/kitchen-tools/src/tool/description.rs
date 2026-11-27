//! What a tool offers: its operations, what each reads and writes, how its recipe looks.

use kitchen_contract::{Format, Type};
use serde::{Deserialize, Serialize};

use crate::tool::TOOL_PROTOCOL;
use crate::tool::protocol::ToolRef;

/// A tool's answer to `describe`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: String,
    pub version: String,
    /// The tool protocol version it speaks ([`TOOL_PROTOCOL`]).
    pub tool_protocol: u32,
    pub operations: Vec<Operation>,
}

impl ToolDescription {
    /// The handshake: whether the orchestrator can drive it (the same tool protocol version).
    pub fn compatible(&self) -> Result<(), String> {
        match self.tool_protocol {
            TOOL_PROTOCOL => Ok(()),
            p if p > TOOL_PROTOCOL => Err(format!(
                "{} speaks tool protocol {p}; this orchestrator speaks {TOOL_PROTOCOL}: update the orchestrator",
                self.name
            )),
            p => Err(format!(
                "{} speaks tool protocol {p}; this orchestrator speaks {TOOL_PROTOCOL}: update the tool",
                self.name
            )),
        }
    }

    pub fn operation(&self, name: &str) -> Option<&Operation> {
        self.operations.iter().find(|o| o.name == name)
    }

    /// The tool as provenance names it (the image is the runner's to add).
    pub fn tool_ref(&self) -> ToolRef {
        ToolRef { name: self.name.clone(), version: self.version.clone(), image: None }
    }

    /// Every operation's ports and names well formed: unique operation names, unique port names
    /// within an operation, at least one output.
    pub fn check(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for op in &self.operations {
            if !seen.insert(op.name.as_str()) {
                return Err(format!("{}: operation {} twice", self.name, op.name));
            }
            let mut ports = std::collections::BTreeSet::new();
            for name in op.inputs.iter().map(|p| &p.name).chain(op.outputs.iter().map(|p| &p.name)) {
                if !ports.insert(name.as_str()) {
                    return Err(format!("{}.{}: port {name} twice", self.name, op.name));
                }
            }
            if op.outputs.is_empty() {
                return Err(format!("{}.{}: no outputs", self.name, op.name));
            }
        }
        Ok(())
    }
}

/// One thing a tool does (`convert`, `sort`, `segment`, `extract`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    /// One line, for people choosing it.
    pub summary: String,
    pub inputs: Vec<InputPort>,
    pub outputs: Vec<OutputPort>,
    /// The JSON Schema of the operation's recipe (the tool's session or pipeline spec): the
    /// orchestrator checks a recipe against it before anything runs.
    #[serde(default)]
    pub recipe_schema: serde_json::Value,
    #[serde(default)]
    pub resources: Resources,
}

impl Operation {
    /// The input port that accepts a result of type `ty` in `format`, if one does.
    pub fn input_for(&self, ty: &Type, format: &Format) -> Option<&InputPort> {
        self.inputs.iter().find(|p| p.accepts(ty, format))
    }
}

/// What an operation reads: a result of the contract's type `ty`, in one of `formats` (any format
/// when empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputPort {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<Format>,
    /// Takes several artifacts (a folder of recordings), not one.
    #[serde(default)]
    pub many: bool,
    #[serde(default)]
    pub optional: bool,
}

impl InputPort {
    /// Whether it takes a result of type `ty` in `format`.
    pub fn accepts(&self, ty: &Type, format: &Format) -> bool {
        ty.feeds(&self.ty) && (self.formats.is_empty() || self.formats.contains(format))
    }

    /// What it takes, for messages (`signal in nwb-zarr or raw`).
    pub fn takes(&self) -> String {
        if self.formats.is_empty() {
            self.ty.to_string()
        } else {
            format!("{} in {}", self.ty, self.formats.iter().map(Format::as_str).collect::<Vec<_>>().join(" or "))
        }
    }
}

/// What an operation writes: a result of the contract's type `ty` in `format`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputPort {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub format: Format,
}

/// What an operation needs to run, so the orchestrator sends it where it can.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    /// Needs a GPU (it still runs on the CPU runtime, slower, when none is there).
    #[serde(default)]
    pub gpu: bool,
    /// Memory it needs, about (GiB).
    #[serde(default)]
    pub memory_gib: Option<f32>,
}
