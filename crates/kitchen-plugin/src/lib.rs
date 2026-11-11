//! `kitchen-plugin`: plugins for the neuro-kitchen apps, as separate programs (lumen's model
//! plugins, dsp-kitchen's sorters). The app never links a plugin, so a plugin never adds weight,
//! licences or a runtime to the app, and can be installed, updated or removed on its own.
//!
//! - [`protocol`]: requests and events, JSON lines over the program's stdin and stdout.
//! - [`PluginInfo`]: what a plugin offers (its manifest): name, version, the protocol it speaks,
//!   its licence, its capabilities, whether its runtime is ready, and the app's own fields.
//!   [`PluginInfo::compatible`] is the handshake.
//! - [`PluginKind`]: an app's plugins (programs named `<prefix><name>`) and where they are
//!   found ([`PluginKind::discover`]).
//! - [`PluginProcess`]: a plugin in server mode, kept alive between requests.
//! - [`Manager`]: install (from a file or a verified download), update, remove, disk use.
//! - `ui` (feature, on by default): [`ui::PluginsPage`], the manager's Settings page.

#![warn(missing_docs)]

pub mod client;
pub mod discover;
pub mod manager;
pub mod protocol;
#[cfg(feature = "ui")]
pub mod ui;

pub use client::PluginProcess;
pub use discover::PluginKind;
pub use manager::{Installed, Manager};
pub use protocol::{CoreRequest, Event, PROTOCOL};

use serde::{Deserialize, Serialize};

/// Whether a plugin's runtime can run here, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    /// Ready to run.
    pub ready: bool,
    /// What runs it (`ONNX Runtime 1.22 (CUDA, CPU)`), or what is missing and how to get it.
    pub detail: String,
}

fn protocol_one() -> u32 {
    1
}

/// A plugin program and what it offers (its answer to `info`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// The protocol version it speaks (1 when it does not say).
    #[serde(default = "protocol_one")]
    pub protocol: u32,
    /// Its licence (it may differ from the app's).
    #[serde(default)]
    pub licence: String,
    /// What it can do (`segment`, `instances`, `sort`): the app offers what these name.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Whether its runtime is ready.
    #[serde(default)]
    pub runtime: Option<RuntimeStatus>,
    /// Where it was found (set by discovery, not by the plugin).
    #[serde(default)]
    pub program: String,
    /// The app's own fields (lumen's `models`).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl PluginInfo {
    /// The handshake: whether this app can talk to it (the same protocol version).
    pub fn compatible(&self) -> Result<(), String> {
        match self.protocol {
            PROTOCOL => Ok(()),
            p if p > PROTOCOL => Err(format!("{} speaks protocol {p}; this app speaks {PROTOCOL}: update the app", self.name)),
            p => Err(format!("{} speaks protocol {p}; this app speaks {PROTOCOL}: update the plugin", self.name)),
        }
    }

    /// Whether it can do `capability`.
    pub fn can(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_keep_the_apps_fields_and_check_the_protocol() {
        let info: PluginInfo = serde_json::from_str(r#"{"name":"lumen-ml-sam","version":"0.1.0","licence":"Apache-2.0","capabilities":["segment"],"models":[{"id":"sam2-tiny"}]}"#).unwrap();
        assert_eq!(info.protocol, 1, "a plugin that does not say speaks 1");
        assert!(info.compatible().is_ok() && info.can("segment") && !info.can("sort"));
        assert_eq!(info.extra["models"][0]["id"], "sam2-tiny");
        let newer = PluginInfo { protocol: 2, ..info.clone() };
        assert!(newer.compatible().unwrap_err().contains("update the app"));
        let back: PluginInfo = serde_json::from_str(&serde_json::to_string(&info).unwrap()).unwrap();
        assert_eq!(back, info);
    }
}
