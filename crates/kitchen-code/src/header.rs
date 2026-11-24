//! What a script's header says it is: the `kitchen:` comment lines at its top.
//!
//! ```text
//! # kitchen: command = Count cells
//! # kitchen: hint = Counts the cells of the shown image
//! # kitchen: args = {"type": "object", "properties": {"min_area": {"type": "number"}}}
//! # kitchen: node = in image:image; out counts:table
//! ```
//!
//! (`//` in Rust.) With `command`, the script becomes a command: in the menus, the search and an
//! agent's tools, its arguments the schema given. With `node`, it becomes a pipeline step with
//! typed inputs and outputs, whose types are the app's.

use serde_json::Value;

/// A typed port of a script node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    /// Its name (`image`).
    pub name: String,
    /// Its type, one the app declares (`image`, `mask`, `labels`, `table`; `signal`, `events`).
    pub ty: String,
}

/// What a script's header says it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptMeta {
    /// As a command: its title.
    pub command: Option<String>,
    /// What it does.
    pub hint: Option<String>,
    /// The JSON Schema of its arguments (as a command).
    pub args: Option<Value>,
    /// As a pipeline node: its inputs and outputs.
    pub node: Option<(Vec<Port>, Vec<Port>)>,
    /// What in the header did not read (line, why).
    pub problems: Vec<(usize, String)>,
}

impl ScriptMeta {
    /// Reads the `kitchen:` lines at the top of `text` (`#` or `//` comments, up to the first
    /// line that is neither a comment nor blank).
    pub fn parse(text: &str) -> ScriptMeta {
        let mut m = ScriptMeta::default();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("#!") {
                continue;
            }
            let Some(comment) = t.strip_prefix('#').or_else(|| t.strip_prefix("//")) else { break };
            let Some(rest) = comment.trim().strip_prefix("kitchen:") else { continue };
            let Some((key, value)) = rest.split_once('=') else {
                m.problems.push((i + 1, "expected `kitchen: key = value`".into()));
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "command" => m.command = Some(value.to_string()),
                "hint" => m.hint = Some(value.to_string()),
                "args" => match serde_json::from_str::<Value>(value) {
                    Ok(v) if v.is_object() => m.args = Some(v),
                    Ok(_) => m.problems.push((i + 1, "args: a JSON Schema object".into())),
                    Err(e) => m.problems.push((i + 1, format!("args: {e}"))),
                },
                "node" => match parse_node(value) {
                    Ok(n) => m.node = Some(n),
                    Err(e) => m.problems.push((i + 1, format!("node: {e}"))),
                },
                other => m.problems.push((i + 1, format!("unknown key {other:?} (command, hint, args, node)"))),
            }
        }
        m
    }
}

/// `in image:image, mask:mask; out counts:table`.
fn parse_node(text: &str) -> Result<(Vec<Port>, Vec<Port>), String> {
    let ports = |list: &str| -> Result<Vec<Port>, String> {
        list.split(',').map(str::trim).filter(|p| !p.is_empty()).map(|p| match p.split_once(':') {
            Some((n, t)) if !n.trim().is_empty() && !t.trim().is_empty() => Ok(Port { name: n.trim().into(), ty: t.trim().into() }),
            _ => Err(format!("{p:?}: expected name:type")),
        }).collect()
    };
    let (mut inputs, mut outputs) = (Vec::new(), Vec::new());
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(l) = part.strip_prefix("in ") {
            inputs = ports(l)?;
        } else if let Some(l) = part.strip_prefix("out ") {
            outputs = ports(l)?;
        } else {
            return Err(format!("{part:?}: expected `in …` or `out …`"));
        }
    }
    if outputs.is_empty() {
        return Err("a node needs at least one output".into());
    }
    Ok((inputs, outputs))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn headers_say_what_a_script_is() {
        let m = ScriptMeta::parse(
            "#!/usr/bin/env python3\n# kitchen: command = Count cells\n# kitchen: hint = Counts them\n# kitchen: args = {\"type\": \"object\"}\n# kitchen: node = in image:image, mask:mask; out counts:table\nimport json\n# kitchen: command = ignored after code\n",
        );
        assert_eq!(m.command.as_deref(), Some("Count cells"));
        assert_eq!(m.hint.as_deref(), Some("Counts them"));
        assert_eq!(m.args, Some(json!({"type": "object"})));
        let (i, o) = m.node.unwrap();
        assert_eq!(i, vec![Port { name: "image".into(), ty: "image".into() }, Port { name: "mask".into(), ty: "mask".into() }]);
        assert_eq!(o, vec![Port { name: "counts".into(), ty: "table".into() }]);
        assert!(m.problems.is_empty());

        let bad = ScriptMeta::parse("// kitchen: args = [1]\n// kitchen: node = in a:image\n// kitchen: colour = red\n// kitchen: oops\nfn main() {}\n");
        let lines: Vec<usize> = bad.problems.iter().map(|p| p.0).collect();
        assert_eq!(lines, vec![1, 2, 3, 4]);
    }
}
