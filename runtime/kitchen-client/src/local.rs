//! The in-process runtime: what an app's own view-models call, with no encoding and no network.
//!
//! - **Ops** come from the app's [`Catalog`]: its own, the shared ones, its tools', its folders'
//!   scripts and recipes; another app's are not in it (decision 0016).
//! - **Runs** go to a pool of compute workers apart from the caller's thread, so a long run never
//!   delays a small call (decision 0017); `run.watch` reads where each is, `run.cancel` stops one
//!   between pieces.
//! - **What runs** is the app's [`Runner`] (its engine over its domain, a tool): the runtime hands
//!   it the inputs as paths and a folder for its outputs, then keeps the outputs in the store
//!   (by reference), writes the run record, and recognizes an identical run by its run key and
//!   reuses its outputs (never for an op with effects, or a cancelled run's).
//! - **Results, lineage, export** are the store's ([`Workspace`]).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, RwLock};

use kitchen_contract::{CONTRACT_VERSION, Catalog, ContentId, Format, OpRef, OpSpec, ParamValue, RunKey, RunRecord, RunStatus, Type, check_params, types};
use kitchen_store::Workspace;

use crate::calls::{Answer, Caller, OpsFilter, Request, ResultsFilter, RunState, RuntimeError, Source, Target};
use crate::runtime::{Reply, Runtime};

/// Stops a run from another thread, between pieces.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Whether the run was asked to stop.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// One run, as the runtime hands it to the app's [`Runner`].
#[derive(Clone, Debug)]
pub struct Job {
    /// Its id.
    pub run: String,
    /// What runs.
    pub target: Target,
    /// For an op: every parameter, checked, defaults filled.
    pub params: BTreeMap<String, ParamValue>,
    /// Source name → the file or folder that fills it.
    pub inputs: BTreeMap<String, PathBuf>,
    /// An empty folder for its outputs.
    pub out_dir: PathBuf,
    /// Run options (device, piece size, threads).
    pub options: BTreeMap<String, serde_json::Value>,
}

/// An output a run wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Produced {
    /// Its port (an op's) or its name (a pipeline's output).
    pub name: String,
    /// The file or folder, inside the job's `out_dir`.
    pub path: PathBuf,
    /// Its type.
    pub ty: Type,
    /// Its format.
    pub format: Format,
}

/// What actually runs a job: the app's engine over its domain, or a tool. Called on a compute
/// worker; `progress(done, pieces)` after each piece; `cancel` checked between pieces.
pub trait Runner: Send + Sync {
    /// Runs `job`; what it wrote.
    fn run(&self, job: &Job, cancel: &Cancel, progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String>;
}

struct Entry {
    state: RunState,
    cancel: Cancel,
}

type Work = Box<dyn FnOnce() + Send>;

/// The in-process runtime of one app.
pub struct Local {
    app: String,
    catalog: RwLock<Catalog>,
    store: Workspace,
    runner: Arc<dyn Runner>,
    runs: Arc<Mutex<HashMap<String, Entry>>>,
    work: Mutex<Sender<Work>>,
    next: AtomicU64,
}

impl Local {
    /// The runtime of `app` (`dspk`) with its `catalog`, its store in `folder`'s workspace
    /// (`.<app>/`), `runner` for what runs, and `workers` compute threads (at least one).
    pub fn new(app: &str, catalog: Catalog, folder: &std::path::Path, runner: Arc<dyn Runner>, workers: usize) -> Result<Self, RuntimeError> {
        let store = Workspace::open(folder, app).map_err(|e| RuntimeError::Io(e.to_string()))?;
        let (tx, rx) = channel::<Work>();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..workers.max(1) {
            let rx: Arc<Mutex<Receiver<Work>>> = rx.clone();
            std::thread::Builder::new()
                .name(format!("kitchen-runtime-{i}"))
                .spawn(move || {
                    loop {
                        let job = rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        match job {
                            Ok(work) => work(),
                            Err(_) => break,
                        }
                    }
                })
                .map_err(|e| RuntimeError::Io(e.to_string()))?;
        }
        Ok(Self { app: app.into(), catalog: RwLock::new(catalog), store, runner, runs: Arc::new(Mutex::new(HashMap::new())), work: Mutex::new(tx), next: AtomicU64::new(1) })
    }

    /// The store, for what the runtime API does not say (an app's own views of it).
    pub fn store(&self) -> &Workspace {
        &self.store
    }

    /// Adds or replaces an op description (a script checked, a recipe saved).
    pub fn add_op(&self, spec: OpSpec) -> Result<(), RuntimeError> {
        self.catalog.write().unwrap_or_else(|e| e.into_inner()).add(spec).map_err(RuntimeError::Invalid)
    }

    fn ops(&self, filter: &OpsFilter) -> Vec<OpSpec> {
        let catalog = self.catalog.read().unwrap_or_else(|e| e.into_inner());
        let mut ops: Vec<&OpSpec> = match &filter.fillable_by {
            Some(offers) => catalog.fillable_by(offers),
            None => catalog.all().collect(),
        };
        if let Some(t) = &filter.input {
            ops.retain(|o| o.inputs.iter().any(|p| t.feeds(&p.ty)));
        }
        if let Some(f) = &filter.family {
            ops.retain(|o| &o.family == f);
        }
        if let Some(q) = &filter.search {
            let found: Vec<String> = catalog.search(q).iter().map(|o| o.id.clone()).collect();
            ops.retain(|o| found.contains(&o.id));
        }
        ops.into_iter().cloned().collect()
    }

    fn results(&self, filter: &ResultsFilter) -> Reply<Vec<kitchen_contract::ResultRecord>> {
        let io = |e: kitchen_store::StoreError| RuntimeError::Io(e.to_string());
        let mut kept: Vec<kitchen_contract::ResultRecord> = self.store.results().map_err(io)?.into_iter().map(|k| k.record).collect();
        if let Some(from) = &filter.from {
            let made = self.store.made_from(from).map_err(io)?;
            kept.retain(|r| made.contains(&r.id));
        }
        if let Some(t) = &filter.ty {
            kept.retain(|r| &r.ty == t);
        }
        if let Some(run) = &filter.run {
            kept.retain(|r| r.run.as_deref() == Some(run.as_str()));
        }
        if let Some(op) = &filter.op {
            let runs = self.store.runs().map_err(io)?;
            kept.retain(|r| r.run.as_ref().is_some_and(|id| runs.iter().any(|x| &x.run == id && &x.op.id == op)));
        }
        kept.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(kept)
    }

    /// The op (or pipeline) a target names, its version key, its parameters checked, and whether
    /// it acts outside the store.
    fn resolve(&self, target: &Target) -> Reply<(OpRef, BTreeMap<String, ParamValue>, bool)> {
        match target {
            Target::Op { op, params } => {
                let catalog = self.catalog.read().unwrap_or_else(|e| e.into_inner());
                let spec = catalog.get(op).ok_or_else(|| RuntimeError::NotFound(format!("no op {op:?} in {} (another app's op runs in a chain, on the orchestrator)", self.app)))?;
                if !spec.available {
                    return Err(RuntimeError::Invalid(format!("{op} cannot run here: {}", spec.unavailable.clone().unwrap_or_default())));
                }
                let params = check_params(&spec.params, params).map_err(|p| RuntimeError::Invalid(format!("{op}: {}", p.join("; "))))?;
                Ok((OpRef { id: op.clone(), version: spec.version_key() }, params, spec.effects))
            }
            Target::Pipeline { pipeline } => {
                let catalog = self.catalog.read().unwrap_or_else(|e| e.into_inner());
                pipeline.check(&catalog).map_err(|p| RuntimeError::Invalid(p.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; ")))?;
                let id = kitchen_contract::ContentId::of_json(&serde_json::to_value(pipeline).map_err(|e| RuntimeError::Invalid(e.to_string()))?);
                let effects = pipeline.nodes.values().any(|n| catalog.get(&n.op).is_some_and(|s| s.effects));
                Ok((OpRef { id: "pipeline".into(), version: id.into() }, BTreeMap::new(), effects))
            }
        }
    }

    /// The id and the path of each source of a filling (a path is kept as a source result).
    fn inputs(&self, filling: &BTreeMap<String, Source>) -> Reply<(BTreeMap<String, ContentId>, BTreeMap<String, PathBuf>)> {
        let mut ids = BTreeMap::new();
        let mut paths = BTreeMap::new();
        for (name, source) in filling {
            let (id, path) = match source {
                Source::Result { id } => (id.clone(), self.store.locate(id).map_err(|e| RuntimeError::NotFound(e.to_string()))?),
                Source::Path { path, ty, format } => {
                    let kept = self.store.keep(path, ty.clone(), format.clone(), None, false).map_err(|e| RuntimeError::Io(e.to_string()))?;
                    (kept.record.id, kept.path)
                }
            };
            ids.insert(name.clone(), id);
            paths.insert(name.clone(), path);
        }
        Ok((ids, paths))
    }

    fn start(&self, caller: &Caller, target: Target, filling: BTreeMap<String, Source>, options: BTreeMap<String, serde_json::Value>) -> Reply<String> {
        let (op, params, effects) = self.resolve(&target)?;
        let (input_ids, input_paths) = self.inputs(&filling)?;
        let run_key = RunKey::of(&op.id, &op.version, &params, &input_ids);
        let run = format!("run-{}-{}", jiff::Timestamp::now().as_millisecond(), self.next.fetch_add(1, Ordering::Relaxed));

        // An identical run already done: its outputs stand in (decision 0016: never with effects)
        if !effects {
            let done = self.store.runs().map_err(|e| RuntimeError::Io(e.to_string()))?;
            if let Some(earlier) = done.into_iter().find(|r| r.run_key == run_key && r.reusable(false)) {
                let mut record = earlier.clone();
                record.run = run.clone();
                record.by = caller.by.clone();
                record.via = caller.via.clone();
                record.started = jiff::Timestamp::now().to_string();
                record.ended = Some(record.started.clone());
                record.metrics = [("reused".to_string(), serde_json::json!(earlier.run))].into();
                self.store.record_run(&record).map_err(|e| RuntimeError::Io(e.to_string()))?;
                self.runs.lock().unwrap_or_else(|e| e.into_inner()).insert(run.clone(), Entry { state: RunState::Ended { record: Box::new(record) }, cancel: Cancel::default() });
                return Ok(run);
            }
        }

        let out_dir = self.store.root().join("outputs").join(&run);
        std::fs::create_dir_all(&out_dir).map_err(|e| RuntimeError::Io(format!("{}: {e}", out_dir.display())))?;
        let cancel = Cancel::default();
        self.runs.lock().unwrap_or_else(|e| e.into_inner()).insert(run.clone(), Entry { state: RunState::Queued, cancel: cancel.clone() });
        let job = Job { run: run.clone(), target, params: params.clone(), inputs: input_paths, out_dir, options: options.clone() };
        let record = RunRecord {
            contract: CONTRACT_VERSION.into(),
            run: run.clone(),
            op,
            params,
            inputs: input_ids,
            outputs: BTreeMap::new(),
            by: caller.by.clone(),
            via: caller.via.clone(),
            started: String::new(),
            ended: None,
            status: RunStatus::Succeeded,
            environment: [("app".to_string(), serde_json::json!(self.app)), ("options".to_string(), serde_json::json!(options))].into(),
            metrics: BTreeMap::new(),
            run_key,
        };
        let (runner, runs, store) = (self.runner.clone(), self.runs.clone(), self.store.clone());
        let work: Work = Box::new(move || execute(job, record, cancel, runner, runs, store));
        self.work.lock().unwrap_or_else(|e| e.into_inner()).send(work).map_err(|_| RuntimeError::Io("the compute workers stopped".into()))?;
        Ok(run)
    }
}

/// Runs one job on a compute worker, then keeps its outputs and writes its record.
fn execute(job: Job, mut record: RunRecord, cancel: Cancel, runner: Arc<dyn Runner>, runs: Arc<Mutex<HashMap<String, Entry>>>, store: Workspace) {
    let set = |state: RunState| {
        if let Some(e) = runs.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&job.run) {
            e.state = state;
        }
    };
    record.started = jiff::Timestamp::now().to_string();
    let started = std::time::Instant::now();
    set(RunState::Running { done: 0, pieces: 0 });
    let result = runner.run(&job, &cancel, &mut |done, pieces| set(RunState::Running { done, pieces }));
    let cancelled = cancel.is_cancelled();
    match result {
        Ok(produced) => {
            for p in produced {
                match store.keep(&p.path, p.ty, p.format, Some(job.run.clone()), cancelled) {
                    Ok(kept) => {
                        record.outputs.insert(p.name, kept.record.id);
                    }
                    Err(e) => {
                        record.status = RunStatus::Failed { why: format!("keeping {}: {e}", p.name) };
                    }
                }
            }
            if cancelled {
                record.status = RunStatus::Cancelled;
            }
        }
        Err(why) => record.status = if cancelled { RunStatus::Cancelled } else { RunStatus::Failed { why } },
    }
    record.ended = Some(jiff::Timestamp::now().to_string());
    record.metrics.insert("seconds".into(), serde_json::json!(started.elapsed().as_secs_f64()));
    if let Err(e) = store.record_run(&record) {
        record.status = RunStatus::Failed { why: format!("recording the run: {e}") };
    }
    set(RunState::Ended { record: Box::new(record) });
}

impl Runtime for Local {
    fn call(&self, caller: &Caller, request: Request) -> Reply<Answer> {
        let io = |e: kitchen_store::StoreError| RuntimeError::Io(e.to_string());
        match request {
            Request::Hello { contract, .. } => {
                let major = |v: &str| v.split('.').next().unwrap_or("").to_string();
                if major(&contract) != major(CONTRACT_VERSION) {
                    return Err(RuntimeError::Unsupported(format!("contract {contract}; this runtime speaks {CONTRACT_VERSION}")));
                }
                Ok(Answer::Welcome { contract: CONTRACT_VERSION.into(), capabilities: vec!["recipes".into(), "batches".into(), "lineage".into()], apps: vec![self.app.clone()] })
            }
            Request::Types => {
                let catalog = self.catalog.read().unwrap_or_else(|e| e.into_inner());
                let mut out = types::shared();
                for t in catalog.types() {
                    if !out.iter().any(|d| &d.name == t) {
                        out.push(kitchen_contract::TypeDescription { name: t.clone(), summary: format!("{}'s type", self.app), formats: vec![], axes: vec![] });
                    }
                }
                Ok(Answer::Types { types: out })
            }
            Request::Ops { filter } => Ok(Answer::Ops { ops: self.ops(&filter) }),
            Request::PipelineCheck { pipeline } => {
                let catalog = self.catalog.read().unwrap_or_else(|e| e.into_inner());
                Ok(match pipeline.check(&catalog) {
                    Ok(plan) => Answer::Checked { order: plan.order, margin: plan.margin, problems: vec![] },
                    Err(problems) => Answer::Checked { order: vec![], margin: kitchen_contract::Margin::NONE, problems: problems.iter().map(|p| p.to_string()).collect() },
                })
            }
            Request::Run { target, fillings, options } => {
                if fillings.is_empty() {
                    return Err(RuntimeError::Invalid("a run needs at least one filling of its sources".into()));
                }
                let mut started = Vec::new();
                for filling in fillings {
                    started.push(self.start(caller, target.clone(), filling, options.clone())?);
                }
                let batch = if started.len() == 1 { started[0].clone() } else { format!("batch-{}", started[0]) };
                Ok(Answer::Started { batch, runs: started })
            }
            Request::RunWatch { run } => {
                let runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
                match runs.get(&run) {
                    Some(e) => Ok(Answer::Run { state: e.state.clone() }),
                    None => {
                        drop(runs);
                        let record = self.store.run(&run).map_err(|_| RuntimeError::NotFound(format!("no run {run}")))?;
                        Ok(Answer::Run { state: RunState::Ended { record: Box::new(record) } })
                    }
                }
            }
            Request::RunCancel { run } => {
                let runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
                let e = runs.get(&run).ok_or_else(|| RuntimeError::NotFound(format!("no run {run} in progress")))?;
                e.cancel.cancel();
                Ok(Answer::Done)
            }
            Request::Results { filter } => Ok(Answer::Results { results: self.results(&filter)? }),
            Request::ResultRead { id } => {
                let path = self.store.locate(&id).map_err(|e| RuntimeError::NotFound(e.to_string()))?;
                if path.is_dir() {
                    return Err(RuntimeError::Unsupported(format!("{} is a folder: read it by range (step 4e)", id.short())));
                }
                Ok(Answer::Bytes { bytes: std::fs::read(&path).map_err(|e| RuntimeError::Io(e.to_string()))? })
            }
            Request::ResultExport { id, path } => {
                self.store.export(&id, &path).map_err(io)?;
                Ok(Answer::Done)
            }
            Request::Lineage { id } => Ok(Answer::Lineage { runs: self.store.lineage(&id).map_err(io)? }),
        }
    }
}
