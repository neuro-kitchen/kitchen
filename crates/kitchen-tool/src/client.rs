//! The orchestrator's side: a tool program started once and asked many times. Blocking: the
//! orchestrator's worker calls it from a thread of its own.

use kitchen_plugin::PluginProcess;
use serde_json::Value;

use crate::{Operation, RunInput, RunReport, RunRequest, ToolDescription, ToolRequest, Validation};

/// A running tool.
pub struct ToolClient {
    process: PluginProcess,
    description: ToolDescription,
}

impl ToolClient {
    /// Starts `program args…` (`dspk tool serve`, or `podman run … dspk tool serve`) and shakes
    /// hands: its description is read, its tool protocol checked, its operations checked.
    pub fn start(program: &str, args: &[&str]) -> Result<Self, String> {
        let mut process = PluginProcess::start_command(program, args)?;
        let answer = process.request(&ToolRequest::Describe, &mut |_, _| {})?;
        let description: ToolDescription =
            serde_json::from_value(answer).map_err(|e| format!("{program}: not a tool description: {e}"))?;
        description.compatible()?;
        description.check()?;
        Ok(Self { process, description })
    }

    pub fn description(&self) -> &ToolDescription {
        &self.description
    }

    /// Whether `recipe` is one `operation` can run, and if not, why.
    pub fn validate(&mut self, operation: &str, recipe: &Value) -> Result<Validation, String> {
        self.operation(operation)?;
        let request = ToolRequest::Validate { operation: operation.to_string(), recipe: recipe.clone() };
        let answer = self.process.request(&request, &mut |_, _| {})?;
        serde_json::from_value(answer).map_err(|e| format!("{}: not a validation: {e}", self.description.name))
    }

    /// Runs `request`: its inputs are checked against the operation's ports first, its report
    /// against the operation's outputs after. `progress` sees the tool's progress events.
    pub fn run(
        &mut self,
        request: &RunRequest,
        progress: &mut dyn FnMut(&str, Option<f32>),
    ) -> Result<RunReport, String> {
        let operation = self.operation(&request.operation)?.clone();
        check_inputs(&operation, &request.inputs)?;
        let answer = self.process.request(&ToolRequest::Run(request.clone()), progress)?;
        let report: RunReport =
            serde_json::from_value(answer).map_err(|e| format!("{}: not a run report: {e}", self.description.name))?;
        report.check(&operation, &request.output_dir)?;
        Ok(report)
    }

    /// Whether the program is still running (a crash ends it; start it again).
    pub fn alive(&mut self) -> bool {
        self.process.alive()
    }

    fn operation(&self, name: &str) -> Result<&Operation, String> {
        self.description.operation(name).ok_or_else(|| format!("{} has no operation {name}", self.description.name))
    }
}

/// Every input on a port of `operation` that accepts its kind; one per port unless the port takes
/// many; every port that is not optional given.
pub fn check_inputs(operation: &Operation, inputs: &[RunInput]) -> Result<(), String> {
    for input in inputs {
        let port = operation
            .inputs
            .iter()
            .find(|p| p.name == input.name)
            .ok_or_else(|| format!("{}: no input port {}", operation.name, input.name))?;
        if !port.accepts.iter().any(|pattern| pattern.matches(&input.kind)) {
            let accepts: Vec<&str> = port.accepts.iter().map(|p| p.as_str()).collect();
            return Err(format!(
                "{}.{}: takes {}, not {}",
                operation.name,
                port.name,
                accepts.join(" or "),
                input.kind
            ));
        }
    }
    for port in &operation.inputs {
        let given = inputs.iter().filter(|i| i.name == port.name).count();
        if given == 0 && !port.optional {
            return Err(format!("{}.{}: no input given", operation.name, port.name));
        }
        if given > 1 && !port.many {
            return Err(format!("{}.{}: takes one input, {given} given", operation.name, port.name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputPort, OutputPort};
    use kitchen_artifacts::{ArtifactId, ArtifactKind, KindPattern};

    fn input(name: &str, kind: &str) -> RunInput {
        RunInput {
            name: name.into(),
            kind: ArtifactKind::parse(kind).unwrap(),
            path: "/data/x".into(),
            id: ArtifactId::parse(&format!("sha256:{}", "a".repeat(64))).unwrap(),
        }
    }

    #[test]
    fn inputs_are_checked_against_the_ports() {
        let op = Operation {
            name: "sort".into(),
            summary: String::new(),
            inputs: vec![
                InputPort {
                    name: "recording".into(),
                    accepts: vec![KindPattern::parse("zarr/nwb").unwrap()],
                    many: false,
                    optional: false,
                },
                InputPort {
                    name: "probe".into(),
                    accepts: vec![KindPattern::parse("file/*").unwrap()],
                    many: false,
                    optional: true,
                },
            ],
            outputs: vec![OutputPort { name: "sorting".into(), kind: ArtifactKind::parse("zarr/sorting").unwrap() }],
            recipe_schema: Value::Null,
            resources: Default::default(),
        };
        assert!(check_inputs(&op, &[input("recording", "zarr/nwb")]).is_ok());
        assert!(check_inputs(&op, &[]).is_err(), "a required port is missing");
        assert!(check_inputs(&op, &[input("recording", "zarr/ome")]).is_err(), "the wrong kind");
        assert!(
            check_inputs(&op, &[input("recording", "zarr/nwb"), input("recording", "zarr/nwb")]).is_err(),
            "one only"
        );
        assert!(check_inputs(&op, &[input("recording", "zarr/nwb"), input("probe", "file/json")]).is_ok());
    }
}
