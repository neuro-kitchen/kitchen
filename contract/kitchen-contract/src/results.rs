//! Results and run records ([standard §3](../../../standard/spec/03-results.md)).
//!
//! - [`ContentId`]: `sha256:` and 64 hex digits, the same bytes giving the same id wherever they
//!   are (decision 0003). Hashing files and Zarr folders reads them, so it is the runtime's
//!   (kitchen-store' `identify`); the id itself is the contract's.
//! - [`canonical_json`]: object keys sorted, no whitespace, so the order fields were written in
//!   never changes a hash.
//! - [`ResultRecord`]: a stored result, with its type, format and the run that made it.
//! - [`RunRecord`]: every run leaves one: which op at which version, its parameters, its inputs
//!   and outputs by id, who ran it and through what, how it ended.
//! - [`RunKey`]: what decides a run's result, so an identical run is recognized and reused.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::params::ParamValue;
use crate::types::{Format, Type};

/// Prefix of every id: the algorithm, so another can be added without ambiguity.
const ALGORITHM: &str = "sha256:";
/// What every run key's hash starts with: the key format's version.
const RUN_KEY_DOMAIN: &str = "kitchen-run-v2";

/// A result's content identity: `sha256:<64 lowercase hex digits>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentId(String);

impl ContentId {
    /// `text` as an id, checked.
    pub fn parse(text: &str) -> Result<Self, String> {
        let hex = text.strip_prefix(ALGORITHM).unwrap_or("");
        if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            Ok(Self(text.to_string()))
        } else {
            Err(format!("content id {text:?}: expected sha256:<64 lowercase hex digits>"))
        }
    }

    /// The id of a SHA-256 digest (32 bytes).
    pub fn from_sha256(digest: &[u8]) -> Self {
        Self(format!("{ALGORITHM}{}", to_hex(digest)))
    }

    /// The id of `bytes`.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self::from_sha256(&Sha256::digest(bytes))
    }

    /// The id of a JSON value: the hash of its canonical form (a pipeline's, a recipe's).
    pub fn of_json(value: &serde_json::Value) -> Self {
        Self::of_bytes(canonical_json(value).as_bytes())
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first 12 hex digits, for names and logs.
    pub fn short(&self) -> &str {
        &self.0[ALGORITHM.len()..ALGORITHM.len() + 12]
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ContentId {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

impl From<ContentId> for String {
    fn from(id: ContentId) -> String {
        id.0
    }
}

impl JsonSchema for ContentId {
    fn schema_name() -> Cow<'static, str> {
        "ContentId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" })
    }
}

/// Lowercase hex of `bytes`.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `value` as JSON with every object's keys sorted and no whitespace.
pub fn canonical_json(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let fields: Vec<String> = keys.into_iter().map(|k| format!("{}:{}", Value::String(k.clone()), canonical_json(&map[k]))).collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

/// A run's key: `sha256:<hex>`, the hash of what decides its result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RunKey(String);

impl RunKey {
    /// The key of running op `op` at `version` (its [`OpSpec::version_key`](crate::OpSpec::version_key))
    /// with `params` (every one, defaults filled) on `inputs` (port → result id). Run options and
    /// metrics are not in it: they change how fast, not what.
    pub fn of(op: &str, version: &str, params: &BTreeMap<String, ParamValue>, inputs: &BTreeMap<String, ContentId>) -> Self {
        let fields = serde_json::json!({
            "key": RUN_KEY_DOMAIN,
            "op": op,
            "version": version,
            "params": params,
            "inputs": inputs,
        });
        Self::from_fields(&fields)
    }

    /// The key of any JSON `fields` (a tool run's: tool, image, operation, recipe), hashed in
    /// canonical form.
    pub fn from_fields(fields: &serde_json::Value) -> Self {
        Self(ContentId::of_json(fields).into())
    }

    /// `text` as a key, checked.
    pub fn parse(text: &str) -> Result<Self, String> {
        ContentId::parse(text).map(|id| Self(id.into())).map_err(|_| format!("run key {text:?}"))
    }

    /// The key as text.
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
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

impl From<RunKey> for String {
    fn from(key: RunKey) -> String {
        key.0
    }
}

impl JsonSchema for RunKey {
    fn schema_name() -> Cow<'static, str> {
        "RunKey".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" })
    }
}

/// A stored result ([standard §3.1](../../../standard/spec/03-results.md)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResultRecord {
    /// Its content id.
    pub id: ContentId,
    /// What it is.
    #[serde(rename = "type")]
    pub ty: Type,
    /// How it is encoded.
    pub format: Format,
    /// Its size in bytes.
    pub size: u64,
    /// The run that made it (`None` for a source the store was given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Made by a run that was cancelled: usable and exportable, its metadata says *run
    /// incomplete*, and never reused by run key.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub incomplete: bool,
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunStatus {
    /// Every output written.
    Succeeded,
    /// It stopped, and why.
    Failed {
        /// Why, said for people.
        why: String,
    },
    /// Cancelled: its outputs so far are kept, marked incomplete.
    Cancelled,
}

/// The op a run ran, at which version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpRef {
    /// Its id.
    pub id: String,
    /// Its [`OpSpec::version_key`](crate::OpSpec::version_key): `v<n>`, or a script's or recipe's hash.
    pub version: String,
}

/// Every run of an op (or a pipeline) leaves one ([standard §3.2](../../../standard/spec/03-results.md)).
/// Appended, never edited: a correction is a new run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunRecord {
    /// The contract version it was written under ([`CONTRACT_VERSION`](crate::CONTRACT_VERSION)).
    pub contract: String,
    /// Its id.
    pub run: String,
    /// The op that ran.
    pub op: OpRef,
    /// Every parameter, defaults filled.
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
    /// Port → result id (for a source the app reads, the source's id).
    #[serde(default)]
    pub inputs: BTreeMap<String, ContentId>,
    /// Port → result id.
    #[serde(default)]
    pub outputs: BTreeMap<String, ContentId>,
    /// Who: a person.
    pub by: String,
    /// Through what: an app, a generated interface, an agent.
    pub via: String,
    /// When it started (RFC 3339).
    pub started: String,
    /// When it ended (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
    /// How it ended.
    #[serde(flatten)]
    pub status: RunStatus,
    /// The runtime's and the app's versions, the device, and the run's options (piece size,
    /// read-ahead, threads, upload mode).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, serde_json::Value>,
    /// Time per op, bytes read, moved and written; never part of the run key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metrics: BTreeMap<String, serde_json::Value>,
    /// What decides its result ([`RunKey::of`]).
    pub run_key: RunKey,
}

impl RunRecord {
    /// Whether its outputs may stand in for an identical later run: it succeeded and its op acts
    /// on nothing outside the store.
    pub fn reusable(&self, effects: bool) -> bool {
        self.status == RunStatus::Succeeded && !effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(c: char) -> ContentId {
        ContentId::parse(&format!("sha256:{}", c.to_string().repeat(64))).unwrap()
    }

    #[test]
    fn ids_parse_and_hash() {
        assert!(ContentId::parse("sha256:abc").is_err());
        assert!(ContentId::parse(&format!("sha256:{}", "A".repeat(64))).is_err(), "lowercase only");
        assert_eq!(ContentId::of_bytes(b"abc").as_str(), "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(id('a').short(), "aaaaaaaaaaaa");
        let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":[2,{"d":3,"c":4}]}"#).unwrap();
        assert_eq!(canonical_json(&a), r#"{"a":[2,{"c":4,"d":3}],"b":1}"#);
        let reordered: serde_json::Value = serde_json::from_str(r#"{"a":[2,{"c":4,"d":3}],"b":1}"#).unwrap();
        assert_eq!(ContentId::of_json(&a), ContentId::of_json(&reordered));
    }

    #[test]
    fn the_same_run_has_the_same_key() {
        let params: BTreeMap<String, ParamValue> = [("cutoff_hz".to_string(), ParamValue::Number(300.0))].into();
        let inputs: BTreeMap<String, ContentId> = [("signal".to_string(), id('a'))].into();
        let key = RunKey::of("dspk:highpass", "v1", &params, &inputs);
        assert_eq!(key, RunKey::of("dspk:highpass", "v1", &params.clone(), &inputs.clone()));
        assert_ne!(key, RunKey::of("dspk:highpass", "v2", &params, &inputs), "a new version runs again");
        let other: BTreeMap<String, ContentId> = [("signal".to_string(), id('b'))].into();
        assert_ne!(key, RunKey::of("dspk:highpass", "v1", &params, &other));
        assert!(RunKey::parse(key.as_str()).is_ok());
    }

    #[test]
    fn run_records_read_and_say_what_may_be_reused() {
        let params: BTreeMap<String, ParamValue> = BTreeMap::new();
        let inputs: BTreeMap<String, ContentId> = [("signal".to_string(), id('a'))].into();
        let record = RunRecord {
            contract: crate::CONTRACT_VERSION.into(),
            run: "run-1".into(),
            op: OpRef { id: "dspk:kilosort".into(), version: "v4".into() },
            params: params.clone(),
            inputs: inputs.clone(),
            outputs: [("sorting".to_string(), id('b'))].into(),
            by: "ana".into(),
            via: "dsp-kitchen".into(),
            started: "2026-11-27T10:00:00Z".into(),
            ended: Some("2026-11-27T12:10:00Z".into()),
            status: RunStatus::Cancelled,
            environment: [("device".to_string(), serde_json::json!("cuda:0"))].into(),
            metrics: BTreeMap::new(),
            run_key: RunKey::of("dspk:kilosort", "v4", &params, &inputs),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains(r#""status":"cancelled""#), "{json}");
        let back: RunRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
        assert!(!record.reusable(false), "a cancelled run is never reused");
        let done = RunRecord { status: RunStatus::Succeeded, ..record };
        assert!(done.reusable(false));
        assert!(!done.reusable(true), "an op with effects is never reused");
        let failed: RunStatus = serde_json::from_str(r#"{"status":"failed","why":"out of memory"}"#).unwrap();
        assert_eq!(failed, RunStatus::Failed { why: "out of memory".into() });
    }
}
