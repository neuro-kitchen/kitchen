//! Running a script from the editor: kitchen-scripts runs it (a process reading one JSON object on
//! its stdin, printing its result as JSON on its last line); the editor maps its languages onto the
//! runtime's and shows the result.

use std::sync::Arc;

use kitchen_contract::Language;
use kitchen_ui::widgets::CodeLanguage;
use serde_json::Value;

use crate::script::Script;

pub use kitchen_scripts::run::RunOutput;

/// Runs a script with its input (blocking; called off the UI thread).
pub type Runner = Arc<dyn Fn(&Script, &Value) -> Result<RunOutput, String> + Send + Sync>;

/// The runtime's language for a script, if it runs.
fn language(script: &Script) -> Result<Language, String> {
    match script.language {
        CodeLanguage::Python => Ok(Language::Python),
        CodeLanguage::Rust => Ok(Language::Rust),
        other => Err(format!("{} scripts do not run", other.name())),
    }
}

/// The runner used by default: kitchen-scripts' (Python with `python3`, Rust with `rust-script`),
/// the input on the script's stdin.
pub fn default_runner() -> Runner {
    Arc::new(|script: &Script, input: &Value| kitchen_scripts::run::run_file(&script.path, language(script)?, input))
}

/// Errors on the script's own lines in `stderr` (kitchen-scripts' [`kitchen_scripts::run::script_errors`]).
pub fn script_errors(script: &Script, stderr: &str) -> Vec<(usize, String)> {
    language(script).map(|l| kitchen_scripts::run::script_errors(&script.path, l, stderr)).unwrap_or_default()
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
