//! Example 01 of the standard through a server: a program pairs with a code (then connects with its
//! token), lists, runs, reads, traces and exports over WebTransport, as it would in-process; the
//! server records the person it let in, not who the caller says it is; a read-only peer reads but
//! does not run; an unknown token and a wrong code are not let in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kitchen_client::{Caller, Job, Local, OpsFilter, Produced, Runner, Runtime, RuntimeError, Source, Target};
use kitchen_contract::{Catalog, Format, OpSpec, ParamSpec, ParamValue, PortSpec, RunStatus, Type};
use kitchen_network::auth::Authority;
use kitchen_network::pair::Permission;
use kitchen_server::{Remote, RemoteOptions, ServeInfo, Server, ServerError, ServerOptions};

/// A stand-in for an app's engine: writes a "figure" from its inputs and parameters.
struct Fake;

impl Runner for Fake {
    fn run(&self, job: &Job, _cancel: &kitchen_client::Cancel, progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        let mut text = String::new();
        for (name, path) in &job.inputs {
            text.push_str(&format!("{name}={}\n", std::fs::read_to_string(path).map_err(|e| e.to_string())?));
        }
        text.push_str(&format!("{:?}\n", job.params));
        progress(1, 1);
        let path = job.out_dir.join("out.txt");
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(vec![Produced { name: "figure".into(), path, ty: Type::new("figure"), format: Format::parse("svg").unwrap() }])
    }
}

struct Served {
    dir: PathBuf,
    authority: Arc<Authority>,
    options: RemoteOptions,
}

/// A runtime with example 01's op, served on a free loopback port from a thread of its own.
fn serve(name: &str) -> Served {
    let dir = std::env::temp_dir().join(format!("kitchen-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut catalog = Catalog::new(["events", "figure", "dspk:unit"].map(Type::new));
    let plot = OpSpec::new("script:firing_rate_by_condition", "Firing rate by condition")
        .family("Plots")
        .input(PortSpec::new("unit", "dspk:unit"))
        .input(PortSpec::new("trials", "events"))
        .output(PortSpec::new("figure", "figure"))
        .param(ParamSpec::integer("bin_ms", 500).range(10.0, 10000.0));
    catalog.add(plot).unwrap();
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("dspk", catalog, &dir, Arc::new(Fake), 2).unwrap());
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let (tx, rx) = std::sync::mpsc::channel::<(String, ServeInfo)>();
    let a = authority.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let server = Server::bind(ServerOptions::loopback("test server"), a, runtime).unwrap();
            tx.send((server.url("127.0.0.1").unwrap(), server.serve_info().clone())).unwrap();
            server.run().await;
        });
    });
    let (url, info) = rx.recv().unwrap();
    assert_eq!(info.certificate_sha256.len(), 32);
    Served { dir, authority, options: RemoteOptions { url, certificate_sha256: info.certificate_sha256 } }
}

fn file(dir: &Path, name: &str, text: &str, ty: &str, format: &str) -> Source {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    Source::Path { path, ty: Type::new(ty), format: Format::parse(format).unwrap() }
}

#[test]
fn example_01_runs_over_webtransport_after_pairing() {
    let served = serve("example01");
    let dir = &served.dir;

    // Pair with the code the server shows, then come back with the token
    let code = served.authority.new_pairing_code(Permission::Control).unwrap();
    let (first, paired) = Remote::pair(&served.options, &code.display(), "ana").unwrap();
    assert_eq!(first.welcome().peer_name, "ana");
    assert_eq!(paired.permission, Permission::Control);
    drop(first);
    let remote = Remote::connect(&served.options, &paired.token).unwrap();
    assert_eq!(remote.welcome().server_name, "test server");

    // The caller says it is someone else: the server records who it let in
    let me = Caller::new("mallory", "notebook");
    let offers = OpsFilter { fillable_by: Some(vec![Type::new("dspk:unit"), Type::new("events")]), ..Default::default() };
    let ops: Vec<String> = remote.ops(&me, offers).unwrap().into_iter().map(|o| o.id).collect();
    assert_eq!(ops, ["script:firing_rate_by_condition"]);

    let target = Target::Op { op: "script:firing_rate_by_condition".into(), params: [("bin_ms".to_string(), ParamValue::Integer(250))].into() };
    let filling: BTreeMap<String, Source> = [("unit".into(), file(dir, "unit17.json", "unit 17", "dspk:unit", "json")), ("trials".into(), file(dir, "trials.parquet", "trials", "events", "parquet"))].into();
    let run = remote.run(&me, target.clone(), filling.clone()).unwrap();
    let record = remote.wait(&me, &run).unwrap();
    assert_eq!(record.status, RunStatus::Succeeded);
    assert_eq!((record.by.as_str(), record.via.as_str()), ("ana", "notebook"));
    let figure = record.outputs["figure"].clone();
    assert!(String::from_utf8(remote.read(&me, &figure).unwrap()).unwrap().contains("unit=unit 17"));

    // Reused when run again; traced; refused with why; exported
    let again = remote.wait(&me, &remote.run(&me, target, filling.clone()).unwrap()).unwrap();
    assert_eq!(again.metrics["reused"], serde_json::json!(run));
    assert_eq!(remote.lineage(&me, &figure).unwrap()[0].run, run);
    let bad = Target::Op { op: "script:firing_rate_by_condition".into(), params: [("bin_ms".to_string(), ParamValue::Integer(1))].into() };
    assert!(matches!(remote.run(&me, bad, filling.clone()), Err(RuntimeError::Invalid(why)) if why.contains("at least 10")));
    remote.export(&me, &figure, &dir.join("figure3.svg")).unwrap();
    assert!(dir.join("figure3.svg.run.json").exists());

    // Calls from several threads at once, each on its own stream
    let remote = Arc::new(remote);
    let types = remote.types(&me).unwrap().len();
    assert!(types >= 3);
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (remote, me) = (remote.clone(), me.clone());
            std::thread::spawn(move || remote.types(&me).unwrap().len())
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), types);
    }
}

#[test]
fn a_reader_reads_and_strangers_are_not_let_in() {
    let served = serve("permissions");
    let (_, token) = served.authority.enroll_token("ben", Permission::Read).unwrap();
    let reader = Remote::connect(&served.options, &token).unwrap();
    assert_eq!(reader.welcome().permission, Permission::Read);
    let me = Caller::new("ben", "page");
    assert!(reader.types(&me).unwrap().iter().any(|t| t.name == Type::new("dspk:unit")));
    let filling: BTreeMap<String, Source> = [("unit".into(), file(&served.dir, "u.json", "u", "dspk:unit", "json")), ("trials".into(), file(&served.dir, "t.parquet", "t", "events", "parquet"))].into();
    let target = Target::Op { op: "script:firing_rate_by_condition".into(), params: BTreeMap::new() };
    assert!(matches!(reader.run(&me, target, filling), Err(RuntimeError::Forbidden(_))));

    // An unknown token; a wrong code
    assert!(matches!(Remote::connect(&served.options, "0123456789abcdef"), Err(ServerError::Refused(_))));
    let code = served.authority.new_pairing_code(Permission::Control).unwrap();
    let mut wrong = code.display().chars().filter(char::is_ascii_digit).collect::<String>();
    let last = wrong.pop().unwrap();
    wrong.push(if last == '0' { '1' } else { '0' });
    assert!(matches!(Remote::pair(&served.options, &wrong, "eve"), Err(ServerError::Refused(_))));

    // A wrong certificate hash: the session is never opened
    let elsewhere = RemoteOptions { certificate_sha256: vec![0; 32], ..served.options.clone() };
    assert!(Remote::connect(&elsewhere, &token).is_err());
}
