//! Tool run keys: a run is known by what decides its result. The same tool and version (and image),
//! operation, recipe, inputs and parameters give the same key, so the orchestrator reuses a
//! finished run's artifacts instead of computing them again, and two runs that should agree can
//! be checked to have had the same key.
//!
//! Hashed over canonical JSON (object keys sorted, no spaces), so the order a recipe's fields were
//! written in never changes a key.

use crate::ArtifactId;
use crate::provenance::{ArtifactRef, ToolRef};

pub use kitchen_contract::{RunKey, canonical_json};

/// What every tool run key's hash starts with: the key format's version.
const DOMAIN: &str = "kitchen-run-v1";

/// The key of running `operation` of `tool` with `recipe` on `inputs` (by port name: their order
/// does not matter) with `params`: the contract's [`RunKey`] over a tool run's fields (unchanged
/// from before the contract, so earlier keys still match).
pub fn tool_run_key(tool: &ToolRef, operation: &str, recipe: &serde_json::Value, inputs: &[ArtifactRef], params: &serde_json::Value) -> RunKey {
    let mut named: Vec<(&str, &str)> = inputs.iter().map(|i| (i.name.as_str(), i.id.as_str())).collect();
    named.sort();
    RunKey::from_fields(&serde_json::json!({
        "key": DOMAIN,
        "tool": tool.name,
        "version": tool.version,
        "image": tool.image,
        "operation": operation,
        "recipe": recipe_id(recipe).as_str(),
        "inputs": named,
        "params": params,
    }))
}

/// A recipe's id: the SHA-256 of its canonical JSON.
pub fn recipe_id(recipe: &serde_json::Value) -> ArtifactId {
    ArtifactId::of_json(recipe)
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
        let key = tool_run_key(&tool, "sort", &recipe, &[a.clone(), b.clone()], &serde_json::Value::Null);
        assert_eq!(key, tool_run_key(&tool, "sort", &reordered, &[b.clone(), a.clone()], &serde_json::Value::Null));
        let newer = ToolRef { version: "0.0.14".into(), ..tool.clone() };
        assert_ne!(key, tool_run_key(&newer, "sort", &recipe, &[a.clone(), b.clone()], &serde_json::Value::Null));
        assert_ne!(key, tool_run_key(&tool, "sort", &recipe, &[a, input("probe", 'c')], &serde_json::Value::Null));
        assert_eq!(canonical_json(&reordered), r#"{"sorter":"ks4","threshold":6}"#);
    }
}
