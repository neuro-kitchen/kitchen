//! Where artifacts come from (from dsp-kitchen's hub): a URI names the provider and the file,
//! resolved to an HTTPS URL (or a local file).
//!
//! - `hf://<org>/<repo>[@<rev>]/<path>`: Hugging Face (`main` by default);
//! - `gh://<owner>/<repo>[@<rev>]/<path>`: a file in a GitHub repository;
//! - `zenodo://<record>/<file>`: a Zenodo record's file;
//! - `https://…` (or `http://…`): as is;
//! - `file:///…`: a local file (tests, offline installs).

/// Which provider a URI names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Hugging Face.
    HuggingFace,
    /// A GitHub repository.
    GitHub,
    /// Zenodo.
    Zenodo,
    /// A plain HTTP(S) URL.
    Https,
    /// A local file.
    LocalFile,
}

fn repo_rev_path<'a>(uri: &str, rest: &'a str) -> Result<(&'a str, &'a str, &'a str), String> {
    let mut parts = rest.splitn(3, '/');
    let (org, repo_rev, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if org.is_empty() || repo_rev.is_empty() || path.is_empty() {
        return Err(format!("{uri}: expected <owner>/<repo>[@<revision>]/<path>"));
    }
    let (repo, rev) = match repo_rev.split_once('@') {
        Some((r, v)) if !r.is_empty() && !v.is_empty() => (r, v),
        Some(_) => return Err(format!("{uri}: malformed @revision")),
        None => (repo_rev, "main"),
    };
    Ok((&rest[..org.len() + 1 + repo.len()], rev, path))
}

/// The provider of `uri` and the URL (or `file://` path) to fetch.
pub fn resolve(uri: &str) -> Result<(Provider, String), String> {
    let uri = uri.trim();
    if let Some(rest) = uri.strip_prefix("hf://") {
        let (repo, rev, path) = repo_rev_path(uri, rest)?;
        Ok((Provider::HuggingFace, format!("https://huggingface.co/{repo}/resolve/{rev}/{path}")))
    } else if let Some(rest) = uri.strip_prefix("gh://") {
        let (repo, rev, path) = repo_rev_path(uri, rest)?;
        Ok((Provider::GitHub, format!("https://raw.githubusercontent.com/{repo}/{rev}/{path}")))
    } else if let Some(rest) = uri.strip_prefix("zenodo://") {
        match rest.split_once('/') {
            Some((record, file)) if !record.is_empty() && !file.is_empty() => Ok((Provider::Zenodo, format!("https://zenodo.org/records/{record}/files/{file}?download=1"))),
            _ => Err(format!("{uri}: expected zenodo://<record>/<file>")),
        }
    } else if uri.starts_with("https://") || uri.starts_with("http://") {
        Ok((Provider::Https, uri.to_string()))
    } else if uri.starts_with("file://") {
        Ok((Provider::LocalFile, uri.to_string()))
    } else {
        Err(format!("{uri}: unsupported scheme (expected hf://, gh://, zenodo://, https:// or file://)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_resolve_to_their_provider() {
        assert_eq!(
            resolve("hf://SpikeInterface/UnitRefine@main/models/unitrefine.onnx").unwrap(),
            (Provider::HuggingFace, "https://huggingface.co/SpikeInterface/UnitRefine/resolve/main/models/unitrefine.onnx".into())
        );
        assert_eq!(resolve("gh://MouseLand/Kilosort@v4.0/kilosort/models/wPCA.npy").unwrap().1, "https://raw.githubusercontent.com/MouseLand/Kilosort/v4.0/kilosort/models/wPCA.npy");
        assert_eq!(resolve("gh://a/b/c.bin").unwrap().1, "https://raw.githubusercontent.com/a/b/main/c.bin");
        assert_eq!(resolve("zenodo://123/w.pt").unwrap().1, "https://zenodo.org/records/123/files/w.pt?download=1");
        assert_eq!(resolve("https://x.org/a").unwrap().0, Provider::Https);
        assert_eq!(resolve("file:///tmp/a").unwrap().0, Provider::LocalFile);
        assert!(resolve("ftp://x").is_err());
        assert!(resolve("hf://only-org").is_err());
        assert!(resolve("gh://a/b@/c").is_err());
    }
}
