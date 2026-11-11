//! The plugin manager: what is installed and how much room it takes; install a plugin from a
//! file or a verified download ([`kitchen_hub`]), update it, remove it (and what it downloaded).
//! A plugin is installed only after it answers the handshake (its manifest, the same protocol).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kitchen_hub::{Artifact, Hub, dir_size};

use crate::discover::{PluginKind, info};
use crate::PluginInfo;

/// A plugin found, and what the manager knows of it.
#[derive(Debug, Clone)]
pub struct Installed {
    /// Its program's file name (`lumen-ml-sam`).
    pub name: String,
    /// Its program.
    pub program: PathBuf,
    /// Its manifest, or why it could not be read.
    pub info: Result<PluginInfo, String>,
    /// Bytes it takes: its program and what it downloaded (its family in the hub).
    pub bytes: u64,
    /// Whether the manager installed it (in the user's plugin folder), so it can remove it.
    pub removable: bool,
}

/// An app's plugin manager.
#[derive(Debug, Clone)]
pub struct Manager {
    /// The app's plugins.
    pub kind: PluginKind,
    /// Where plugins keep what they download (one family per plugin, by its short name).
    pub hub: Option<Hub>,
}

impl Manager {
    /// A manager of `kind`'s plugins, whose downloads are in `hub`.
    pub fn new(kind: PluginKind, hub: Option<Hub>) -> Self {
        Self { kind, hub }
    }

    /// Every plugin found (each asked for its manifest; blocking: call it off the UI thread).
    pub fn list(&self) -> Vec<Installed> {
        self.kind
            .discover()
            .into_iter()
            .map(|(program, info)| {
                let name = program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let data = self.hub.as_ref().map_or(0, |h| h.disk_use(Some(self.kind.short_name(&name))));
                let bytes = std::fs::metadata(&program).map(|m| m.len()).unwrap_or(0) + data;
                let removable = program.parent() == Some(self.kind.user_dir.as_path());
                Installed { name, program, info, bytes, removable }
            })
            .collect()
    }

    /// Installs (or updates) the plugin program at `source` into the user's plugin folder, after
    /// it answers the handshake; returns its manifest.
    pub fn install_from(&self, source: &Path) -> Result<PluginInfo, String> {
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("no file name")?;
        if !self.kind.is_plugin(source) {
            return Err(format!("{name}: not a plugin of this app (its name starts with {})", self.kind.prefix));
        }
        info(&name, source)?;
        std::fs::create_dir_all(&self.kind.user_dir).map_err(|e| format!("{}: {e}", self.kind.user_dir.display()))?;
        let dest = self.kind.user_dir.join(&name);
        // Copied beside, then renamed over the old one: a running copy keeps working
        let part = dest.with_extension("part");
        std::fs::copy(source, &part).map_err(|e| format!("{}: {e}", part.display()))?;
        make_executable(&part)?;
        std::fs::rename(&part, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        info(&name, &dest)
    }

    /// Downloads a plugin program published as `artifact` (verified by its hash), then installs it.
    pub fn install_artifact(&self, artifact: &Artifact, progress: &mut dyn FnMut(u64, u64), cancel: Option<&AtomicBool>) -> Result<PluginInfo, String> {
        let hub = self.hub.as_ref().ok_or("no download cache set")?;
        let file = hub.pull(artifact, progress, cancel)?;
        // The downloaded file keeps the program's name
        let named = file.with_file_name(&artifact.file_name);
        make_executable(&named)?;
        self.install_from(&named)
    }

    /// Removes the installed plugin `name` (and, with `data`, what it downloaded).
    pub fn remove(&self, name: &str, data: bool) -> Result<(), String> {
        let program = self.kind.user_dir.join(name);
        if !program.is_file() {
            return Err(format!("{name} is not in {}: it was not installed here", self.kind.user_dir.display()));
        }
        std::fs::remove_file(&program).map_err(|e| format!("{}: {e}", program.display()))?;
        if data && let Some(hub) = &self.hub {
            let dir = hub.family_dir(self.kind.short_name(name));
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
        }
        Ok(())
    }

    /// Bytes every plugin takes with what it downloaded.
    pub fn disk_use(&self) -> u64 {
        dir_size(&self.kind.user_dir) + self.hub.as_ref().map_or(0, |h| h.disk_use(None))
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut p = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.permissions();
    p.set_mode(p.mode() | 0o755);
    std::fs::set_permissions(path, p).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), String> {
    Ok(())
}
