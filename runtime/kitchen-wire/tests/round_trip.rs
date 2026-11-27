//! Every call and answer survives the wire unchanged; frames refuse what is too large; chunks
//! come back as they went.

use std::collections::BTreeMap;

use kitchen_client::{Answer, Caller, OpsFilter, Request, ResultsFilter, RunState, RuntimeError, Source, Target};
use kitchen_contract::{CONTRACT_VERSION, ContentId, Format, Margin, OpRef, OpSpec, ParamValue, Pipeline, PortSpec, ResultRecord, RunKey, RunRecord, RunStatus, Type, types};
use kitchen_wire::chunk::{self, Compression};
use kitchen_wire::{MAX_FRAME, decode_answer, decode_request, encode_answer, encode_request, parse_answer, parse_request, pb, read_frame, write_frame};
use prost::Message;

fn id(c: char) -> ContentId {
    ContentId::parse(&format!("sha256:{}", c.to_string().repeat(64))).unwrap()
}

fn record() -> RunRecord {
    let params: BTreeMap<String, ParamValue> = [("bin_ms".to_string(), ParamValue::Integer(250))].into();
    let inputs: BTreeMap<String, ContentId> = [("unit".to_string(), id('a'))].into();
    RunRecord {
        contract: CONTRACT_VERSION.into(),
        run: "run-1".into(),
        op: OpRef { id: "script:plot".into(), version: "sha256:ab".into() },
        run_key: RunKey::of("script:plot", "sha256:ab", &params, &inputs),
        params,
        inputs,
        outputs: [("figure".to_string(), id('b'))].into(),
        by: "ana".into(),
        via: "page".into(),
        started: "2026-11-27T10:00:00Z".into(),
        ended: Some("2026-11-27T10:00:01Z".into()),
        status: RunStatus::Failed { why: "out of memory".into() },
        environment: [("device".to_string(), serde_json::json!("cuda:0"))].into(),
        metrics: BTreeMap::new(),
    }
}

#[test]
fn every_call_round_trips() {
    let caller = Caller::new("ana", "dsp-kitchen");
    let pipeline = Pipeline::from_json(r#"{"sources":{"a":"image"},"nodes":{"b":{"op":"blur","params":{"sigma":2.0}}},"edges":[["a","b.image"]],"outputs":{"o":"b.image"}}"#).unwrap();
    let filling: BTreeMap<String, Source> = [
        ("unit".to_string(), Source::Result { id: id('c') }),
        ("trials".to_string(), Source::Path { path: "/data/trials.parquet".into(), ty: Type::new("events"), format: Format::parse("parquet").unwrap() }),
    ]
    .into();
    let calls = vec![
        Request::Hello { contract: "0.1".into(), capabilities: vec!["recipes".into()] },
        Request::Types,
        Request::Ops { filter: OpsFilter::default() },
        Request::Ops { filter: OpsFilter { input: Some(Type::new("signal")), fillable_by: Some(vec![]), family: Some("Plots".into()), search: Some("rate".into()) } },
        Request::Ops { filter: OpsFilter { fillable_by: Some(vec![Type::new("dspk:unit"), Type::new("events")]), ..Default::default() } },
        Request::PipelineCheck { pipeline: pipeline.clone() },
        Request::Run { target: Target::Op { op: "script:plot".into(), params: [("bin_ms".to_string(), ParamValue::Integer(250))].into() }, fillings: vec![filling.clone(), BTreeMap::new()], options: [("device".to_string(), serde_json::json!("cpu"))].into() },
        Request::Run { target: Target::Pipeline { pipeline }, fillings: vec![filling], options: BTreeMap::new() },
        Request::RunWatch { run: "run-1".into() },
        Request::RunCancel { run: "run-1".into() },
        Request::Results { filter: ResultsFilter::default() },
        Request::Results { filter: ResultsFilter { ty: Some(Type::new("figure")), op: Some("x".into()), run: Some("run-1".into()), from: Some(id('d')) } },
        Request::ResultRead { id: id('e') },
        Request::ResultExport { id: id('e'), path: "/out/figure3.svg".into() },
        Request::Lineage { id: id('e') },
    ];
    for call in calls {
        let bytes = encode_request(&caller, &call).encode_to_vec();
        let (c, back) = parse_request(&bytes).unwrap();
        assert_eq!((c, back), (caller.clone(), call.clone()), "{call:?}");
        assert_eq!(decode_request(pb::Request::decode(bytes.as_slice()).unwrap()).unwrap().1, call);
    }
    // A call without a caller is refused
    assert!(decode_request(pb::Request { caller: None, call: Some(pb::request::Call::Types(pb::Types {})) }).is_err());
}

#[test]
fn every_answer_round_trips() {
    let op = OpSpec::new("blur", "Blur").input(PortSpec::new("image", "image")).output(PortSpec::new("image", "image")).margin(Margin::around(6, 2));
    let result = ResultRecord { id: id('a'), ty: Type::new("table"), format: Format::parse("parquet").unwrap(), size: 42, run: Some("run-1".into()), incomplete: true };
    let answers: Vec<Result<Answer, RuntimeError>> = vec![
        Ok(Answer::Welcome { contract: "0.1".into(), capabilities: vec!["batches".into()], apps: vec!["dspk".into()] }),
        Ok(Answer::Types { types: types::shared() }),
        Ok(Answer::Ops { ops: vec![op] }),
        Ok(Answer::Checked { order: vec!["b".into()], margin: Margin::time(3, 4), problems: vec![] }),
        Ok(Answer::Checked { order: vec![], margin: Margin::NONE, problems: vec!["b: no operation".into()] }),
        Ok(Answer::Started { batch: "batch-run-1".into(), runs: vec!["run-1".into(), "run-2".into()] }),
        Ok(Answer::Run { state: RunState::Queued }),
        Ok(Answer::Run { state: RunState::Running { done: 3, pieces: 10 } }),
        Ok(Answer::Run { state: RunState::Ended { record: Box::new(record()) } }),
        Ok(Answer::Results { results: vec![result] }),
        Ok(Answer::Bytes { bytes: vec![0, 1, 2, 255] }),
        Ok(Answer::Lineage { runs: vec![record()] }),
        Ok(Answer::Done),
        Err(RuntimeError::NotFound("no run x".into())),
        Err(RuntimeError::Invalid("bin_ms is at least 10".into())),
        Err(RuntimeError::Unsupported("read by range".into())),
        Err(RuntimeError::Io("disk full".into())),
        Err(RuntimeError::Forbidden("read only".into())),
    ];
    for answer in answers {
        let bytes = encode_answer(&answer).encode_to_vec();
        assert_eq!(parse_answer(&bytes).unwrap(), answer);
        assert_eq!(decode_answer(pb::Answer::decode(bytes.as_slice()).unwrap()).unwrap(), answer);
    }
}

#[test]
fn frames_are_length_prefixed_and_bounded() {
    let mut stream = Vec::new();
    let a = encode_request(&Caller::new("a", "b"), &Request::Types);
    let b = encode_answer(&Ok(Answer::Done));
    write_frame(&mut stream, &a).unwrap();
    write_frame(&mut stream, &b).unwrap();
    let mut r = stream.as_slice();
    assert_eq!(read_frame(&mut r, MAX_FRAME).unwrap().unwrap(), a.encode_to_vec());
    assert_eq!(read_frame(&mut r, MAX_FRAME).unwrap().unwrap(), b.encode_to_vec());
    assert_eq!(read_frame(&mut r, MAX_FRAME).unwrap(), None, "a clean end");
    // A length above the limit is refused before anything is allocated
    let huge = [0xff, 0xff, 0xff, 0xff];
    let e = read_frame(&mut huge.as_slice(), 1024).unwrap_err();
    assert!(e.to_string().contains("above the limit"));
}

#[test]
fn chunks_come_back_as_they_went() {
    let samples: Vec<u8> = (0..4096u32).flat_map(|i| ((i % 64) as f32).to_le_bytes()).collect();
    for compression in [Compression::None, Compression::Lz4, Compression::Zstd] {
        let region = pb::Region { start: vec![0, 0, 1024], size: vec![1, 4, 1024] };
        let c = chunk::array("view-1", 7, "f32", &[4, 1024], Some(region.clone()), 2, &samples, compression, false);
        let back = pb::Chunk::decode(c.encode_to_vec().as_slice()).unwrap();
        assert_eq!(chunk::bytes(&back).unwrap(), samples, "{compression:?}");
        assert_eq!((back.region, back.level, back.dtype.as_str()), (Some(region), 2, "f32"));
        if compression != Compression::None {
            assert!(back.data.len() < samples.len() / 2, "{compression:?} shrinks repetitive samples");
        }
    }
    let t = chunk::table("results", 0, b"ARROW1 batch", Compression::Lz4, true);
    assert_eq!(chunk::bytes(&t).unwrap(), b"ARROW1 batch");
    assert!(t.last && t.encoding == "arrow-ipc");
    // A shape that does not match the bytes is refused
    let wrong = chunk::array("v", 0, "f32", &[3], None, 0, &[0u8; 8], Compression::None, true);
    assert!(chunk::bytes(&wrong).is_err());
}
