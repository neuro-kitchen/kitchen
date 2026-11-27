//! The calls of the runtime API ([standard §6.1](../../../standard/spec/06-runtime-api.md)) as data:
//! a [`Request`] and its [`Answer`], made by a [`Caller`]. Every transport carries these same
//! messages (in-process as they are, on the wire as protobuf: step 4b), as dspk-stream's `Request`
//! and `Response` are carried over QUIC, a WebSocket or a worker's messages alike.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitchen_contract::{ContentId, Margin, OpSpec, ParamValue, Pipeline, ResultRecord, RunRecord, Type, TypeDescription};
use serde::{Deserialize, Serialize};

/// Who is calling (`by`, a person) and through what (`via`: an app, a page, an agent). Recorded on
/// every run ([standard §6.3](../../../standard/spec/06-runtime-api.md)).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caller {
    /// The person.
    pub by: String,
    /// What they called through.
    pub via: String,
}

impl Caller {
    /// `by` through `via`.
    pub fn new(by: impl Into<String>, via: impl Into<String>) -> Self {
        Self { by: by.into(), via: via.into() }
    }
}

/// Which ops to list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpsFilter {
    /// Only ops with an input that accepts this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Type>,
    /// Only ops whose required inputs a screen's offers can all fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fillable_by: Option<Vec<Type>>,
    /// Only this family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Only ops whose title, id, family or summary contain every word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

/// Which results to list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultsFilter {
    /// Only results of this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<Type>,
    /// Only results made by this op.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    /// Only results of this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Only results made from this one: everything whose lineage reaches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<ContentId>,
}

/// What fills one source of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Source {
    /// A result the store keeps.
    Result {
        /// Its id.
        id: ContentId,
    },
    /// A file or folder the app reads (a recording, a slide): kept as a source, by reference.
    Path {
        /// Where it is.
        path: PathBuf,
        /// Its type.
        #[serde(rename = "type")]
        ty: Type,
        /// Its format.
        format: kitchen_contract::Format,
    },
}

/// What a run runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// One op, with its parameters (what is left out takes the default).
    Op {
        /// Its id.
        op: String,
        /// Its parameters.
        #[serde(default)]
        params: BTreeMap<String, ParamValue>,
    },
    /// A pipeline.
    Pipeline {
        /// The pipeline.
        pipeline: Pipeline,
    },
}

/// How a run is going.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunState {
    /// Waiting for a worker.
    Queued,
    /// Running: pieces done of pieces.
    Running {
        /// Pieces done.
        done: usize,
        /// Pieces in all.
        pieces: usize,
    },
    /// Ended: its record.
    Ended {
        /// The run's record (its status says how it ended).
        record: Box<RunRecord>,
    },
}

/// A call to the runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum Request {
    /// The caller's contract version and capabilities.
    Hello {
        /// The contract version it speaks.
        contract: String,
        /// What it can do (`recipes`, `preview`, …).
        #[serde(default)]
        capabilities: Vec<String>,
    },
    /// Every type: shared and the app's.
    Types,
    /// Op descriptions.
    Ops {
        /// Which.
        #[serde(default)]
        filter: OpsFilter,
    },
    /// Checks a pipeline: its problems, or its plan.
    PipelineCheck {
        /// The pipeline.
        pipeline: Pipeline,
    },
    /// Runs an op or a pipeline once per filling of its sources (one filling: one run).
    Run {
        /// What runs.
        target: Target,
        /// Source name → what fills it; one map per run (a batch when there are several).
        fillings: Vec<BTreeMap<String, Source>>,
        /// Run options (device, piece size, threads): recorded, never in the run key.
        #[serde(default)]
        options: BTreeMap<String, serde_json::Value>,
    },
    /// Where a run is.
    RunWatch {
        /// The run.
        run: String,
    },
    /// Stops a run (between pieces).
    RunCancel {
        /// The run.
        run: String,
    },
    /// Results kept.
    Results {
        /// Which.
        #[serde(default)]
        filter: ResultsFilter,
    },
    /// A result's bytes (a file; a folder is read by range: step 4e).
    ResultRead {
        /// The result.
        id: ContentId,
    },
    /// An array of a result kept as a Zarr folder: its shape, element type and chunk shape.
    ResultArray {
        /// The result.
        id: ContentId,
        /// The array's path inside the folder (`0` for an OME-Zarr image's full resolution; `""`
        /// for an array at the root).
        #[serde(default)]
        array: String,
    },
    /// A region of such an array: what a viewer asks for a tile or a window.
    ResultRange {
        /// The result.
        id: ContentId,
        /// The array's path inside the folder.
        #[serde(default)]
        array: String,
        /// The region's first element along each dimension.
        start: Vec<u64>,
        /// Its size along each dimension.
        size: Vec<u64>,
        /// How the bytes travel on the wire (`""` or `none`, `lz4`, `zstd`); in-process, unused.
        #[serde(default)]
        compression: String,
    },
    /// Writes a result where a person asks, its run record beside it.
    ResultExport {
        /// The result.
        id: ContentId,
        /// Where.
        path: PathBuf,
    },
    /// The runs a result came from, back to its sources.
    Lineage {
        /// The result.
        id: ContentId,
    },
}

/// The answer to a [`Request`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum Answer {
    /// To `hello`.
    Welcome {
        /// The runtime's contract version.
        contract: String,
        /// What it can do.
        capabilities: Vec<String>,
        /// The apps (domains) it hosts.
        apps: Vec<String>,
    },
    /// To `types`.
    Types {
        /// Every type.
        types: Vec<TypeDescription>,
    },
    /// To `ops`.
    Ops {
        /// The ops.
        ops: Vec<OpSpec>,
    },
    /// To `pipeline.check`: the order and the margin, or every problem.
    Checked {
        /// The nodes in the order they run (empty when there are problems).
        order: Vec<String>,
        /// The margin the sources are read with.
        margin: Margin,
        /// Every problem (empty when it can run).
        problems: Vec<String>,
    },
    /// To `run`: one run id per filling, under a batch id when there are several.
    Started {
        /// The batch (the first run's id when there is one filling).
        batch: String,
        /// The runs, in the fillings' order.
        runs: Vec<String>,
    },
    /// To `run.watch`.
    Run {
        /// Where it is.
        state: RunState,
    },
    /// To `results`.
    Results {
        /// The results.
        results: Vec<ResultRecord>,
    },
    /// To `result.read`.
    Bytes {
        /// The result's bytes.
        bytes: Vec<u8>,
    },
    /// To `result.array`.
    Array {
        /// The element type (`u8`, `u16`, `f32`, …).
        dtype: String,
        /// Elements along each dimension.
        shape: Vec<u64>,
        /// A chunk's shape: regions of whole chunks read fastest.
        chunks: Vec<u64>,
    },
    /// To `result.range`: the region's elements, little-endian, in C order.
    Region {
        /// The element type.
        dtype: String,
        /// Where the region starts.
        start: Vec<u64>,
        /// Its size along each dimension.
        size: Vec<u64>,
        /// The elements.
        data: Vec<u8>,
        /// How they travel on the wire (as asked).
        #[serde(default)]
        compression: String,
    },
    /// To `lineage`.
    Lineage {
        /// The runs, nearest first.
        runs: Vec<RunRecord>,
    },
    /// Done, nothing to say (`run.cancel`, `result.export`).
    Done,
}

/// Why a call failed, said so a person (or an agent) can act on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum RuntimeError {
    /// Something named does not exist (an op, a run, a result).
    #[error("{0}")]
    NotFound(String),
    /// The request is not one the runtime can do as asked (bad parameters, an unchecked script).
    #[error("{0}")]
    Invalid(String),
    /// The runtime cannot do this call here (yet).
    #[error("{0}")]
    Unsupported(String),
    /// The caller may not do this (a read-only peer asking to run or export).
    #[error("{0}")]
    Forbidden(String),
    /// Reading or writing failed.
    #[error("{0}")]
    Io(String),
}
