//! The orchestrator's side: a tool program started once and asked many times. Blocking: the
//! orchestrator's worker calls it from a thread of its own.

use crate::plugin::PluginProcess;
use serde_json::Value;

use crate::tool::{Operation, RunInput, RunReport, RunRequest, ToolDescription, ToolRequest, Validation};

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

/// Every input on a port of `operation` that accepts its type and format; one per port unless the
/// port takes many; every port that is not optional given.
pub fn check_inputs(operation: &Operation, inputs: &[RunInput]) -> Result<(), String> {
    for input in inputs {
        let port = operation
            .inputs
            .iter()
            .find(|p| p.name == input.name)
            .ok_or_else(|| format!("{}: no input port {}", operation.name, input.name))?;
        if !port.accepts(&input.ty, &input.format) {
            return Err(format!("{}.{}: takes {}, not {} in {}", operation.name, port.name, port.takes(), input.ty, input.format));
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
    use crate::tool::{InputPort, OutputPort};
    use kitchen_contract::{ContentId, Format, Type};

    fn input(name: &str, ty: &str, format: &str) -> RunInput {
        RunInput {
            name: name.into(),
            ty: Type::new(ty),
            format: Format::parse(format).unwrap(),
            path: "/data/x".into(),
            id: ContentId::parse(&format!("sha256:{}", "a".repeat(64))).unwrap(),
        }
    }

    #[test]
    fn inputs_are_checked_against_the_ports() {
        let op = Operation {
            name: "sort".into(),
            summary: String::new(),
            inputs: vec![
                InputPort { name: "recording".into(), ty: Type::new("signal"), formats: vec![Format::parse("nwb-zarr").unwrap()], many: false, optional: false },
                InputPort { name: "probe".into(), ty: Type::new("file"), formats: vec![], many: false, optional: true },
            ],
            outputs: vec![OutputPort { name: "sorting".into(), ty: Type::new("dspk:sorting"), format: Format::parse("zarr").unwrap() }],
            recipe_schema: Value::Null,
            resources: Default::default(),
        };
        assert!(check_inputs(&op, &[input("recording", "signal", "nwb-zarr")]).is_ok());
        assert!(check_inputs(&op, &[]).is_err(), "a required port is missing");
        let wrong = check_inputs(&op, &[input("recording", "image", "ome-zarr")]).unwrap_err();
        assert!(wrong.contains("takes signal in nwb-zarr, not image in ome-zarr"), "{wrong}");
        assert!(check_inputs(&op, &[input("recording", "signal", "raw")]).is_err(), "the wrong format");
        assert!(check_inputs(&op, &[input("recording", "signal", "nwb-zarr"), input("recording", "signal", "nwb-zarr")]).is_err(), "one only");
        assert!(check_inputs(&op, &[input("recording", "signal", "nwb-zarr"), input("probe", "file", "json")]).is_ok(), "any format");
    }
}
