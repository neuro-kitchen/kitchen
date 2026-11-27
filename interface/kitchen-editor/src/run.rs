//! Running a script: a process reading one JSON object on its stdin (`{"args": …, "context": …}`)
//! and printing its result as JSON on its last line, its errors read back onto its own lines.

use std::process::{Command, Stdio};
use std::sync::Arc;

use kitchen_ui::widgets::CodeLanguage;
use serde_json::Value;

use crate::script::Script;

/// What a run of a script produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunOutput {
    /// Whether it exited well.
    pub ok: bool,
    /// What it printed (stdout, then stderr).
    pub stdout: String,
    /// Its error output.
    pub stderr: String,
    /// Its result: the last line of its output, when that is JSON.
    pub result: Option<Value>,
    /// Errors on lines of the script (Python tracebacks, Rust compiler messages): line, message.
    pub errors: Vec<(usize, String)>,
}

/// Runs a script with its input (blocking; called off the UI thread).
pub type Runner = Arc<dyn Fn(&Script, &Value) -> Result<RunOutput, String> + Send + Sync>;

/// The runner used by default: Python with `python3`; Rust with `rust-script` (install it with
/// `cargo install rust-script`). The input goes to the script's stdin.
pub fn default_runner() -> Runner {
    Arc::new(|script: &Script, input: &Value| {
        let mut cmd = match script.language {
            CodeLanguage::Python => {
                let mut c = Command::new(if cfg!(windows) { "python" } else { "python3" });
                c.arg("-u").arg(&script.path);
                c
            }
            CodeLanguage::Rust => {
                let mut c = Command::new("rust-script");
                c.arg(&script.path);
                c
            }
            other => return Err(format!("{} scripts do not run", other.name())),
        };
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| match script.language {
            CodeLanguage::Rust => format!("Rust scripts run with rust-script ({e}): cargo install rust-script"),
            _ => format!("python3: {e}"),
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write as _;
            let _ = writeln!(stdin, "{input}");
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned());
        let result = stdout.lines().rev().find(|l| !l.trim().is_empty()).and_then(|l| serde_json::from_str::<Value>(l.trim()).ok()).filter(|v| v.is_object() || v.is_array());
        let errors = script_errors(script, &stderr);
        Ok(RunOutput { ok: out.status.success(), stdout, stderr, result, errors })
    })
}

/// Errors on the script's own lines in `stderr`: a Python traceback's last frame in the script
/// (`File "…/count.py", line 7`) with its last line, or a Rust message (`--> count.rs:7:5`).
pub fn script_errors(script: &Script, stderr: &str) -> Vec<(usize, String)> {
    let file = script.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    let mut out = Vec::new();
    match script.language {
        CodeLanguage::Python => {
            let line = stderr
                .lines()
                .filter(|l| l.contains("File \"") && l.contains(&file))
                .filter_map(|l| l.split(", line ").nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse::<usize>().ok())
                .last();
            if let Some(n) = line {
                out.push((n, last));
            }
        }
        CodeLanguage::Rust => {
            let mut message = String::new();
            for l in stderr.lines() {
                if l.starts_with("error") {
                    message = l.trim().to_string();
                } else if let Some(at) = l.trim().strip_prefix("--> ")
                    && at.contains(&file)
                    && let Some(n) = at.split(':').nth(1).and_then(|n| n.parse::<usize>().ok())
                {
                    out.push((n, message.clone()));
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::ScriptMeta;

    #[test]
    fn errors_point_at_the_scripts_lines() {
        let py = Script { path: "/x/count.py".into(), language: CodeLanguage::Python, meta: ScriptMeta::default() };
        let tb = "Traceback (most recent call last):\n  File \"/x/count.py\", line 7, in <module>\n    main()\n  File \"/x/count.py\", line 4, in main\n    1/0\nZeroDivisionError: division by zero\n";
        assert_eq!(script_errors(&py, tb), vec![(4, "ZeroDivisionError: division by zero".to_string())]);
        let rs = Script { path: "/x/count.rs".into(), language: CodeLanguage::Rust, meta: ScriptMeta::default() };
        let msg = "error[E0308]: mismatched types\n --> /x/count.rs:6:13\n";
        assert_eq!(script_errors(&rs, msg), vec![(6, "error[E0308]: mismatched types".to_string())]);
    }
}
