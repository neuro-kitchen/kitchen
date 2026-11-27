//! The store on a person's machine: the workspace next to the data ([standard §2.2, §3.5](../../../standard/spec/03-results.md)).
//!
//! ```text
//! experiment-12/
//! ├── session_12.bin
//! └── .dspk/
//!     ├── runs/        one JSON file per run (the contract's RunRecord), appended, never edited
//!     └── results/     one JSON file per result: its record and where it is (kept by reference)
//! ```
//!
//! Everything is a plain file a person can read, copy and commit, so the history travels with the
//! folder (decision 0016). A lab's orchestrator keeps the same records in SQLite instead.
//!
//! - **Run records** are written once ([`Workspace::record_run`]); writing a run id twice is refused.
//! - **Results are kept by reference** ([`Workspace::keep`]): the file or Zarr folder stays where it
//!   is; the store keeps its id, type, format and place, and [`Workspace::locate`] checks the id
//!   before it gives the place back, so a changed file is never mistaken for its result.
//! - **Lineage** both ways, built only on written records: [`Workspace::lineage`] (back from a
//!   result to its sources) and [`Workspace::made_from`] ("what was made from this?").
//! - **Export** ([`Workspace::export`]): a result written where a person asks, with its run record
//!   beside it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use kitchen_contract::{ContentId, Format, ResultRecord, RunRecord, Type};
use serde::{Deserialize, Serialize};

use crate::identity::identify;
use crate::{ArtifactError, ArtifactResult};

/// A result the store keeps: its record, and where its bytes are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// The contract's record (id, type, format, size, run, incomplete).
    #[serde(flatten)]
    pub record: ResultRecord,
    /// Where it is: a file or a Zarr folder, absolute.
    pub path: PathBuf,
}

/// An app's workspace in a folder: `<folder>/.<app>/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// The workspace of `app` (`dspk`, `lucem`) in `folder`, made if it is not there yet.
    pub fn open(folder: &Path, app: &str) -> ArtifactResult<Self> {
        if app.is_empty() || !app.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(ArtifactError::Invalid(format!("app {app:?}: lowercase letters, digits and '-'")));
        }
        let root = folder.join(format!(".{app}"));
        for dir in [root.join("runs"), root.join("results")] {
            std::fs::create_dir_all(&dir).map_err(|e| ArtifactError::io(&dir, e))?;
        }
        Ok(Self { root })
    }

    /// The workspace's own folder (`.<app>/`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes `run`'s record, once: a run id already written is refused (a correction is a new run).
    pub fn record_run(&self, run: &RunRecord) -> ArtifactResult<PathBuf> {
        let path = self.root.join("runs").join(file_name(&run.run)?);
        if path.exists() {
            return Err(ArtifactError::Invalid(format!("run {} is already recorded: a correction is a new run", run.run)));
        }
        write_json(&path, run)?;
        Ok(path)
    }

    /// The record of run `id`.
    pub fn run(&self, id: &str) -> ArtifactResult<RunRecord> {
        read_json(&self.root.join("runs").join(file_name(id)?))
    }

    /// Every run recorded, in no particular order; a file that does not read is reported, not skipped.
    pub fn runs(&self) -> ArtifactResult<Vec<RunRecord>> {
        read_all(&self.root.join("runs"))
    }

    /// Keeps the file or Zarr folder at `path` as a result of type `ty` in `format`, made by `run`
    /// (`None` for a source): hashed now, kept where it is.
    pub fn keep(&self, path: &Path, ty: Type, format: Format, run: Option<String>, incomplete: bool) -> ArtifactResult<Kept> {
        let path = std::fs::canonicalize(path).map_err(|e| ArtifactError::io(path, e))?;
        let identity = identify(&path)?;
        let kept = Kept { record: ResultRecord { id: identity.id, ty, format, size: identity.bytes, run, incomplete }, path };
        write_json(&self.result_path(&kept.record.id), &kept)?;
        Ok(kept)
    }

    /// The result `id`, as kept.
    pub fn result(&self, id: &ContentId) -> ArtifactResult<Kept> {
        read_json(&self.result_path(id))
    }

    /// Every result kept.
    pub fn results(&self) -> ArtifactResult<Vec<Kept>> {
        read_all(&self.root.join("results"))
    }

    /// Where result `id` is, after checking that the bytes there are still that result.
    pub fn locate(&self, id: &ContentId) -> ArtifactResult<PathBuf> {
        let kept = self.result(id)?;
        let now = identify(&kept.path)?.id;
        if &now != id {
            return Err(ArtifactError::Invalid(format!("{}: changed since it was kept as {} (it is {} now)", kept.path.display(), id.short(), now.short())));
        }
        Ok(kept.path)
    }

    /// The runs that led to `id`, back to its sources: the run that made it, the runs that made
    /// that run's inputs, and so on. Nearest first. A run that reused an earlier run's outputs is
    /// not where they came from: the earlier run is.
    pub fn lineage(&self, id: &ContentId) -> ArtifactResult<Vec<RunRecord>> {
        let runs = self.runs()?;
        // The run that made each result: a run that reused an earlier one's outputs (its metrics
        // say `reused`) names them too, but did not make them
        let mut by_output: BTreeMap<&ContentId, &RunRecord> = BTreeMap::new();
        for r in runs.iter().filter(|r| r.metrics.contains_key("reused")).chain(runs.iter().filter(|r| !r.metrics.contains_key("reused"))) {
            for out in r.outputs.values() {
                by_output.insert(out, r);
            }
        }
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([id.clone()]);
        while let Some(next) = queue.pop_front() {
            let Some(run) = by_output.get(&next) else { continue };
            if !seen.insert(run.run.clone()) {
                continue;
            }
            queue.extend(run.inputs.values().cloned());
            out.push((*run).clone());
        }
        Ok(out)
    }

    /// What was made from `id`: every result whose lineage reaches it, nearest first (the runtime's
    /// `results` with `from`).
    pub fn made_from(&self, id: &ContentId) -> ArtifactResult<Vec<ContentId>> {
        let runs = self.runs()?;
        let mut out = Vec::new();
        let mut seen = BTreeSet::from([id.clone()]);
        let mut queue = VecDeque::from([id.clone()]);
        while let Some(next) = queue.pop_front() {
            for run in runs.iter().filter(|r| r.inputs.values().any(|i| i == &next)) {
                for made in run.outputs.values() {
                    if seen.insert(made.clone()) {
                        out.push(made.clone());
                        queue.push_back(made.clone());
                    }
                }
            }
        }
        Ok(out)
    }

    /// Writes result `id` to `dest` (a file copied, a Zarr folder copied whole) with the record of
    /// the run that made it beside it (`<dest>.run.json`); the id is checked first.
    pub fn export(&self, id: &ContentId, dest: &Path) -> ArtifactResult<PathBuf> {
        let from = self.locate(id)?;
        copy(&from, dest)?;
        if let Some(run) = self.result(id)?.record.run {
            let mut side = dest.as_os_str().to_owned();
            side.push(".run.json");
            write_json(Path::new(&side), &self.run(&run)?)?;
        }
        Ok(dest.to_path_buf())
    }

    fn result_path(&self, id: &ContentId) -> PathBuf {
        self.root.join("results").join(format!("{}.json", id.as_str().replace(':', "-")))
    }
}

/// A run id as a file name: letters, digits, `-`, `_`, `.`; nothing that leaves the folder.
fn file_name(id: &str) -> ArtifactResult<String> {
    let ok = !id.is_empty() && !id.starts_with('.') && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok { Ok(format!("{id}.json")) } else { Err(ArtifactError::Invalid(format!("run id {id:?}: letters, digits, '-', '_' or '.'"))) }
}

/// Writes `value` whole, through a temporary file renamed into place, so a reader never sees half.
fn write_json<T: Serialize>(path: &Path, value: &T) -> ArtifactResult<()> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| ArtifactError::Invalid(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| ArtifactError::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| ArtifactError::io(path, e))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> ArtifactResult<T> {
    let bytes = std::fs::read(path).map_err(|e| ArtifactError::io(path, e))?;
    serde_json::from_slice(&bytes).map_err(|e| ArtifactError::Invalid(format!("{}: {e}", path.display())))
}

fn read_all<T: for<'de> Deserialize<'de>>(dir: &Path) -> ArtifactResult<Vec<T>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| ArtifactError::io(dir, e))? {
        let path = entry.map_err(|e| ArtifactError::io(dir, e))?.path();
        if path.extension().is_some_and(|e| e == "json") {
            out.push(read_json(&path)?);
        }
    }
    Ok(out)
}

fn copy(from: &Path, to: &Path) -> ArtifactResult<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| ArtifactError::io(to, e))?;
        for entry in std::fs::read_dir(from).map_err(|e| ArtifactError::io(from, e))? {
            let entry = entry.map_err(|e| ArtifactError::io(from, e))?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ()).map_err(|e| ArtifactError::io(from, e))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kitchen_contract::{CONTRACT_VERSION, OpRef, ParamValue, RunKey, RunStatus};

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kitchen-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(id: &str, op: &str, inputs: &[(&str, &ContentId)], outputs: &[(&str, &ContentId)], status: RunStatus) -> RunRecord {
        let params: BTreeMap<String, ParamValue> = BTreeMap::new();
        let inputs: BTreeMap<String, ContentId> = inputs.iter().map(|(k, v)| (k.to_string(), (*v).clone())).collect();
        RunRecord {
            contract: CONTRACT_VERSION.into(),
            run: id.into(),
            op: OpRef { id: op.into(), version: "v1".into() },
            run_key: RunKey::of(op, "v1", &params, &inputs),
            params,
            inputs,
            outputs: outputs.iter().map(|(k, v)| (k.to_string(), (*v).clone())).collect(),
            by: "ana".into(),
            via: "dsp-kitchen".into(),
            started: "2026-11-27T10:00:00Z".into(),
            ended: None,
            status,
            environment: BTreeMap::new(),
            metrics: BTreeMap::new(),
        }
    }

    #[test]
    fn runs_are_written_once_and_read_back() {
        let dir = scratch("runs");
        let ws = Workspace::open(&dir, "dspk").unwrap();
        assert!(ws.root().ends_with(".dspk"));
        let a = ContentId::of_bytes(b"recording");
        let r = run("run-1", "dspk:highpass", &[("signal", &a)], &[], RunStatus::Succeeded);
        let path = ws.record_run(&r).unwrap();
        assert!(path.starts_with(dir.join(".dspk/runs")));
        assert_eq!(ws.run("run-1").unwrap(), r);
        assert!(ws.record_run(&r).is_err(), "appended, never edited");
        assert!(ws.run("../escape").is_err());
        assert!(Workspace::open(&dir, "Not An App").is_err());
        assert_eq!(ws.runs().unwrap().len(), 1);
    }

    #[test]
    fn results_are_kept_where_they_are_and_checked_when_read() {
        let dir = scratch("keep");
        let ws = Workspace::open(&dir, "lucem").unwrap();
        let file = dir.join("cells.parquet");
        std::fs::write(&file, b"rows").unwrap();
        let kept = ws.keep(&file, Type::new("table"), Format::parse("parquet").unwrap(), None, false).unwrap();
        assert_eq!(kept.record.id, ContentId::of_bytes(b"rows"));
        assert_eq!(ws.result(&kept.record.id).unwrap(), kept);
        assert_eq!(ws.locate(&kept.record.id).unwrap(), std::fs::canonicalize(&file).unwrap());
        std::fs::write(&file, b"other rows").unwrap();
        assert!(ws.locate(&kept.record.id).unwrap_err().to_string().contains("changed since it was kept"));
        // A Zarr folder is kept by reference too
        let zarr = dir.join("labels.zarr");
        std::fs::create_dir_all(zarr.join("0")).unwrap();
        std::fs::write(zarr.join("0/0.0"), b"chunk").unwrap();
        let z = ws.keep(&zarr, Type::new("labels"), Format::parse("ome-zarr").unwrap(), Some("run-9".into()), true).unwrap();
        assert!(z.record.incomplete, "a cancelled run's output is marked");
        assert_eq!(ws.results().unwrap().len(), 2);
    }

    #[test]
    fn lineage_goes_back_and_made_from_goes_forward() {
        let dir = scratch("lineage");
        let ws = Workspace::open(&dir, "dspk").unwrap();
        let (raw, filtered, sorting, figure, other) = (ContentId::of_bytes(b"raw"), ContentId::of_bytes(b"hp"), ContentId::of_bytes(b"ks"), ContentId::of_bytes(b"fig"), ContentId::of_bytes(b"other"));
        ws.record_run(&run("r1", "dspk:highpass", &[("signal", &raw)], &[("signal", &filtered)], RunStatus::Succeeded)).unwrap();
        ws.record_run(&run("r2", "dspk:kilosort", &[("signal", &filtered)], &[("sorting", &sorting)], RunStatus::Succeeded)).unwrap();
        ws.record_run(&run("r3", "script:plot", &[("sorting", &sorting)], &[("figure", &figure)], RunStatus::Succeeded)).unwrap();
        ws.record_run(&run("r4", "dspk:highpass", &[("signal", &other)], &[], RunStatus::Succeeded)).unwrap();
        let back: Vec<String> = ws.lineage(&figure).unwrap().into_iter().map(|r| r.run).collect();
        assert_eq!(back, ["r3", "r2", "r1"], "from the figure back to the recording");
        assert_eq!(ws.made_from(&raw).unwrap(), [filtered.clone(), sorting.clone(), figure.clone()], "what was made from this recording");
        assert!(ws.made_from(&figure).unwrap().is_empty());
        assert!(ws.lineage(&raw).unwrap().is_empty(), "a source has no lineage");
    }

    #[test]
    fn export_writes_the_result_and_its_run_beside_it() {
        let dir = scratch("export");
        let ws = Workspace::open(&dir, "dspk").unwrap();
        let file = dir.join("spikes.parquet");
        std::fs::write(&file, b"spikes").unwrap();
        let kept = ws.keep(&file, Type::new("events"), Format::parse("parquet").unwrap(), Some("r1".into()), false).unwrap();
        let raw = ContentId::of_bytes(b"raw");
        ws.record_run(&run("r1", "dspk:detect", &[("signal", &raw)], &[("spikes", &kept.record.id)], RunStatus::Succeeded)).unwrap();
        let dest = dir.join("out").join("session_12.spikes.parquet");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        ws.export(&kept.record.id, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"spikes");
        let side: RunRecord = serde_json::from_slice(&std::fs::read(dir.join("out/session_12.spikes.parquet.run.json")).unwrap()).unwrap();
        assert_eq!(side.run, "r1");
    }
}
