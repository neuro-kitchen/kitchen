//! Requests and events between an app and a plugin: one JSON object per line (from lumen's model
//! plugins). The app writes a request (an object with a `"request"` tag) to the program's stdin;
//! the program answers on its stdout with [`Event`]s: `progress` any number of times, then
//! exactly one `done` or `error`. Large data (images, masks, signals) travels as files named in
//! the request, never through JSON.
//!
//! Every plugin answers the core requests ([`CoreRequest`]); an app adds its own (lumen's
//! `segment`, `instances`) as any serializable type with a `"request"` tag.

use serde::{Deserialize, Serialize};

/// The protocol version this crate speaks. A plugin states the one it speaks in its manifest
/// ([`crate::PluginInfo::protocol`]); the app refuses one that differs (the handshake).
pub const PROTOCOL: u32 = 1;

/// The requests every plugin answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum CoreRequest {
    /// The plugin's manifest (as `<program> info`).
    Info,
    /// Downloads and verifies something it needs (a model's weights, its runtime) by id.
    Install {
        /// What to install.
        id: String,
    },
    /// Removes what it downloaded for `id`.
    Remove {
        /// What to remove.
        id: String,
    },
    /// Ends a server.
    Quit,
}

/// What a plugin answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// How far it is.
    Progress {
        /// What it is doing ("encoding").
        stage: String,
        /// 0–1 when known.
        #[serde(default)]
        fraction: Option<f32>,
    },
    /// The request's answer (its JSON depends on the request).
    Done {
        /// The answer.
        result: serde_json::Value,
    },
    /// Why the request failed.
    Error {
        /// The reason.
        message: String,
    },
}

impl Event {
    /// The event as one line of the protocol.
    pub fn line(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| format!("{{\"event\":\"error\",\"message\":\"{e}\"}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_events_are_one_json_line() {
        let r = CoreRequest::Install { id: "sam2-tiny".into() };
        let line = serde_json::to_string(&r).unwrap();
        assert_eq!(line, r#"{"request":"install","id":"sam2-tiny"}"#);
        assert_eq!(serde_json::from_str::<CoreRequest>(&line).unwrap(), r);
        let e = Event::Progress { stage: "encoding".into(), fraction: Some(0.5) };
        assert!(!e.line().contains('\n'));
        assert_eq!(serde_json::from_str::<Event>(&e.line()).unwrap(), e);
        assert_eq!(serde_json::from_str::<Event>(r#"{"event":"done","result":{"n":3}}"#).unwrap(), Event::Done { result: serde_json::json!({"n": 3}) });
    }
}
