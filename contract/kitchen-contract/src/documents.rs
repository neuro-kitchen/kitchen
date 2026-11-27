//! Documents on disk, as JSON or TOML, in every app ([standard §2.2](../../../standard/spec/02-ops.md),
//! decision 0017): pipelines, recipes, op descriptions, settings. TOML is easier to write by hand
//! (dsp-kitchen's pipelines were the first to take it); JSON is what tools write. Either reads into
//! the same document, and a document's content id is the hash of its canonical JSON
//! ([`content_id`]), so the format a person chose never changes an id.

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::results::ContentId;

/// How a document is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocFormat {
    /// `.json`
    Json,
    /// `.toml`
    Toml,
}

impl DocFormat {
    /// The format of a file, by its extension (`.json`, `.toml`); `None` for anything else.
    pub fn of(path: &Path) -> Option<DocFormat> {
        match path.extension()?.to_str()? {
            "json" => Some(DocFormat::Json),
            "toml" => Some(DocFormat::Toml),
            _ => None,
        }
    }
}

/// `text` in `format` as a JSON value (what a document is, whatever it was written in).
pub fn to_value(text: &str, format: DocFormat) -> Result<serde_json::Value, String> {
    match format {
        DocFormat::Json => serde_json::from_str(text).map_err(|e| e.to_string()),
        DocFormat::Toml => {
            let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
            serde_json::to_value(value).map_err(|e| e.to_string())
        }
    }
}

/// `text` in `format` as a `T` (a [`Pipeline`](crate::Pipeline), an [`OpSpec`](crate::OpSpec), …).
pub fn parse<T: DeserializeOwned>(text: &str, format: DocFormat) -> Result<T, String> {
    serde_json::from_value(to_value(text, format)?).map_err(|e| e.to_string())
}

/// The document at `path` (`.json` or `.toml`).
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let format = DocFormat::of(path).ok_or_else(|| format!("{}: a document is .json or .toml", path.display()))?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text, format).map_err(|e| format!("{}: {e}", path.display()))
}

/// `value` as text in `format`: pretty JSON, or TOML (which has no null: absent fields are left
/// out, as the contract's types already do).
pub fn render<T: Serialize>(value: &T, format: DocFormat) -> Result<String, String> {
    match format {
        DocFormat::Json => serde_json::to_string_pretty(value).map(|s| s + "\n").map_err(|e| e.to_string()),
        DocFormat::Toml => toml::to_string_pretty(value).map_err(|e| e.to_string()),
    }
}

/// Writes `value` to `path` in the format its extension names.
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let format = DocFormat::of(path).ok_or_else(|| format!("{}: a document is .json or .toml", path.display()))?;
    std::fs::write(path, render(value, format)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// A document's content id: the hash of its canonical JSON, the same for its JSON and TOML forms.
pub fn content_id(text: &str, format: DocFormat) -> Result<ContentId, String> {
    Ok(ContentId::of_json(&to_value(text, format)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OpSpec, Pipeline};

    const PIPELINE_TOML: &str = r#"
version = 1
app = "lucem"
edges = [["image", "denoise.image"], ["denoise.image", "threshold.image"]]

[sources]
image = "image"

[nodes.denoise]
op = "lucem:denoise"
params = { sigma = 2.0 }

[nodes.threshold]
op = "lucem:threshold"

[outputs]
mask = "threshold.mask"
"#;

    const PIPELINE_JSON: &str = r#"{"version": 1, "app": "lucem", "sources": {"image": "image"},
      "nodes": {"denoise": {"op": "lucem:denoise", "params": {"sigma": 2.0}}, "threshold": {"op": "lucem:threshold"}},
      "edges": [["image", "denoise.image"], ["denoise.image", "threshold.image"]],
      "outputs": {"mask": "threshold.mask"}}"#;

    #[test]
    fn toml_and_json_read_into_the_same_document_with_the_same_id() {
        let from_toml: Pipeline = parse(PIPELINE_TOML, DocFormat::Toml).unwrap();
        let from_json: Pipeline = parse(PIPELINE_JSON, DocFormat::Json).unwrap();
        assert_eq!(from_toml, from_json);
        assert_eq!(content_id(PIPELINE_TOML, DocFormat::Toml).unwrap(), content_id(PIPELINE_JSON, DocFormat::Json).unwrap(), "the format never changes an id");
    }

    #[test]
    fn documents_write_and_read_back_in_either_format() {
        let dir = std::env::temp_dir().join(format!("kitchen-contract-docs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p: Pipeline = parse(PIPELINE_JSON, DocFormat::Json).unwrap();
        for name in ["p.json", "p.toml"] {
            let path = dir.join(name);
            write(&path, &p).unwrap();
            assert_eq!(read::<Pipeline>(&path).unwrap(), p, "{name}");
        }
        let op: OpSpec = parse("id = \"x\"\ntitle = \"X\"\ninputs = []\noutputs = [{ name = \"o\", type = \"image\" }]\nsource = { from = \"builtin\" }\n", DocFormat::Toml).unwrap();
        assert_eq!(op.version, 1);
        assert!(read::<Pipeline>(&dir.join("p.yaml")).is_err(), "only JSON and TOML");
    }
}
