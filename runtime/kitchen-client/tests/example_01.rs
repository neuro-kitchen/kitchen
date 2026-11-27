//! Example 01 of the standard through the client: an op is listed where a screen's offers fit, run
//! (alone and as a batch), kept with its run record, reused when run again identically, traced back
//! and forward, exported; a cancelled run is kept as incomplete; an op with effects is never reused.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kitchen_client::{Caller, Job, Local, OpsFilter, Produced, Request, ResultsFilter, Runner, Runtime, Source, Target};
use kitchen_contract::{Catalog, Format, OpSpec, ParamSpec, ParamValue, PortSpec, RunStatus, Type};

/// A stand-in for an app's engine: `plot` writes a "figure" from its inputs and `bin_ms`; `slow`
/// runs ten pieces, stopping when cancelled; `upload` acts outside the store.
struct Fake;

impl Runner for Fake {
    fn run(&self, job: &Job, cancel: &kitchen_client::Cancel, progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        let Target::Op { op, .. } = &job.target else { return Err("ops only".into()) };
        let mut text = String::new();
        for (name, path) in &job.inputs {
            text.push_str(&format!("{name}={}\n", std::fs::read_to_string(path).map_err(|e| e.to_string())?));
        }
        text.push_str(&format!("{:?}\n", job.params));
        if op == "slow" {
            for i in 0..10 {
                if cancel.is_cancelled() {
                    break;
                }
                progress(i + 1, 10);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let path = job.out_dir.join("out.txt");
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(vec![Produced { name: "figure".into(), path, ty: Type::new("figure"), format: Format::parse("svg").unwrap() }])
    }
}

fn runtime(name: &str) -> (Local, PathBuf) {
    let dir = std::env::temp_dir().join(format!("kitchen-client-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut catalog = Catalog::new(["signal", "events", "figure", "dspk:unit", "file", "table"].map(Type::new));
    let plot = OpSpec::new("script:firing_rate_by_condition", "Firing rate by condition")
        .family("Plots")
        .input(PortSpec::new("unit", "dspk:unit"))
        .input(PortSpec::new("trials", "events"))
        .output(PortSpec::new("figure", "figure"))
        .param(ParamSpec::integer("bin_ms", 500).range(10.0, 10000.0));
    catalog.add(plot).unwrap();
    catalog.add(OpSpec::new("dspk:kilosort", "Kilosort").input(PortSpec::new("signal", "signal")).output(PortSpec::new("figure", "figure"))).unwrap();
    catalog.add(OpSpec::new("slow", "Slow").input(PortSpec::new("signal", "signal")).output(PortSpec::new("figure", "figure"))).unwrap();
    catalog.add(OpSpec::new("upload", "Upload").input(PortSpec::new("signal", "signal")).output(PortSpec::new("figure", "figure")).effects()).unwrap();
    let local = Local::new("dspk", catalog, &dir, Arc::new(Fake), 2).unwrap();
    (local, dir)
}

fn file(dir: &Path, name: &str, text: &str, ty: &str, format: &str) -> Source {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    Source::Path { path, ty: Type::new(ty), format: Format::parse(format).unwrap() }
}

#[test]
fn a_script_op_is_listed_run_kept_reused_and_traced() {
    let (rt, dir) = runtime("example01");
    let ana = Caller::new("ana", "dsp-kitchen");

    // Listed where Analysis's offers fit, and not kilosort
    let offers = OpsFilter { fillable_by: Some(vec![Type::new("dspk:unit"), Type::new("events")]), ..Default::default() };
    let fits: Vec<String> = rt.ops(&ana, offers).unwrap().into_iter().map(|o| o.id).collect();
    assert_eq!(fits, ["script:firing_rate_by_condition"]);

    // Run it: parameters checked, the outputs kept, the record written
    let target = Target::Op { op: "script:firing_rate_by_condition".into(), params: [("bin_ms".to_string(), ParamValue::Integer(250))].into() };
    let filling: BTreeMap<String, Source> = [("unit".into(), file(&dir, "unit17.json", "unit 17", "dspk:unit", "json")), ("trials".into(), file(&dir, "trials.parquet", "trials", "events", "parquet"))].into();
    let run = rt.run(&ana, target.clone(), filling.clone()).unwrap();
    let record = rt.wait(&ana, &run).unwrap();
    assert_eq!(record.status, RunStatus::Succeeded);
    assert_eq!((record.by.as_str(), record.via.as_str()), ("ana", "dsp-kitchen"));
    assert_eq!(record.params["bin_ms"], ParamValue::Integer(250));
    let figure = record.outputs["figure"].clone();
    assert!(String::from_utf8(rt.read(&ana, &figure).unwrap()).unwrap().contains("unit=unit 17"));

    // The same run again: recognized by its run key, its outputs reused, a record of its own
    let again = rt.wait(&ana, &rt.run(&Caller::new("ben", "page"), target.clone(), filling.clone()).unwrap()).unwrap();
    assert_eq!(again.outputs["figure"], figure);
    assert_eq!(again.metrics["reused"], serde_json::json!(run));
    assert_eq!(again.by, "ben");

    // Bad parameters are refused with why
    let bad = Target::Op { op: "script:firing_rate_by_condition".into(), params: [("bin_ms".to_string(), ParamValue::Integer(1))].into() };
    assert!(rt.run(&ana, bad, filling.clone()).unwrap_err().to_string().contains("at least 10"));
    // Another app's op is not here
    let other = Target::Op { op: "lucem:watershed".into(), params: BTreeMap::new() };
    assert!(rt.run(&ana, other, filling.clone()).unwrap_err().to_string().contains("chain"));

    // Traced back and forward, listed, exported
    let unit_id = record.inputs["unit"].clone();
    assert_eq!(rt.lineage(&ana, &figure).unwrap()[0].run, run);
    let made = rt.results(&ana, ResultsFilter { from: Some(unit_id), ..Default::default() }).unwrap();
    assert_eq!(made.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), std::slice::from_ref(&figure), "what was made from unit 17");
    let figures = rt.results(&ana, ResultsFilter { ty: Some(Type::new("figure")), ..Default::default() }).unwrap();
    assert_eq!(figures.len(), 1);
    let dest = dir.join("figure3.svg");
    rt.export(&ana, &figure, &dest).unwrap();
    assert!(dir.join("figure3.svg.run.json").exists());
}

#[test]
fn batches_cancel_and_effects() {
    let (rt, dir) = runtime("batches");
    let me = Caller::new("ana", "test");
    // A batch: one run per filling
    let fillings: Vec<BTreeMap<String, Source>> = (0..3).map(|i| [("signal".to_string(), file(&dir, &format!("s{i}.bin"), &format!("session {i}"), "signal", "raw"))].into()).collect();
    let answer = rt.call(&me, Request::Run { target: Target::Op { op: "dspk:kilosort".into(), params: BTreeMap::new() }, fillings, options: BTreeMap::new() }).unwrap();
    let kitchen_client::Answer::Started { batch, runs } = answer else { panic!("{answer:?}") };
    assert_eq!(runs.len(), 3);
    assert!(batch.starts_with("batch-"));
    for r in &runs {
        assert_eq!(rt.wait(&me, r).unwrap().status, RunStatus::Succeeded);
    }

    // Cancelled between pieces: its output kept, marked incomplete, never reused
    let one: BTreeMap<String, Source> = [("signal".to_string(), file(&dir, "long.bin", "long", "signal", "raw"))].into();
    let slow = Target::Op { op: "slow".into(), params: BTreeMap::new() };
    let run = rt.run(&me, slow.clone(), one.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    rt.cancel(&me, &run).unwrap();
    let record = rt.wait(&me, &run).unwrap();
    assert_eq!(record.status, RunStatus::Cancelled);
    let kept = rt.results(&me, ResultsFilter { run: Some(run.clone()), ..Default::default() }).unwrap();
    assert!(kept[0].incomplete);
    let again = rt.wait(&me, &rt.run(&me, slow, one.clone()).unwrap()).unwrap();
    assert!(!again.metrics.contains_key("reused"), "a cancelled run is never reused");

    // An op with effects runs every time
    let upload = Target::Op { op: "upload".into(), params: BTreeMap::new() };
    let first = rt.wait(&me, &rt.run(&me, upload.clone(), one.clone()).unwrap()).unwrap();
    let second = rt.wait(&me, &rt.run(&me, upload, one).unwrap()).unwrap();
    assert!(!second.metrics.contains_key("reused") && first.run != second.run);
}
