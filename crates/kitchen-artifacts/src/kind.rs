//! Artifact kinds: `family/format`, lowercase.
//!
//! | Family | Is | Examples |
//! |---|---|---|
//! | `raw` | a recording or image as an instrument wrote it (a file or a folder) | `raw/spikeglx`, `raw/czi`, `raw/tiff` |
//! | `zarr` | a Zarr store (a folder or a prefix): the middle of most chains | `zarr/nwb`, `zarr/ome`, `zarr/sorting`, `zarr/labels` |
//! | `table` | a table file | `table/parquet`, `table/geoparquet`, `table/csv` |
//! | `file` | anything else a tool writes (a figure, a report) | `file/png`, `file/json` |
//!
//! New formats need no change here: a kind is any `family/format` of the known families. The
//! constants name the ones the current tools use.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ArtifactError, ArtifactResult};

/// The families an artifact kind can belong to.
pub const FAMILIES: [&str; 4] = ["raw", "zarr", "table", "file"];

/// NWB in a Zarr store (neuro-convert writes it; dsp-kitchen reads it).
pub const ZARR_NWB: &str = "zarr/nwb";
/// OME-Zarr images (lucem reads and writes them).
pub const ZARR_OME: &str = "zarr/ome";
/// A spike sorting in a Zarr store (dsp-kitchen's `.sorting.zarr`).
pub const ZARR_SORTING: &str = "zarr/sorting";
/// Label images and objects in a Zarr store (lucem's segmentation).
pub const ZARR_LABELS: &str = "zarr/labels";
/// An Apache Parquet table.
pub const TABLE_PARQUET: &str = "table/parquet";
/// A GeoParquet table (shapes with their measurements).
pub const TABLE_GEOPARQUET: &str = "table/geoparquet";

/// What an artifact is: `family/format`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactKind(String);

impl ArtifactKind {
    /// `text` as a kind, checked: a known family, then a format of lowercase letters, digits,
    /// `-`, `_` or `.`.
    pub fn parse(text: &str) -> ArtifactResult<Self> {
        let (family, format) = text
            .split_once('/')
            .ok_or_else(|| ArtifactError::Invalid(format!("kind {text:?}: expected family/format")))?;
        if !FAMILIES.contains(&family) {
            return Err(ArtifactError::Invalid(format!("kind {text:?}: the family is one of {}", FAMILIES.join(", "))));
        }
        if !valid_format(format) {
            return Err(ArtifactError::Invalid(format!(
                "kind {text:?}: the format is lowercase letters, digits, '-', '_' or '.'"
            )));
        }
        Ok(Self(text.to_string()))
    }

    /// `raw`, `zarr`, `table` or `file`.
    pub fn family(&self) -> &str {
        self.0.split_once('/').map_or("", |(f, _)| f)
    }

    /// The part after the family (`nwb` of `zarr/nwb`).
    pub fn format(&self) -> &str {
        self.0.split_once('/').map_or("", |(_, f)| f)
    }

    /// Whether artifacts of this kind are stores (folders or prefixes), not single files: every
    /// `zarr` kind. A `raw` kind may be either; its path decides.
    pub fn is_store(&self) -> bool {
        self.family() == "zarr"
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_format(format: &str) -> bool {
    !format.is_empty()
        && format.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'))
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ArtifactKind {
    type Error = ArtifactError;
    fn try_from(text: String) -> ArtifactResult<Self> {
        Self::parse(&text)
    }
}

impl From<ArtifactKind> for String {
    fn from(kind: ArtifactKind) -> String {
        kind.0
    }
}

impl std::str::FromStr for ArtifactKind {
    type Err = ArtifactError;
    fn from_str(text: &str) -> ArtifactResult<Self> {
        Self::parse(text)
    }
}

/// What a tool accepts: one kind (`zarr/nwb`), or every format of a family (`raw/*`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KindPattern(String);

impl KindPattern {
    pub fn parse(text: &str) -> ArtifactResult<Self> {
        match text.strip_suffix("/*") {
            Some(family) if FAMILIES.contains(&family) => Ok(Self(text.to_string())),
            Some(_) => {
                Err(ArtifactError::Invalid(format!("pattern {text:?}: the family is one of {}", FAMILIES.join(", "))))
            }
            None => ArtifactKind::parse(text).map(|k| Self(k.0)),
        }
    }

    /// Whether `kind` is one this pattern accepts.
    pub fn matches(&self, kind: &ArtifactKind) -> bool {
        match self.0.strip_suffix("/*") {
            Some(family) => kind.family() == family,
            None => self.0 == kind.0,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KindPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for KindPattern {
    type Error = ArtifactError;
    fn try_from(text: String) -> ArtifactResult<Self> {
        Self::parse(&text)
    }
}

impl From<KindPattern> for String {
    fn from(pattern: KindPattern) -> String {
        pattern.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_parse_and_patterns_match() {
        let nwb = ArtifactKind::parse(ZARR_NWB).unwrap();
        assert_eq!((nwb.family(), nwb.format(), nwb.is_store()), ("zarr", "nwb", true));
        for bad in ["nwb", "zarr/", "Zarr/nwb", "video/mp4", "zarr/NWB", "zarr/a b"] {
            assert!(ArtifactKind::parse(bad).is_err(), "{bad}");
        }
        let raw = KindPattern::parse("raw/*").unwrap();
        assert!(raw.matches(&ArtifactKind::parse("raw/spikeglx").unwrap()));
        assert!(!raw.matches(&nwb));
        assert!(KindPattern::parse(ZARR_NWB).unwrap().matches(&nwb));
        assert!(KindPattern::parse("video/*").is_err());
        // As JSON: plain strings, checked on the way in
        assert_eq!(serde_json::to_string(&nwb).unwrap(), r#""zarr/nwb""#);
        assert!(serde_json::from_str::<ArtifactKind>(r#""nope""#).is_err());
    }
}
