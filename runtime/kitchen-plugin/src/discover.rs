//! Finding an app's plugin programs (from lumen's discovery): every executable named
//! `<prefix><name>` in, in order, `$<ENV>` (folders separated as in `PATH`), the user's plugin
//! folder (`$XDG_DATA_HOME/<app>/plugins` or `~/.local/share/<app>/plugins`), the folder of the
//! running program (an installed app with its plugins beside it, or `target/<profile>` in
//! development) and `PATH`. The first program of each name wins; each is asked for its manifest
//! (`<program> info`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::PluginInfo;

/// An app's kind of plugin: how its programs are named and where they are looked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginKind {
    /// The app (`lumen`): names the user's plugin folder.
    pub app: String,
    /// Program names start with this (`lumen-ml-`).
    pub prefix: String,
    /// The variable listing more folders (`LUMEN_PLUGINS`).
    pub env: String,
    /// Where the manager installs plugins (the user's plugin folder by default).
    pub user_dir: PathBuf,
    /// Whether the running program's folder and `PATH` are searched too.
    pub system: bool,
}

impl PluginKind {
    /// The plugins of `app`, named `<prefix><name>`, with more folders in `$<env>`.
    pub fn new(app: &str, prefix: &str, env: &str) -> Self {
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
            .unwrap_or_else(std::env::temp_dir);
        Self { app: app.into(), prefix: prefix.into(), env: env.into(), user_dir: data.join(app).join("plugins"), system: true }
    }

    /// Installs into, and looks first in, `dir` instead of the user's plugin folder.
    pub fn with_user_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.user_dir = dir.into();
        self
    }

    /// Looks only in `$<env>` and the user's plugin folder (not beside the program or on `PATH`).
    pub fn without_system(mut self) -> Self {
        self.system = false;
        self
    }

    /// The folders searched, in order.
    pub fn dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(v) = std::env::var_os(&self.env) {
            dirs.extend(std::env::split_paths(&v));
        }
        dirs.push(self.user_dir.clone());
        if self.system {
            if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
                dirs.push(exe_dir);
            }
            if let Some(p) = std::env::var_os("PATH") {
                dirs.extend(std::env::split_paths(&p));
            }
        }
        dirs
    }

    /// Whether `path` is one of these plugins by its name.
    pub fn is_plugin(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false };
        let stem = name.strip_suffix(".exe").unwrap_or(name);
        // Build by-products (`lumen-ml-sam.d`) are not programs
        stem.starts_with(&self.prefix) && stem.len() > self.prefix.len() && !stem.contains('.') && path.is_file()
    }

    /// The plugin's short name (`sam` of `lumen-ml-sam`).
    pub fn short_name<'a>(&self, program: &'a str) -> &'a str {
        let stem = program.strip_suffix(".exe").unwrap_or(program);
        stem.strip_prefix(self.prefix.as_str()).unwrap_or(stem)
    }

    /// The plugin programs found, by name.
    pub fn programs(&self) -> BTreeMap<String, PathBuf> {
        let mut out = BTreeMap::new();
        for dir in self.dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if self.is_plugin(&p) {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    out.entry(name).or_insert(p);
                }
            }
        }
        out
    }

    /// Every plugin found and its manifest; a program that does not answer, or speaks another
    /// protocol, is reported as an error (its name and why), not left out silently.
    pub fn discover(&self) -> Vec<(PathBuf, Result<PluginInfo, String>)> {
        self.programs().into_iter().map(|(name, path)| {
            let info = info(&name, &path);
            (path, info)
        }).collect()
    }
}

/// Asks the program at `path` for its manifest and checks the protocol.
pub fn info(name: &str, path: &Path) -> Result<PluginInfo, String> {
    let out = Command::new(path).arg("info").output().map_err(|e| format!("{name}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{name}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let mut info: PluginInfo = serde_json::from_slice(&out.stdout).map_err(|e| format!("{name}: its manifest does not read: {e}"))?;
    info.program = path.to_string_lossy().into_owned();
    info.compatible()?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_programs_with_the_prefix_are_plugins() {
        let dir = std::env::temp_dir().join(format!("kitchen-plugin-discover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["demo-ml-sam", "demo-ml-sam.d", "demo-ml-", "demo-app", "notes.txt"] {
            std::fs::write(dir.join(n), b"").unwrap();
        }
        let kind = PluginKind::new("demo", "demo-ml-", "KITCHEN_PLUGIN_TEST_UNSET").with_user_dir(&dir).without_system();
        let found: Vec<String> = kind.programs().into_keys().collect();
        assert_eq!(found, vec!["demo-ml-sam".to_string()]);
        assert_eq!(kind.short_name("demo-ml-sam"), "sam");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
