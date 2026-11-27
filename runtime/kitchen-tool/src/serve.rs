//! The tool's side: implement [`HeadlessTool`], then answer requests with [`serve_stdio`] (a
//! tool's `tool serve` subcommand: `dspk tool serve`, `lucem tool serve`).
//!
//! The loop answers the plugin protocol's `info` and `quit` too, refuses to run a recipe that does
//! not validate, and checks every report against the operation's declared outputs, so a tool that
//! writes the wrong kind fails loudly instead of feeding the next tool.

use std::io::{BufRead, Write};

use kitchen_plugin::{Event, PROTOCOL, PluginInfo};
use serde_json::Value;

use crate::{Operation, RunReport, RunRequest, ToolDescription, ToolRequest, Validation};

/// A tool the orchestrator can drive.
pub trait HeadlessTool {
    fn description(&self) -> ToolDescription;

    /// Its licence, as the plugin manifest shows it.
    fn licence(&self) -> &str {
        "MIT OR Apache-2.0"
    }

    /// What is wrong with `recipe` for `operation`; empty when it can run. Cheap: no data is
    /// read.
    fn validate(&self, operation: &Operation, recipe: &Value) -> Vec<String>;

    /// Runs `request` (its recipe already validated), reporting progress (`stage`, 0–1 when
    /// known); writes only into `request.output_dir`.
    fn run(&self, request: &RunRequest, progress: &mut dyn FnMut(&str, Option<f32>)) -> Result<RunReport, String>;
}

/// The plugin manifest of `tool`: its name and version, capability `tool`.
pub fn plugin_info(tool: &dyn HeadlessTool) -> PluginInfo {
    let description = tool.description();
    PluginInfo {
        name: description.name,
        version: description.version,
        protocol: PROTOCOL,
        licence: tool.licence().to_string(),
        capabilities: vec!["tool".into()],
        runtime: None,
        program: String::new(),
        extra: serde_json::Map::new(),
    }
}

/// Answers requests on stdin until `quit` or the end of input.
pub fn serve_stdio(tool: &dyn HeadlessTool) -> std::io::Result<()> {
    serve(tool, std::io::stdin().lock(), std::io::stdout().lock())
}

/// Answers the requests read from `input`, one JSON line each, with events on `output`.
pub fn serve(tool: &dyn HeadlessTool, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Err(e) => Err(format!("not JSON: {e}")),
            Ok(value) => match value.get("request").and_then(Value::as_str) {
                Some("quit") => return Ok(()),
                Some("info") => to_value(&plugin_info(tool)),
                Some("describe" | "validate" | "run") => match serde_json::from_value::<ToolRequest>(value) {
                    Ok(request) => answer(tool, request, &mut output),
                    Err(e) => Err(format!("malformed request: {e}")),
                },
                Some(other) => Err(format!("unknown request {other}")),
                None => Err("a request names itself with \"request\"".into()),
            },
        };
        let event = match answer {
            Ok(result) => Event::Done { result },
            Err(message) => Event::Error { message },
        };
        writeln!(output, "{}", event.line())?;
        output.flush()?;
    }
    Ok(())
}

fn answer(tool: &dyn HeadlessTool, request: ToolRequest, output: &mut impl Write) -> Result<Value, String> {
    let description = tool.description();
    match request {
        ToolRequest::Describe => to_value(&description),
        ToolRequest::Validate { operation, recipe } => {
            let problems = match description.operation(&operation) {
                Some(op) => tool.validate(op, &recipe),
                None => vec![format!("{} has no operation {operation}", description.name)],
            };
            to_value(&Validation { problems })
        }
        ToolRequest::Run(request) => {
            let op = description
                .operation(&request.operation)
                .ok_or_else(|| format!("{} has no operation {}", description.name, request.operation))?;
            let problems = tool.validate(op, &request.recipe);
            if !problems.is_empty() {
                return Err(format!("the recipe cannot run: {}", problems.join("; ")));
            }
            if !request.output_dir.is_dir() {
                return Err(format!("{}: the output folder does not exist", request.output_dir.display()));
            }
            let report = {
                let mut progress = |stage: &str, fraction: Option<f32>| {
                    let line = Event::Progress { stage: stage.to_string(), fraction }.line();
                    let _ = writeln!(output, "{line}").and_then(|_| output.flush());
                };
                tool.run(&request, &mut progress)?
            };
            report.check(op, &request.output_dir).map_err(|e| format!("the tool's report is wrong: {e}"))?;
            to_value(&report)
        }
    }
}

fn to_value(value: &impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputPort, OutputPort, RunOutput, TOOL_PROTOCOL};
    use kitchen_store::{ArtifactKind, KindPattern};

    /// Copies its input's size into a one-line table.
    struct Echo;

    impl HeadlessTool for Echo {
        fn description(&self) -> ToolDescription {
            ToolDescription {
                name: "echo".into(),
                version: "0.1.0".into(),
                tool_protocol: TOOL_PROTOCOL,
                operations: vec![Operation {
                    name: "measure".into(),
                    summary: "the input's size".into(),
                    inputs: vec![InputPort {
                        name: "data".into(),
                        accepts: vec![KindPattern::parse("raw/*").unwrap()],
                        many: false,
                        optional: false,
                    }],
                    outputs: vec![OutputPort { name: "table".into(), kind: ArtifactKind::parse("table/csv").unwrap() }],
                    recipe_schema: Value::Null,
                    resources: Default::default(),
                }],
            }
        }

        fn validate(&self, _: &Operation, recipe: &Value) -> Vec<String> {
            if recipe.get("unit").is_some() { vec![] } else { vec!["the recipe needs a unit".into()] }
        }

        fn run(&self, request: &RunRequest, progress: &mut dyn FnMut(&str, Option<f32>)) -> Result<RunReport, String> {
            progress("measuring", Some(0.5));
            let path = request.output_dir.join("size.csv");
            std::fs::write(&path, "bytes\n3\n").map_err(|e| e.to_string())?;
            Ok(RunReport {
                outputs: vec![RunOutput {
                    name: "table".into(),
                    kind: ArtifactKind::parse("table/csv").unwrap(),
                    path,
                }],
                ..Default::default()
            })
        }
    }

    fn events(input: &str) -> Vec<Event> {
        let mut out = Vec::new();
        serve(&Echo, input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn describes_validates_and_runs() {
        let dir = std::env::temp_dir().join(format!("kitchen-tool-serve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |recipe: &str| {
            format!(
                r#"{{"request":"run","operation":"measure","recipe":{recipe},"inputs":[],"output_dir":{}}}"#,
                serde_json::to_string(&dir).unwrap()
            )
        };
        let input = [
            r#"{"request":"describe"}"#.to_string(),
            r#"{"request":"validate","operation":"measure","recipe":{}}"#.to_string(),
            run("{}"),
            run(r#"{"unit":"bytes"}"#),
            r#"{"request":"quit"}"#.to_string(),
            r#"{"request":"describe"}"#.to_string(),
        ]
        .join("\n");
        let events = events(&input);
        let Event::Done { result } = &events[0] else { panic!("{events:?}") };
        assert_eq!(serde_json::from_value::<ToolDescription>(result.clone()).unwrap(), Echo.description());
        let Event::Done { result } = &events[1] else { panic!("{events:?}") };
        assert!(!serde_json::from_value::<Validation>(result.clone()).unwrap().ok());
        assert!(
            matches!(&events[2], Event::Error { message } if message.contains("needs a unit")),
            "a recipe that does not validate is not run"
        );
        assert!(matches!(&events[3], Event::Progress { .. }));
        assert!(matches!(&events[4], Event::Done { .. }));
        assert_eq!(events.len(), 5, "nothing after quit");
        assert!(dir.join("size.csv").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
