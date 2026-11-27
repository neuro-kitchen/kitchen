//! Run keys: a run is known by what decides its result. The same tool and version (and image),
//! operation, recipe, inputs and parameters give the same key, so the orchestrator reuses a
//! finished run's artifacts instead of computing them again, and two runs that should agree can
//! be checked to have had the same key.
//!
//! Hashed over canonical JSON (object keys sorted, no spaces), so the order a recipe's fields were
//! written in never changes a key.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::provenance::{ArtifactRef, ToolRef};
use crate::{ArtifactError, ArtifactId, ArtifactResult};

/// What every key's hash starts with: the key format's version.
const DOMAIN: &str = "kitchen-run-v1";

/// A run's key: `sha256:<hex>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RunKey(String);

impl RunKey {
    /// The key of running `operation` of `tool` with `recipe` on `inputs` (by port name: their
    /// order does not matter) with `params`.
    pub fn new(
        tool: &ToolRef,
        operation: &str,
        recipe: &serde_json::Value,
        inputs: &[ArtifactRef],
        params: &serde_json::Value,
    ) -> Self {
        let mut named: Vec<(&str, &str)> = inputs.iter().map(|i| (i.name.as_str(), i.id.as_str())).collect();
        named.sort();
        let fields = serde_json::json!({
            "key": DOMAIN,
            "tool": tool.name,
            "version": tool.version,
            "image": tool.image,
            "operation": operation,
            "recipe": recipe_id(recipe).as_str(),
            "inputs": named,
            "params": params,
        });
        Self(hash_id(canonical_json(&fields).as_bytes()).into())
    }

    pub fn parse(text: &str) -> ArtifactResult<Self> {
        ArtifactId::parse(text)
            .map(|id| Self(id.into()))
            .map_err(|_| ArtifactError::Invalid(format!("run key {text:?}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RunKey {
    type Error = ArtifactError;
    fn try_from(text: String) -> ArtifactResult<Self> {
        Self::parse(&text)
    }
}

impl From<RunKey> for String {
    fn from(key: RunKey) -> String {
        key.0
    }
}

/// A recipe's id: the SHA-256 of its canonical JSON.
pub fn recipe_id(recipe: &serde_json::Value) -> ArtifactId {
    hash_id(canonical_json(recipe).as_bytes())
}

fn hash_id(bytes: &[u8]) -> ArtifactId {
    ArtifactId::from_digest(&Sha256::digest(bytes))
}

/// `value` as JSON with every object's keys sorted and no whitespace.
pub fn canonical_json(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let fields: Vec<String> =
                keys.into_iter().map(|k| format!("{}:{}", Value::String(k.clone()), canonical_json(&map[k]))).collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArtifactKind;

    fn input(name: &str, hex: char) -> ArtifactRef {
        ArtifactRef {
            id: ArtifactId::parse(&format!("sha256:{}", hex.to_string().repeat(64))).unwrap(),
            kind: ArtifactKind::parse("zarr/nwb").unwrap(),
            name: name.into(),
        }
    }

    #[test]
    fn the_same_run_has_the_same_key() {
        let tool = ToolRef { name: "dspk".into(), version: "0.0.13".into(), image: None };
        let (a, b) = (input("recording", 'a'), input("probe", 'b'));
        let recipe = serde_json::json!({"sorter": "ks4", "threshold": 6});
        let reordered: serde_json::Value = serde_json::from_str(r#"{"threshold": 6, "sorter": "ks4"}"#).unwrap();
        let key = RunKey::new(&tool, "sort", &recipe, &[a.clone(), b.clone()], &serde_json::Value::Null);
        assert_eq!(key, RunKey::new(&tool, "sort", &reordered, &[b.clone(), a.clone()], &serde_json::Value::Null));
        let newer = ToolRef { version: "0.0.14".into(), ..tool.clone() };
        assert_ne!(key, RunKey::new(&newer, "sort", &recipe, &[a.clone(), b.clone()], &serde_json::Value::Null));
        assert_ne!(key, RunKey::new(&tool, "sort", &recipe, &[a, input("probe", 'c')], &serde_json::Value::Null));
        assert_eq!(canonical_json(&reordered), r#"{"sorter":"ks4","threshold":6}"#);
    }
}
