//! The contract's JSON Schemas, generated from its types into `standard/schemas/`. A test fails
//! when a committed schema differs from the types, so a change to a document's format is always
//! visible in review. Regenerate: `UPDATE_SCHEMAS=1 cargo test -p kitchen-contract --test schemas`.

use std::path::PathBuf;

use kitchen_contract::{OpSpec, Pipeline, ResultRecord, RunRecord, TypeDescription};
use schemars::{JsonSchema, schema_for};

/// `value` with every object's keys sorted: the same text whatever order serde_json keeps keys
/// in (its `preserve_order` feature, turned on by another crate in a larger build, changes it).
fn sorted(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().map(|(k, v)| (k, sorted(v))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn check<T: JsonSchema>(file: &str) -> Option<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../standard/schemas").join(file);
    let schema = sorted(serde_json::to_value(schema_for!(T)).unwrap());
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&schema).unwrap() + "\n").unwrap();
    }
    let committed: Option<serde_json::Value> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    (committed.map(sorted).as_ref() != Some(&schema)).then(|| format!("{} is stale", path.display()))
}

#[test]
fn committed_schemas_are_current() {
    let stale: Vec<String> = [
        check::<TypeDescription>("type.json"),
        check::<OpSpec>("op.json"),
        check::<ResultRecord>("result.json"),
        check::<RunRecord>("run-record.json"),
        check::<Pipeline>("pipeline.json"),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(stale.is_empty(), "{stale:?}: run the tests with UPDATE_SCHEMAS=1 and commit them");
}
