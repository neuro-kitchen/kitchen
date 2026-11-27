//! Types: what flows between ops and what is stored ([standard §1](../../../standard/spec/01-types.md)).
//!
//! One type system for both: a port says `table`; the result it produced says `table` in
//! `parquet`. This merges kitchen-pipeline's port types and kitchen-artifacts' kinds
//! (`zarr/ome`, `table/parquet`), which named the same things at two moments
//! ([`from_kind`] reads the old kinds).
//!
//! - **Shared types** have bare names, defined by the standard ([`shared`]).
//! - **App types** are namespaced, `<namespace>:<name>` (`dspk:sorting`, `lucem:cells`).
//! - An output of type `T` feeds an input of type `T`, or of `any`. There is no inheritance.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// A type's name: `image`, `signal`, or an app's `dspk:sorting`. `any` accepts every type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Type(pub String);

impl Type {
    /// The type every input of type `any` accepts, and every type is.
    pub const ANY: &'static str = "any";

    /// The type named `name`, trimmed and lowercased (lenient: [`Type::parse`] checks the name).
    pub fn new(name: impl AsRef<str>) -> Self {
        Type(name.as_ref().trim().to_lowercase())
    }

    /// `name` as a type, checked: lowercase letters, digits and `-`, optionally after one
    /// `<namespace>:` of the same characters.
    pub fn parse(name: &str) -> Result<Self, String> {
        let ok = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let valid = match name.split_once(':') {
            Some((ns, n)) => ok(ns) && ok(n),
            None => ok(name),
        };
        if valid { Ok(Type(name.to_string())) } else { Err(format!("type {name:?}: lowercase letters, digits and '-', as `name` or `namespace:name`")) }
    }

    /// Its name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The namespace of an app type (`dspk` of `dspk:sorting`); `None` for a shared type.
    pub fn namespace(&self) -> Option<&str> {
        self.0.split_once(':').map(|(ns, _)| ns)
    }

    /// Whether an output of type `self` may feed an input of type `input`.
    pub fn feeds(&self, input: &Type) -> bool {
        self == input || input.0 == Self::ANY || self.0 == Self::ANY
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl JsonSchema for Type {
    fn schema_name() -> Cow<'static, str> {
        "Type".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "A type's name: a shared type (`image`) or an app's (`dspk:sorting`).",
            "pattern": "^([a-z0-9-]+:)?[a-z0-9-]+$"
        })
    }
}

/// How a stored result is encoded: `ome-zarr`, `nwb-zarr`, `parquet`, `csv`, `svg`, `png`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Format(pub String);

impl Format {
    /// `name` as a format, checked: lowercase letters, digits, `-`, `_` or `.`.
    pub fn parse(name: &str) -> Result<Self, String> {
        let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'));
        if ok { Ok(Format(name.to_string())) } else { Err(format!("format {name:?}: lowercase letters, digits, '-', '_' or '.'")) }
    }

    /// Its name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl JsonSchema for Format {
    fn schema_name() -> Cow<'static, str> {
        "Format".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "How a stored result is encoded (`ome-zarr`, `parquet`, `svg`).",
            "pattern": "^[a-z0-9._-]+$"
        })
    }
}

/// An axis of an array type, and the kind of unit along it. The values (a recording's 30 kHz, a
/// slide's 0.25 µm per pixel) come from each result's metadata, never from the type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Axis {
    /// Its name (`channels`, `samples`, `y`).
    pub name: String,
    /// The kind of unit along it (`time`, `length`), or `None` for a count (channels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// A type, described once: what an agent reads to write an op that takes it correctly
/// ([standard §1.5](../../../standard/spec/01-types.md)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TypeDescription {
    /// Its name.
    pub name: Type,
    /// One paragraph: what it is.
    pub summary: String,
    /// The formats it can be stored in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<Format>,
    /// For arrays, its axes in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<Axis>,
}

fn describe(name: &str, summary: &str, formats: &[&str], axes: &[(&str, Option<&str>)]) -> TypeDescription {
    TypeDescription {
        name: Type(name.into()),
        summary: summary.into(),
        formats: formats.iter().map(|f| Format((*f).into())).collect(),
        axes: axes.iter().map(|(n, u)| Axis { name: (*n).into(), unit: u.map(String::from) }).collect(),
    }
}

/// The shared types ([standard §1.3](../../../standard/spec/01-types.md)).
pub fn shared() -> Vec<TypeDescription> {
    vec![
        describe("signal", "Samples over time on channels, with a rate.", &["raw", "nwb-zarr", "zarr", "nwb"], &[("channels", None), ("samples", Some("time"))]),
        describe("events", "Points in time with labels: spikes, stimuli, TTLs.", &["parquet", "csv", "nwb"], &[]),
        describe("image", "Pixels: a plane, a stack, a pyramid; channels.", &["ome-zarr", "ome-tiff", "png"], &[("channels", None), ("z", Some("length")), ("y", Some("length")), ("x", Some("length"))]),
        describe("mask", "Yes or no per pixel or sample.", &["ome-zarr", "png"], &[]),
        describe("labels", "An integer per pixel or sample: objects.", &["ome-zarr"], &[]),
        describe("region", "Shapes in an image's or a recording's coordinates: polygons, intervals.", &["geojson", "geoparquet", "json"], &[]),
        describe("table", "Rows and named columns.", &["parquet", "csv"], &[]),
        describe("figure", "Something to look at: static, or a description an app draws (decision 0013).", &["svg", "png", "pdf", "plot-json"], &[]),
        describe("video", "Frames with exact times.", &["mp4", "image-sequence"], &[("frames", Some("time")), ("y", Some("length")), ("x", Some("length"))]),
        describe("file", "Anything else, opaque.", &[], &[]),
        describe("any", "Accepts every type (inputs only).", &[], &[]),
    ]
}

/// The type and format of an old artifact kind (`family/format`, kitchen-artifacts), as the
/// contract names them; `None` for a kind no type covers.
///
/// | Kind | Type | Format |
/// |---|---|---|
/// | `raw/*` | `signal` | `raw` |
/// | `zarr/nwb` | `signal` | `nwb-zarr` |
/// | `zarr/ome` | `image` | `ome-zarr` |
/// | `zarr/labels` | `labels` | `ome-zarr` |
/// | `zarr/sorting` | `dspk:sorting` | `zarr` |
/// | `table/parquet` | `table` | `parquet` |
/// | `table/csv` | `table` | `csv` |
/// | `table/geoparquet` | `region` | `geoparquet` |
/// | `file/*` | `file` | the format |
pub fn from_kind(kind: &str) -> Option<(Type, Format)> {
    let (family, format) = kind.split_once('/')?;
    let pair = |t: &str, f: &str| Some((Type(t.into()), Format(f.into())));
    match (family, format) {
        ("raw", _) => pair("signal", "raw"),
        ("zarr", "nwb") => pair("signal", "nwb-zarr"),
        ("zarr", "ome") => pair("image", "ome-zarr"),
        ("zarr", "labels") => pair("labels", "ome-zarr"),
        ("zarr", "sorting") => pair("dspk:sorting", "zarr"),
        ("table", "parquet") => pair("table", "parquet"),
        ("table", "csv") => pair("table", "csv"),
        ("table", "geoparquet") => pair("region", "geoparquet"),
        ("file", f) if !f.is_empty() => pair("file", f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_feed_their_own_and_any() {
        let (image, mask, any) = (Type::new("Image"), Type::new("mask"), Type::new("any"));
        assert_eq!(image.as_str(), "image");
        assert!(image.feeds(&image));
        assert!(!image.feeds(&mask));
        assert!(image.feeds(&any));
        assert!(any.feeds(&mask));
    }

    #[test]
    fn names_are_checked_and_namespaced() {
        assert!(Type::parse("signal").is_ok());
        assert_eq!(Type::parse("dspk:sorting").unwrap().namespace(), Some("dspk"));
        assert_eq!(Type::parse("table").unwrap().namespace(), None);
        for bad in ["Image", "a:b:c", ":x", "x:", "two words", ""] {
            assert!(Type::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Format::parse("ome-zarr").is_ok());
        assert!(Format::parse("OME").is_err());
    }

    #[test]
    fn every_shared_type_is_described_once() {
        let all = shared();
        let mut names: Vec<&str> = all.iter().map(|t| t.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len());
        for t in &all {
            Type::parse(t.name.as_str()).unwrap();
            assert!(!t.summary.is_empty());
        }
        let signal = all.iter().find(|t| t.name.as_str() == "signal").unwrap();
        assert_eq!(signal.axes[1].unit.as_deref(), Some("time"), "units are kinds; the rate is the data's");
    }

    #[test]
    fn old_kinds_read_as_type_and_format() {
        assert_eq!(from_kind("zarr/ome"), Some((Type::new("image"), Format("ome-zarr".into()))));
        assert_eq!(from_kind("raw/spikeglx").unwrap().0, Type::new("signal"));
        assert_eq!(from_kind("zarr/sorting").unwrap().0.namespace(), Some("dspk"));
        assert_eq!(from_kind("table/geoparquet").unwrap().0, Type::new("region"));
        assert_eq!(from_kind("file/png"), Some((Type::new("file"), Format("png".into()))));
        assert_eq!(from_kind("zarr/unknown"), None);
        assert_eq!(from_kind("nonsense"), None);
    }
}
