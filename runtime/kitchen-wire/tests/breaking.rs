//! The wire format only grows: every field of the committed snapshot
//! (`contract/proto/kitchen/runtime/v1/runtime.lock.json`) is still in `runtime.proto` with the
//! same number, type and label. New fields and messages are fine; anything else is a new package
//! (`kitchen.runtime.v2`). After adding fields, refresh the snapshot with
//! `UPDATE_WIRE_LOCK=1 cargo test -p kitchen-wire --test breaking` and commit it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use prost_reflect::{Cardinality, DescriptorPool, Kind};

/// message → field → "number type label"
fn fields() -> BTreeMap<String, BTreeMap<String, String>> {
    let pool = DescriptorPool::decode(kitchen_wire::DESCRIPTOR_SET).unwrap();
    let mut out = BTreeMap::new();
    for m in pool.all_messages().filter(|m| m.package_name() == "kitchen.runtime.v1") {
        let mut f = BTreeMap::new();
        for field in m.fields() {
            let kind = match field.kind() {
                Kind::Message(t) => t.full_name().to_string(),
                Kind::Enum(t) => t.full_name().to_string(),
                other => format!("{other:?}"),
            };
            let label = match field.cardinality() {
                Cardinality::Repeated => "repeated",
                Cardinality::Optional => "optional",
                Cardinality::Required => "required",
            };
            let oneof = field.containing_oneof().map(|o| format!(" oneof:{}", o.name())).unwrap_or_default();
            f.insert(field.name().to_string(), format!("{} {kind} {label}{oneof}", field.number()));
        }
        out.insert(m.full_name().to_string(), f);
    }
    out
}

#[test]
fn the_wire_format_only_grows() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contract/proto/kitchen/runtime/v1/runtime.lock.json");
    let now = fields();
    if std::env::var_os("UPDATE_WIRE_LOCK").is_some() || !path.exists() {
        std::fs::write(&path, serde_json::to_string_pretty(&now).unwrap() + "\n").unwrap();
    }
    let locked: BTreeMap<String, BTreeMap<String, String>> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut broken = Vec::new();
    for (message, fields) in &locked {
        let Some(current) = now.get(message) else {
            broken.push(format!("{message} was removed"));
            continue;
        };
        for (name, was) in fields {
            match current.get(name) {
                None => broken.push(format!("{message}.{name} ({was}) was removed")),
                Some(is) if is != was => broken.push(format!("{message}.{name} changed: {was} → {is}")),
                _ => {}
            }
        }
        let numbers: Vec<&str> = current.values().map(|v| v.split(' ').next().unwrap()).collect();
        let mut unique = numbers.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != numbers.len() {
            broken.push(format!("{message} uses a field number twice"));
        }
    }
    assert!(broken.is_empty(), "breaking changes to kitchen.runtime.v1 (make kitchen.runtime.v2 instead): {broken:#?}");
}
