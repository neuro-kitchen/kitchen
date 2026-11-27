//! The runtime API on the wire (decision 0017): `contract/proto/kitchen/runtime/v1/runtime.proto`'s
//! messages ([`pb`]), conversions between them and the client's calls ([`encode_request`],
//! [`decode_request`], [`encode_answer`], [`decode_answer`]), the framing every stream uses
//! ([`write_frame`], [`read_frame`]: a 4-byte big-endian length, then the message), and data chunks
//! ([`chunk`]: raw arrays and Arrow IPC tables, compressed with lz4 or zstd).
//!
//! Documents travel as their JSON inside `bytes` fields (dsp-kitchen's convention), so the contract
//! crate stays their one definition and content ids stay hashes of canonical JSON.

pub mod chunk;

use std::collections::BTreeMap;
use std::io::{Read, Write};

use kitchen_client::{Answer, Caller, OpsFilter, Request, ResultsFilter, RunState, RuntimeError, Source, Target};
use kitchen_contract::{ContentId, Format, Type};
use prost::Message;

/// The messages generated from `runtime.proto` (package `kitchen.runtime.v1`).
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/kitchen.runtime.v1.rs"));
}

/// `runtime.proto`'s descriptor set (what the breaking-change test compares).
pub const DESCRIPTOR_SET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.fds"));

/// The largest message a receiver accepts by default (64 MiB): data comes in chunks well below it.
pub const MAX_FRAME: usize = 64 << 20;

/// Why bytes could not be read as a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireError(pub String);

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for WireError {}

fn err(e: impl std::fmt::Display) -> WireError {
    WireError(e.to_string())
}

fn json<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

fn from_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, WireError> {
    serde_json::from_slice(bytes).map_err(err)
}

fn id(text: &str) -> Result<ContentId, WireError> {
    ContentId::parse(text).map_err(WireError)
}

/// Writes `message` as one frame: its length (4 bytes, big-endian), then its encoding.
pub fn write_frame(w: &mut impl Write, message: &impl Message) -> std::io::Result<()> {
    let bytes = message.encode_to_vec();
    let len = u32::try_from(bytes.len()).map_err(|_| std::io::Error::other("a message over 4 GiB"))?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&bytes)
}

/// Reads one frame's bytes; a length above `max` is refused before anything is allocated.
/// `Ok(None)` at a clean end of the stream.
pub fn read_frame(r: &mut impl Read, max: usize) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > max {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("a frame of {len} bytes, above the limit of {max}")));
    }
    let mut bytes = vec![0u8; len];
    r.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

/// A call as its wire message.
pub fn encode_request(caller: &Caller, request: &Request) -> pb::Request {
    use pb::request::Call;
    let call = match request {
        Request::Hello { contract, capabilities } => Call::Hello(pb::Hello { contract: contract.clone(), capabilities: capabilities.clone() }),
        Request::Types => Call::Types(pb::Types {}),
        Request::Ops { filter } => Call::Ops(pb::OpsFilter {
            input: filter.input.as_ref().map(|t| t.0.clone()),
            fillable_by: filter.fillable_by.iter().flatten().map(|t| t.0.clone()).collect(),
            by_offers: filter.fillable_by.is_some(),
            family: filter.family.clone(),
            search: filter.search.clone(),
        }),
        Request::PipelineCheck { pipeline } => Call::PipelineCheck(pb::PipelineCheck { pipeline: json(pipeline) }),
        Request::Run { target, fillings, options } => Call::Run(pb::Run {
            target: json(target),
            fillings: fillings.iter().map(|f| pb::Filling { sources: f.iter().map(|(k, s)| (k.clone(), source(s))).collect() }).collect(),
            options: json(options),
        }),
        Request::RunWatch { run } => Call::RunWatch(pb::RunWatch { run: run.clone() }),
        Request::RunCancel { run } => Call::RunCancel(pb::RunCancel { run: run.clone() }),
        Request::Results { filter } => Call::Results(pb::ResultsFilter {
            r#type: filter.ty.as_ref().map(|t| t.0.clone()),
            op: filter.op.clone(),
            run: filter.run.clone(),
            from: filter.from.as_ref().map(|i| i.as_str().to_string()),
        }),
        Request::ResultRead { id } => Call::ResultRead(pb::ResultRead { id: id.as_str().into() }),
        Request::ResultArray { id, array } => Call::ResultArray(pb::ResultArray { id: id.as_str().into(), array: array.clone() }),
        Request::ResultRange { id, array, start, size, compression } => Call::ResultRange(pb::ResultRange {
            id: id.as_str().into(),
            array: array.clone(),
            region: Some(pb::Region { start: start.clone(), size: size.clone() }),
            compression: compression.clone(),
        }),
        Request::ResultExport { id, path } => Call::ResultExport(pb::ResultExport { id: id.as_str().into(), path: path.to_string_lossy().into_owned() }),
        Request::Lineage { id } => Call::Lineage(pb::Lineage { id: id.as_str().into() }),
    };
    pb::Request { caller: Some(pb::Caller { by: caller.by.clone(), via: caller.via.clone() }), call: Some(call) }
}

fn source(s: &Source) -> pb::Source {
    use pb::source::By;
    pb::Source {
        by: Some(match s {
            Source::Result { id } => By::Result(id.as_str().into()),
            Source::Path { path, ty, format } => By::Path(pb::PathSource { path: path.to_string_lossy().into_owned(), r#type: ty.0.clone(), format: format.0.clone() }),
        }),
    }
}

/// A wire message as the call it carries.
pub fn decode_request(message: pb::Request) -> Result<(Caller, Request), WireError> {
    use pb::request::Call;
    let caller = message.caller.map(|c| Caller { by: c.by, via: c.via }).ok_or_else(|| err("a call names who makes it"))?;
    let request = match message.call.ok_or_else(|| err("an empty call"))? {
        Call::Hello(h) => Request::Hello { contract: h.contract, capabilities: h.capabilities },
        Call::Types(_) => Request::Types,
        Call::Ops(f) => Request::Ops {
            filter: OpsFilter {
                input: f.input.map(Type),
                fillable_by: f.by_offers.then(|| f.fillable_by.into_iter().map(Type).collect()),
                family: f.family,
                search: f.search,
            },
        },
        Call::PipelineCheck(p) => Request::PipelineCheck { pipeline: from_json(&p.pipeline)? },
        Call::Run(r) => {
            let mut fillings = Vec::new();
            for f in r.fillings {
                let mut filling = BTreeMap::new();
                for (name, s) in f.sources {
                    let s = match s.by.ok_or_else(|| err(format!("source {name} is empty")))? {
                        pb::source::By::Result(i) => Source::Result { id: id(&i)? },
                        pb::source::By::Path(p) => Source::Path { path: p.path.into(), ty: Type(p.r#type), format: Format::parse(&p.format).map_err(WireError)? },
                    };
                    filling.insert(name, s);
                }
                fillings.push(filling);
            }
            let target: Target = from_json(&r.target)?;
            let options = if r.options.is_empty() { BTreeMap::new() } else { from_json(&r.options)? };
            Request::Run { target, fillings, options }
        }
        Call::RunWatch(r) => Request::RunWatch { run: r.run },
        Call::RunCancel(r) => Request::RunCancel { run: r.run },
        Call::Results(f) => Request::Results {
            filter: ResultsFilter { ty: f.r#type.map(Type), op: f.op, run: f.run, from: f.from.map(|i| id(&i)).transpose()? },
        },
        Call::ResultRead(r) => Request::ResultRead { id: id(&r.id)? },
        Call::ResultArray(r) => Request::ResultArray { id: id(&r.id)?, array: r.array },
        Call::ResultRange(r) => {
            let region = r.region.unwrap_or_default();
            Request::ResultRange { id: id(&r.id)?, array: r.array, start: region.start, size: region.size, compression: r.compression }
        }
        Call::ResultExport(r) => Request::ResultExport { id: id(&r.id)?, path: r.path.into() },
        Call::Lineage(r) => Request::Lineage { id: id(&r.id)? },
    };
    Ok((caller, request))
}

fn docs<T: serde::Serialize>(items: &[T]) -> pb::Documents {
    pb::Documents { items: items.iter().map(json).collect() }
}

fn undocs<T: serde::de::DeserializeOwned>(d: pb::Documents) -> Result<Vec<T>, WireError> {
    d.items.iter().map(|b| from_json(b)).collect()
}

/// An answer (or why the call failed) as its wire message.
pub fn encode_answer(answer: &Result<Answer, RuntimeError>) -> pb::Answer {
    use pb::answer::Answer as A;
    let a = match answer {
        Err(e) => {
            let (kind, message) = match e {
                RuntimeError::NotFound(m) => ("not_found", m),
                RuntimeError::Invalid(m) => ("invalid", m),
                RuntimeError::Unsupported(m) => ("unsupported", m),
                RuntimeError::Forbidden(m) => ("forbidden", m),
                RuntimeError::Io(m) => ("io", m),
            };
            A::Error(pb::Error { kind: kind.into(), message: message.clone() })
        }
        Ok(Answer::Welcome { contract, capabilities, apps }) => A::Welcome(pb::Welcome { contract: contract.clone(), capabilities: capabilities.clone(), apps: apps.clone() }),
        Ok(Answer::Types { types }) => A::Types(docs(types)),
        Ok(Answer::Ops { ops }) => A::Ops(docs(ops)),
        Ok(Answer::Checked { order, margin, problems }) => A::Checked(pb::Checked { order: order.clone(), margin: json(margin), problems: problems.clone() }),
        Ok(Answer::Started { batch, runs }) => A::Started(pb::Started { batch: batch.clone(), runs: runs.clone() }),
        Ok(Answer::Run { state }) => A::Run(pb::RunState {
            state: Some(match state {
                RunState::Queued => pb::run_state::State::Queued(pb::Queued {}),
                RunState::Running { done, pieces } => pb::run_state::State::Running(pb::Running { done: *done as u64, pieces: *pieces as u64 }),
                RunState::Ended { record } => pb::run_state::State::Ended(json(record)),
            }),
        }),
        Ok(Answer::Results { results }) => A::Results(docs(results)),
        Ok(Answer::Bytes { bytes }) => A::Bytes(bytes.clone()),
        Ok(Answer::Lineage { runs }) => A::Lineage(docs(runs)),
        Ok(Answer::Done) => A::Done(pb::Done {}),
        Ok(Answer::Array { dtype, shape, chunks }) => A::Array(pb::ArrayInfo { dtype: dtype.clone(), shape: shape.clone(), chunks: chunks.clone() }),
        Ok(Answer::Region { dtype, start, size, data, compression }) => {
            // An unknown compression is sent as is: the caller asked for something we do not do
            let compression = chunk::Compression::parse(compression).unwrap_or(chunk::Compression::None);
            let region = pb::Region { start: start.clone(), size: size.clone() };
            A::Region(chunk::array("range", 0, dtype, size, Some(region), 0, data, compression, true))
        }
    };
    pb::Answer { answer: Some(a) }
}

/// A wire message as the answer it carries (or why the call failed).
pub fn decode_answer(message: pb::Answer) -> Result<Result<Answer, RuntimeError>, WireError> {
    use pb::answer::Answer as A;
    Ok(Ok(match message.answer.ok_or_else(|| err("an empty answer"))? {
        A::Error(e) => {
            return Ok(Err(match e.kind.as_str() {
                "not_found" => RuntimeError::NotFound(e.message),
                "invalid" => RuntimeError::Invalid(e.message),
                "unsupported" => RuntimeError::Unsupported(e.message),
                "forbidden" => RuntimeError::Forbidden(e.message),
                _ => RuntimeError::Io(e.message),
            }));
        }
        A::Welcome(w) => Answer::Welcome { contract: w.contract, capabilities: w.capabilities, apps: w.apps },
        A::Types(d) => Answer::Types { types: undocs(d)? },
        A::Ops(d) => Answer::Ops { ops: undocs(d)? },
        A::Checked(c) => Answer::Checked { order: c.order, margin: from_json(&c.margin)?, problems: c.problems },
        A::Started(s) => Answer::Started { batch: s.batch, runs: s.runs },
        A::Run(r) => Answer::Run {
            state: match r.state.ok_or_else(|| err("an empty run state"))? {
                pb::run_state::State::Queued(_) => RunState::Queued,
                pb::run_state::State::Running(x) => RunState::Running { done: x.done as usize, pieces: x.pieces as usize },
                pb::run_state::State::Ended(b) => RunState::Ended { record: Box::new(from_json(&b)?) },
            },
        },
        A::Results(d) => Answer::Results { results: undocs(d)? },
        A::Bytes(b) => Answer::Bytes { bytes: b },
        A::Lineage(d) => Answer::Lineage { runs: undocs(d)? },
        A::Done(_) => Answer::Done,
        A::Array(a) => Answer::Array { dtype: a.dtype, shape: a.shape, chunks: a.chunks },
        A::Region(c) => {
            let data = chunk::bytes(&c)?;
            let region = c.region.unwrap_or_default();
            Answer::Region { dtype: c.dtype, start: region.start, size: region.size, data, compression: c.compression }
        }
    }))
}

/// Reads a [`pb::Request`] frame's bytes.
pub fn parse_request(bytes: &[u8]) -> Result<(Caller, Request), WireError> {
    decode_request(pb::Request::decode(bytes).map_err(err)?)
}

/// Reads a [`pb::Answer`] frame's bytes.
pub fn parse_answer(bytes: &[u8]) -> Result<Result<Answer, RuntimeError>, WireError> {
    decode_answer(pb::Answer::decode(bytes).map_err(err)?)
}
