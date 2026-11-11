//! `kitchen-hub`: verified downloads of published artifacts (model weights, data files) and their
//! local cache, shared by the neuro-kitchen apps and their plugins (lumen's and dsp-kitchen's
//! hubs, which were near twins). All network access of the apps' models is here.
//!
//! - An [`Artifact`] says what to fetch: its id, family, file name, URI ([`providers`]: `hf://`,
//!   `gh://`, `zenodo://`, `https://`, `file://`), SHA-256 and size.
//! - [`Hub::pull`] fetches it into the cache: to a temporary name, hashed while it streams,
//!   committed only when its SHA-256 and size match. An artifact without a hash is refused. Its
//!   progress goes to a callback; a cancel flag stops it.
//! - [`Hub::status`] says whether it is there (from the cache index, without hashing);
//!   [`Hub::verify`] hashes it again; [`Hub::remove`], [`Hub::clean`], [`Hub::disk_use`] manage
//!   the space it takes.
//! - [`fetch_archive`] fetches a `.tgz` the same way and unpacks it.
//!
//! The cache is the app's: `Hub::for_app("lumen", "LUMEN_MODELS")` is `$LUMEN_MODELS`, else
//! `$XDG_CACHE_HOME/lumen/hub`, else `~/.cache/lumen/hub`; one folder per family inside.

#![warn(missing_docs)]

pub mod providers;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use providers::{Provider, resolve};

/// A published file to fetch and verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Unique id, `<family>/<name>` (`stardist/2d-versatile-he`).
    pub id: String,
    /// The family it belongs to (one cache folder each).
    pub family: String,
    /// Its file name in the cache.
    pub file_name: String,
    /// Where it comes from ([`providers`]).
    pub uri: String,
    /// Its SHA-256 (lowercase hex). Required.
    pub sha256: String,
    /// Its size in bytes (0: not known; then only the hash is checked).
    #[serde(default)]
    pub bytes: u64,
}

impl Artifact {
    /// The last part of the id (its folder in the family's).
    pub fn slug(&self) -> &str {
        self.id.rsplit('/').next().unwrap_or(&self.id)
    }
}

/// Whether an artifact is in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Downloaded and recorded with the expected hash and size.
    Installed,
    /// Not downloaded.
    Available,
    /// A file is there but its record or size does not match.
    Corrupted,
}

impl Status {
    /// As people read it.
    pub fn label(self) -> &'static str {
        match self {
            Status::Installed => "Installed",
            Status::Available => "Not installed",
            Status::Corrupted => "Damaged: install again",
        }
    }
}

/// The cache index's record of an installed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Its SHA-256.
    pub sha256: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// Where it came from.
    pub uri: String,
}

/// Downloads into, and reads from, one app's cache.
#[derive(Debug, Clone)]
pub struct Hub {
    root: PathBuf,
}

impl Hub {
    /// A hub over the cache folder `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The app's cache: `$<env>` when set, else `$XDG_CACHE_HOME/<app>/hub`, else
    /// `~/.cache/<app>/hub` (the system temp folder when there is no home).
    pub fn for_app(app: &str, env: &str) -> Self {
        if let Some(dir) = std::env::var_os(env).filter(|d| !d.is_empty()) {
            return Self::new(dir);
        }
        let base = std::env::var_os("XDG_CACHE_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(std::env::temp_dir);
        Self::new(base.join(app).join("hub"))
    }

    /// The cache folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A family's folder.
    pub fn family_dir(&self, family: &str) -> PathBuf {
        self.root.join(family)
    }

    /// Where `artifact` is (or would be) in the cache.
    pub fn path(&self, artifact: &Artifact) -> PathBuf {
        self.family_dir(&artifact.family).join(artifact.slug()).join(&artifact.file_name)
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    /// The cache index: installed artifacts by id.
    pub fn index(&self) -> BTreeMap<String, Record> {
        std::fs::read(self.index_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save_index(&self, index: &BTreeMap<String, Record>) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))?;
        let text = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
        let tmp = self.index_path().with_extension("json.part");
        std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, self.index_path())).map_err(|e| format!("{}: {e}", self.index_path().display()))
    }

    /// Whether `artifact` is in the cache, from the index and the file's size (no hashing).
    pub fn status(&self, artifact: &Artifact) -> Status {
        let path = self.path(artifact);
        let Ok(meta) = std::fs::metadata(&path) else { return Status::Available };
        match self.index().get(&artifact.id) {
            Some(r) if r.sha256.eq_ignore_ascii_case(&artifact.sha256) && r.bytes == meta.len() && (artifact.bytes == 0 || artifact.bytes == meta.len()) => Status::Installed,
            _ => Status::Corrupted,
        }
    }

    /// Hashes the cached file again: `Installed` only when it is the published one.
    pub fn verify(&self, artifact: &Artifact) -> Status {
        let path = self.path(artifact);
        if !path.exists() {
            return Status::Available;
        }
        if is_valid(&path, &artifact.sha256, artifact.bytes) { Status::Installed } else { Status::Corrupted }
    }

    /// Fetches `artifact` into the cache unless a valid copy is there; `progress(done, total)`
    /// follows the bytes, `cancel` stops it. Returns the file's path.
    pub fn pull(&self, artifact: &Artifact, progress: &mut dyn FnMut(u64, u64), cancel: Option<&AtomicBool>) -> Result<PathBuf, String> {
        let dest = self.path(artifact);
        if self.status(artifact) == Status::Installed {
            return Ok(dest);
        }
        fetch(&artifact.uri, &dest, &artifact.sha256, artifact.bytes, progress, cancel)?;
        let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(artifact.bytes);
        let mut index = self.index();
        index.insert(artifact.id.clone(), Record { sha256: artifact.sha256.to_lowercase(), bytes, uri: artifact.uri.clone() });
        self.save_index(&index)?;
        Ok(dest)
    }

    /// Removes `artifact` from the cache; whether it was there.
    pub fn remove(&self, artifact: &Artifact) -> Result<bool, String> {
        let dir = self.path(artifact).parent().map(Path::to_path_buf).unwrap_or_default();
        let was = dir.exists();
        if was {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut index = self.index();
        if index.remove(&artifact.id).is_some() {
            self.save_index(&index)?;
        }
        Ok(was)
    }

    /// Empties the cache; the bytes freed.
    pub fn clean(&self) -> Result<u64, String> {
        let freed = dir_size(&self.root);
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))?;
        }
        Ok(freed)
    }

    /// Bytes the cache takes, all of it (`None`) or one family's.
    pub fn disk_use(&self, family: Option<&str>) -> u64 {
        match family {
            Some(f) => dir_size(&self.family_dir(f)),
            None => dir_size(&self.root),
        }
    }
}

/// Bytes the files under `dir` take.
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return std::fs::metadata(dir).map(|m| m.len()).unwrap_or(0) };
    entries.flatten().map(|e| {
        let p = e.path();
        if p.is_dir() { dir_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }
    }).sum()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of a file (lowercase hex).
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Whether `path` holds the expected file (size first, when known, then hash).
pub fn is_valid(path: &Path, sha256: &str, bytes: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| bytes == 0 || m.len() == bytes) && sha256_file(path).is_ok_and(|h| h.eq_ignore_ascii_case(sha256))
}

/// Reads a file only if its hash matches (what a model loads).
pub fn read_verified(path: &Path, sha256: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let got = hex(&Sha256::digest(&bytes));
    if got.eq_ignore_ascii_case(sha256) {
        Ok(bytes)
    } else {
        Err(format!("{}: the file is not the published one (SHA-256 {got}, expected {sha256}); delete it and install again", path.display()))
    }
}

/// Fetches `uri` to `dest` unless a valid copy is there; `progress(done, total)` follows the
/// bytes; `cancel` stops it (the partial file is removed). The file is committed only when its
/// SHA-256 (and size, when given) match; a file without a published hash is refused.
pub fn fetch(uri: &str, dest: &Path, sha256: &str, bytes: u64, progress: &mut dyn FnMut(u64, u64), cancel: Option<&AtomicBool>) -> Result<PathBuf, String> {
    if sha256.trim().is_empty() {
        return Err(format!("{uri}: no hash published; refused"));
    }
    if is_valid(dest, sha256, bytes) {
        return Ok(dest.to_path_buf());
    }
    let (provider, url) = resolve(uri)?;
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let (mut reader, total): (Box<dyn Read>, u64) = match provider {
        Provider::LocalFile => {
            let path = url.strip_prefix("file://").unwrap_or(&url);
            let f = File::open(path).map_err(|e| format!("{path}: {e}"))?;
            let len = f.metadata().map(|m| m.len()).unwrap_or(bytes);
            (Box::new(f), len)
        }
        _ => {
            let resp = ureq::get(&url).call().map_err(|e| format!("{url}: {e}"))?;
            let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(bytes);
            (Box::new(resp.into_body().into_reader()), total)
        }
    };
    let part = dest.with_extension("part");
    let result = (|| {
        let mut out = File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut h = Sha256::new();
        let (mut done, mut buf) = (0u64, vec![0u8; 1 << 20]);
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(format!("{uri}: cancelled"));
            }
            let n = reader.read(&mut buf).map_err(|e| format!("{url}: {e}"))?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            progress(done, total);
        }
        out.flush().map_err(|e| e.to_string())?;
        let got = hex(&h.finalize());
        if !got.eq_ignore_ascii_case(sha256) || (bytes != 0 && done != bytes) {
            return Err(format!("{uri}: the download does not match (SHA-256 {got}, {done} bytes; expected {sha256}, {bytes} bytes)"));
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
    Ok(dest.to_path_buf())
}

/// Fetches a `.tgz` (verified as [`fetch`]) and unpacks it into `dest_dir`, unless `dest_dir`
/// already holds it (a `.complete` mark is written last). The archive is removed once unpacked.
pub fn fetch_archive(uri: &str, archive: &Path, sha256: &str, bytes: u64, dest_dir: &Path, progress: &mut dyn FnMut(u64, u64), cancel: Option<&AtomicBool>) -> Result<PathBuf, String> {
    let mark = dest_dir.join(".complete");
    if std::fs::read_to_string(&mark).is_ok_and(|m| m.trim().eq_ignore_ascii_case(sha256)) {
        return Ok(dest_dir.to_path_buf());
    }
    fetch(uri, archive, sha256, bytes, progress, cancel)?;
    let file = File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let _ = std::fs::remove_dir_all(dest_dir);
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
    tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(dest_dir).map_err(|e| format!("{}: {e}", archive.display()))?;
    std::fs::write(&mark, sha256).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(archive);
    Ok(dest_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kitchen-hub-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_file_is_read_only_with_its_hash() {
        let d = scratch("hash");
        let p = d.join("abc");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), ABC);
        assert!(read_verified(&p, ABC).is_ok() && read_verified(&p, &"0".repeat(64)).is_err());
        assert!(is_valid(&p, ABC, 3) && !is_valid(&p, ABC, 4) && is_valid(&p, ABC, 0));
        assert!(fetch("https://example.invalid/x", &d.join("x"), "", 0, &mut |_, _| {}, None).unwrap_err().contains("no hash"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pulling_verifies_records_and_removes() {
        let d = scratch("pull");
        let src = d.join("weights.bin");
        std::fs::write(&src, b"abc").unwrap();
        let hub = Hub::new(d.join("cache"));
        let good = Artifact { id: "demo/weights".into(), family: "demo".into(), file_name: "w.bin".into(), uri: format!("file://{}", src.display()), sha256: ABC.into(), bytes: 3 };
        assert_eq!(hub.status(&good), Status::Available);
        let mut seen = 0;
        let path = hub.pull(&good, &mut |done, _| seen = done, None).unwrap();
        assert_eq!((seen, std::fs::read(&path).unwrap()), (3, b"abc".to_vec()));
        assert_eq!(hub.status(&good), Status::Installed);
        assert_eq!(hub.verify(&good), Status::Installed);
        assert_eq!(hub.disk_use(Some("demo")), 3);
        // A changed file reads as damaged
        std::fs::write(&path, b"abcd").unwrap();
        assert_eq!(hub.status(&good), Status::Corrupted);
        assert_eq!(hub.verify(&good), Status::Corrupted);
        assert!(hub.remove(&good).unwrap());
        assert_eq!(hub.status(&good), Status::Available);

        // A wrong hash is refused and leaves nothing behind
        let bad = Artifact { sha256: "0".repeat(64), ..good.clone() };
        assert!(hub.pull(&bad, &mut |_, _| {}, None).unwrap_err().contains("does not match"));
        assert!(!hub.path(&bad).exists() && !hub.path(&bad).with_extension("part").exists());
        // Cancelled before the first read
        let stop = AtomicBool::new(true);
        assert!(hub.pull(&good, &mut |_, _| {}, Some(&stop)).unwrap_err().contains("cancelled"));
        assert!(hub.clean().is_ok() && !hub.root().exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn archives_unpack_once() {
        let d = scratch("tgz");
        let tgz = d.join("a.tgz");
        {
            let f = File::create(&tgz).unwrap();
            let mut t = tar::Builder::new(flate2::write::GzEncoder::new(f, flate2::Compression::default()));
            let mut h = tar::Header::new_gnu();
            h.set_size(3);
            h.set_mode(0o644);
            h.set_cksum();
            t.append_data(&mut h, "model/abc.txt", &b"abc"[..]).unwrap();
            t.into_inner().unwrap().finish().unwrap();
        }
        let sha = sha256_file(&tgz).unwrap();
        let out = d.join("out");
        let dir = fetch_archive(&format!("file://{}", tgz.display()), &d.join("dl.tgz"), &sha, 0, &out, &mut |_, _| {}, None).unwrap();
        assert_eq!(std::fs::read(dir.join("model/abc.txt")).unwrap(), b"abc");
        // Unpacked: not fetched again
        assert!(fetch_archive("https://example.invalid/never", &d.join("dl.tgz"), &sha, 0, &out, &mut |_, _| {}, None).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_app_names_its_cache() {
        let hub = Hub::for_app("kitchen-test-app", "KITCHEN_TEST_HUB_UNSET");
        assert!(hub.root().ends_with("kitchen-test-app/hub"));
    }
}
