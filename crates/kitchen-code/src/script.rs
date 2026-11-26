//! A script file: its path, its language, its header.

use std::path::{Path, PathBuf};

use kitchen_ui::widgets::CodeLanguage;

use crate::header::ScriptMeta;

/// A script file.
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// Its file.
    pub path: PathBuf,
    /// Python or Rust.
    pub language: CodeLanguage,
    /// What its header says.
    pub meta: ScriptMeta,
}

impl Script {
    /// The script at `path` (`.py` or `.rs`), its header read.
    pub fn load(path: &Path) -> Option<Script> {
        let language = CodeLanguage::from_name(path.extension()?.to_str()?);
        if !matches!(language, CodeLanguage::Python | CodeLanguage::Rust) {
            return None;
        }
        let text = std::fs::read_to_string(path).ok()?;
        Some(Script { path: path.to_path_buf(), language, meta: ScriptMeta::parse(&text) })
    }

    /// Its name (the file's stem).
    pub fn name(&self) -> String {
        self.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Its command id, when it is a command (`script.count_cells`).
    pub fn command_id(&self) -> Option<String> {
        self.meta.command.as_ref().map(|_| format!("script.{}", self.name()))
    }
}
