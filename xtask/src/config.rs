//! kitchen.toml: the layers and their rules, as xtask reads them.

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Kitchen {
    pub project: Project,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub rules: Rules,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub license: String,
}

#[derive(Debug, Deserialize)]
pub struct Layer {
    pub name: String,
    pub crates: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Rules {
    /// Upward dependencies known and scheduled to go: each names why and when it goes.
    #[serde(default)]
    pub exceptions: Vec<Exception>,
}

#[derive(Debug, Deserialize)]
pub struct Exception {
    #[serde(rename = "crate")]
    pub name: String,
    pub depends_on: String,
    pub why: String,
}

impl Kitchen {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("kitchen.toml");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The layer index of `crate_name` (0 is the lowest), if it is in one.
    pub fn layer_of(&self, crate_name: &str) -> Option<usize> {
        self.layers.iter().position(|layer| layer.crates.iter().any(|c| c == crate_name))
    }

    pub fn exception(&self, crate_name: &str, dependency: &str) -> Option<&Exception> {
        self.rules.exceptions.iter().find(|e| e.name == crate_name && e.depends_on == dependency)
    }
}
