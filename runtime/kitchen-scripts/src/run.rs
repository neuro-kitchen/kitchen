//! Running a script file once, as a program: it reads one JSON object on its stdin
//! (`{"args": …, "context": …}`) and prints its result as JSON on its last line; its errors are read
//! back onto its own lines. The Code workspace runs scripts this way (moved here from
//! kitchen-editor in phase 2, step 3: running is the runtime's, editing the interface's).

use std::path::Path;
use std::process::{Command, Stdio};

use kitchen_contract::Language;
use serde_json::Value;

/// What a run of a script produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunOutput {
    /// Whether it exited well.
    pub ok: bool,
    /// What it printed.
    pub stdout: String,
    /// Its error output.
    pub stderr: String,
    /// Its result: the last line of its output, when that is a JSON object or array.
    pub result: Option<Value>,
    /// Errors on lines of the script (Python tracebacks, Rust compiler messages): line, message.
    pub errors: Vec<(usize, String)>,
}

/// Runs the script at `path` with `input` on its stdin (blocking; call it off the UI thread):
/// Python with `python3`, Rust with `rust-script` (`cargo install rust-script`).
pub fn run_file(path: &Path, language: Language, input: &Value) -> Result<RunOutput, String> {
    let mut cmd = match language {
        Language::Python => {
            let mut c = Command::new(if cfg!(windows) { "python" } else { "python3" });
            c.arg("-u").arg(path);
            c
        }
        Language::Rust => {
            let mut c = Command::new("rust-script");
            c.arg(path);
            c
        }
    };
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| match language {
        Language::Rust => format!("Rust scripts run with rust-script ({e}): cargo install rust-script"),
        Language::Python => format!("python3: {e}"),
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        let _ = writeln!(stdin, "{input}");
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned());
    let result = stdout.lines().rev().find(|l| !l.trim().is_empty()).and_then(|l| serde_json::from_str::<Value>(l.trim()).ok()).filter(|v| v.is_object() || v.is_array());
    let errors = script_errors(path, language, &stderr);
    Ok(RunOutput { ok: out.status.success(), stdout, stderr, result, errors })
}

/// Errors on the script's own lines in `stderr`: a Python traceback's last frame in the script
/// (`File "…/count.py", line 7`) with its last line, or a Rust message (`--> count.rs:7:5`).
pub fn script_errors(path: &Path, language: Language, stderr: &str) -> Vec<(usize, String)> {
    let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    let mut out = Vec::new();
    match language {
        Language::Python => {
            let line = stderr
                .lines()
                .filter(|l| l.contains("File \"") && l.contains(&file))
                .filter_map(|l| l.split(", line ").nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse::<usize>().ok())
                .last();
            if let Some(n) = line {
                out.push((n, last));
            }
        }
        Language::Rust => {
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
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_point_at_the_scripts_lines() {
        let tb = "Traceback (most recent call last):\n  File \"/x/count.py\", line 7, in <module>\n    main()\n  File \"/x/count.py\", line 4, in main\n    1/0\nZeroDivisionError: division by zero\n";
        assert_eq!(script_errors(Path::new("/x/count.py"), Language::Python, tb), vec![(4, "ZeroDivisionError: division by zero".to_string())]);
        let msg = "error[E0308]: mismatched types\n --> /x/count.rs:6:13\n";
        assert_eq!(script_errors(Path::new("/x/count.rs"), Language::Rust, msg), vec![(6, "error[E0308]: mismatched types".to_string())]);
    }
}
