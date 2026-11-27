//! Content identity: the same bytes give the same id, wherever they are and whatever they are
//! called. A run's key is built from its inputs' ids, so identical inputs are never processed twice,
//! and a result can always say exactly what it was computed from.
//!
//! - A **file**: the SHA-256 of its bytes (the same value as `sha256sum`).
//! - A **store** (a Zarr folder, a raw folder): the SHA-256 of its [`Manifest`]: every file's
//!   relative path, size and SHA-256, sorted by path. Chunks are hashed by content, not by size:
//!   float chunks of equal size almost always hold different values. The provenance file at the
//!   store's root ([`STORE_PROVENANCE_FILE`]) is not part of the identity, so
//!   writing it does not change the id it records.
//!
//! Hashing reads every byte. The orchestrator keeps ids by path, size and modification time, so
//! a store is hashed again only when it changed.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ArtifactError, ArtifactResult, STORE_PROVENANCE_FILE};

/// What a store's manifest starts with, so a store's id never equals a file's.
const STORE_DOMAIN: &[u8] = b"kitchen-store-v1\n";

/// An artifact's content identity: the contract's [`ContentId`] (`sha256:<64 hex digits>`).
pub use kitchen_contract::ContentId as ArtifactId;
use kitchen_contract::results::to_hex;

/// An artifact's id, with what was hashed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub id: ArtifactId,
    /// Total bytes hashed.
    pub bytes: u64,
    /// Files hashed (1 for a file).
    pub files: u64,
}

/// One file of a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Relative to the store's root, `/`-separated.
    pub path: String,
    pub bytes: u64,
    /// SHA-256 of the file, lowercase hex.
    pub sha256: String,
}

/// Every file of a store, sorted by path: what a store's id is the hash of.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// The store's id: the hash of its entries, one `path NUL bytes NUL sha256 LF` each.
    pub fn id(&self) -> ArtifactId {
        let mut hasher = Sha256::new();
        hasher.update(STORE_DOMAIN);
        for e in &self.entries {
            hasher.update(e.path.as_bytes());
            hasher.update([0]);
            hasher.update(e.bytes.to_string().as_bytes());
            hasher.update([0]);
            hasher.update(e.sha256.as_bytes());
            hasher.update(b"\n");
        }
        ArtifactId::from_sha256(&hasher.finalize())
    }
}

/// The identity of the file or store at `path`.
pub fn identify(path: &Path) -> ArtifactResult<Identity> {
    let meta = std::fs::metadata(path).map_err(|e| ArtifactError::io(path, e))?;
    if meta.is_dir() {
        let manifest = manifest(path)?;
        let bytes = manifest.entries.iter().map(|e| e.bytes).sum();
        Ok(Identity { id: manifest.id(), bytes, files: manifest.entries.len() as u64 })
    } else {
        let (digest, bytes) = hash_file(path)?;
        Ok(Identity { id: ArtifactId::from_sha256(&digest), bytes, files: 1 })
    }
}

/// The manifest of the store at `root`. Symbolic links are refused: a link could point outside
/// the store, or at itself.
pub fn manifest(root: &Path) -> ArtifactResult<Manifest> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut entries = Vec::with_capacity(files.len());
    for (relative, path) in files {
        let (digest, bytes) = hash_file(&path)?;
        entries.push(ManifestEntry { path: relative, bytes, sha256: to_hex(&digest) });
    }
    Ok(Manifest { entries })
}

/// Every file under `dir`, as (relative `/`-separated path, full path), the store's own
/// provenance file left out.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> ArtifactResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| ArtifactError::io(dir, e))? {
        let entry = entry.map_err(|e| ArtifactError::io(dir, e))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| ArtifactError::io(&path, e))?;
        if kind.is_symlink() {
            return Err(ArtifactError::Invalid(format!("{}: a symbolic link inside a store", path.display())));
        }
        let relative = path
            .strip_prefix(root)
            .expect("collected under the root")
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if kind.is_dir() {
            collect(root, &path, out)?;
        } else if relative != STORE_PROVENANCE_FILE {
            out.push((relative, path));
        }
    }
    Ok(())
}

/// SHA-256 of a file's bytes, read in pieces, and its length.
fn hash_file(path: &Path) -> ArtifactResult<(Vec<u8>, u64)> {
    let mut file = File::open(path).map_err(|e| ArtifactError::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|e| ArtifactError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((hasher.finalize().to_vec(), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kitchen-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_is_its_sha256() {
        let dir = scratch("file");
        let path = dir.join("x.bin");
        std::fs::write(&path, b"abc").unwrap();
        let identity = identify(&path).unwrap();
        assert_eq!(identity.id.as_str(), "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!((identity.bytes, identity.files), (3, 1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_store_is_its_content_not_its_place() {
        let (a, b) = (scratch("store-a"), scratch("store-b"));
        for root in [&a, &b] {
            std::fs::create_dir_all(root.join("0/0")).unwrap();
            std::fs::write(root.join("zarr.json"), b"{}").unwrap();
            std::fs::write(root.join("0/0/c"), b"chunk").unwrap();
        }
        let id = identify(&a).unwrap().id;
        assert_eq!(id, identify(&b).unwrap().id, "same content, other folder: same id");
        // The provenance file does not count; a chunk of the same size but other bytes does
        std::fs::write(b.join(STORE_PROVENANCE_FILE), b"{}").unwrap();
        assert_eq!(id, identify(&b).unwrap().id);
        std::fs::write(b.join("0/0/c"), b"CHUNK").unwrap();
        assert_ne!(id, identify(&b).unwrap().id);
        // A store never has a file's id
        assert_eq!(identify(&a).unwrap().files, 2);
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    #[test]
    fn ids_parse() {
        assert!(ArtifactId::parse("sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").is_ok());
        for bad in ["ba78", "sha256:XYZ", "md5:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"] {
            assert!(ArtifactId::parse(bad).is_err(), "{bad}");
        }
    }
}
