//! [`Runtime`]: anything that answers the calls. The in-process runtime ([`crate::Local`]) is one;
//! a connection to a server (step 4c) is another. View-models, the Python package, a generated
//! page and an agent all speak to a `Runtime`, never to the engine or the store directly.

use std::collections::BTreeMap;
use std::path::Path;

use kitchen_contract::{ContentId, OpSpec, ResultRecord, RunRecord, TypeDescription};

use crate::calls::{Answer, Caller, OpsFilter, Request, ResultsFilter, RunState, RuntimeError, Source, Target};

/// What a call gives.
pub type Reply<T> = Result<T, RuntimeError>;

/// Answers the calls of the runtime API. One method, [`Runtime::call`], so any transport can carry
/// it; the others are its typed forms.
pub trait Runtime: Send + Sync {
    /// Answers `request` made by `caller`.
    fn call(&self, caller: &Caller, request: Request) -> Reply<Answer>;

    /// Every type.
    fn types(&self, caller: &Caller) -> Reply<Vec<TypeDescription>> {
        match self.call(caller, Request::Types)? {
            Answer::Types { types } => Ok(types),
            other => Err(unexpected("types", &other)),
        }
    }

    /// The ops `filter` keeps.
    fn ops(&self, caller: &Caller, filter: OpsFilter) -> Reply<Vec<OpSpec>> {
        match self.call(caller, Request::Ops { filter })? {
            Answer::Ops { ops } => Ok(ops),
            other => Err(unexpected("ops", &other)),
        }
    }

    /// Runs `target` once with `filling`; the run's id.
    fn run(&self, caller: &Caller, target: Target, filling: BTreeMap<String, Source>) -> Reply<String> {
        match self.call(caller, Request::Run { target, fillings: vec![filling], options: BTreeMap::new() })? {
            Answer::Started { runs, .. } => runs.into_iter().next().ok_or_else(|| RuntimeError::Invalid("no run started".into())),
            other => Err(unexpected("run", &other)),
        }
    }

    /// Where run `run` is.
    fn watch(&self, caller: &Caller, run: &str) -> Reply<RunState> {
        match self.call(caller, Request::RunWatch { run: run.into() })? {
            Answer::Run { state } => Ok(state),
            other => Err(unexpected("run.watch", &other)),
        }
    }

    /// Waits for run `run` to end (blocking: off any UI thread); its record.
    fn wait(&self, caller: &Caller, run: &str) -> Reply<RunRecord> {
        loop {
            if let RunState::Ended { record } = self.watch(caller, run)? {
                return Ok(*record);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Stops run `run`.
    fn cancel(&self, caller: &Caller, run: &str) -> Reply<()> {
        self.call(caller, Request::RunCancel { run: run.into() }).map(|_| ())
    }

    /// The results `filter` keeps.
    fn results(&self, caller: &Caller, filter: ResultsFilter) -> Reply<Vec<ResultRecord>> {
        match self.call(caller, Request::Results { filter })? {
            Answer::Results { results } => Ok(results),
            other => Err(unexpected("results", &other)),
        }
    }

    /// A result's bytes.
    fn read(&self, caller: &Caller, id: &ContentId) -> Reply<Vec<u8>> {
        match self.call(caller, Request::ResultRead { id: id.clone() })? {
            Answer::Bytes { bytes } => Ok(bytes),
            other => Err(unexpected("result.read", &other)),
        }
    }

    /// An array of a result kept as a Zarr folder: (element type, shape, chunk shape).
    fn array(&self, caller: &Caller, id: &ContentId, array: &str) -> Reply<(String, Vec<u64>, Vec<u64>)> {
        match self.call(caller, Request::ResultArray { id: id.clone(), array: array.into() })? {
            Answer::Array { dtype, shape, chunks } => Ok((dtype, shape, chunks)),
            other => Err(unexpected("result.array", &other)),
        }
    }

    /// A region of such an array: (element type, its elements little-endian in C order).
    fn range(&self, caller: &Caller, id: &ContentId, array: &str, start: &[u64], size: &[u64]) -> Reply<(String, Vec<u8>)> {
        let request = Request::ResultRange { id: id.clone(), array: array.into(), start: start.to_vec(), size: size.to_vec(), compression: String::new() };
        match self.call(caller, request)? {
            Answer::Region { dtype, data, .. } => Ok((dtype, data)),
            other => Err(unexpected("result.range", &other)),
        }
    }

    /// Writes a result to `path`, its run record beside it.
    fn export(&self, caller: &Caller, id: &ContentId, path: &Path) -> Reply<()> {
        self.call(caller, Request::ResultExport { id: id.clone(), path: path.to_path_buf() }).map(|_| ())
    }

    /// The runs `id` came from, nearest first.
    fn lineage(&self, caller: &Caller, id: &ContentId) -> Reply<Vec<RunRecord>> {
        match self.call(caller, Request::Lineage { id: id.clone() })? {
            Answer::Lineage { runs } => Ok(runs),
            other => Err(unexpected("lineage", &other)),
        }
    }
}

fn unexpected(call: &str, answer: &Answer) -> RuntimeError {
    RuntimeError::Invalid(format!("{call}: unexpected answer {answer:?}"))
}
