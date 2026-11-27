//! Running scripts, out of process.
//!
//! - **Python**: one warm worker per Python program ([`PythonWorker`], `kitchen_worker.py`),
//!   kept alive between pieces so heavy imports are paid once; a changed script is reloaded. A
//!   worker that dies is started again on the next call.
//! - **Rust**: the file is compiled once per version into a small program ([`RustBuild`]): the
//!   harness reads the inputs (`.npy`) and parameters, calls `run`, writes the result.
//! - **Exchange**: each call gets a folder in memory (`/dev/shm` on Linux) holding the inputs as
//!   files, a `request.json` (and `meta.txt` for Rust), and the outputs the script writes; the
//!   app's [`kitchen_engine::domain::Exchange`] turns its data into those files and back.
//!
//! [`ScriptOp`] puts it together: a confirmed script as an [`kitchen_engine::Operation`].

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::interface::Options;
use crate::{Script, hash};
use kitchen_engine::domain::{Domain, ExchangeFile};
use kitchen_engine::op::{Call, Language, OpSpec, Operation};

/// The programs scripts need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tools {
    /// Python (a project's `.venv/bin/python`, else `python3`).
    pub python: PathBuf,
    /// `rustc`, for Rust scripts.
    pub rustc: PathBuf,
}

impl Tools {
    /// `$KITCHEN_PYTHON` or `python3` (`python` on Windows), `$KITCHEN_RUSTC` or `rustc`. Apps set
    /// their own from Settings › Tools.
    pub fn detect() -> Self {
        let python = std::env::var_os("KITCHEN_PYTHON").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "python" } else { "python3" }));
        let rustc = std::env::var_os("KITCHEN_RUSTC").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("rustc"));
        Self { python, rustc }
    }

    /// The Python of a project's `.venv` when there is one, else [`Tools::detect`]'s.
    pub fn for_project(dir: &Path) -> Self {
        let mut t = Self::detect();
        let venv = if cfg!(windows) { dir.join(".venv/Scripts/python.exe") } else { dir.join(".venv/bin/python") };
        if venv.is_file() {
            t.python = venv;
        }
        t
    }
}

/// The Python files the probe and the worker run, written once per process (and version of this
/// crate) into the temporary folder.
pub fn support_dir() -> Result<PathBuf, String> {
    static DIR: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    DIR.get_or_init(|| {
        let files: [(&str, &str); 4] = [
            ("kitchen_pipeline.py", include_str!("../python/kitchen_pipeline.py")),
            ("kitchen_runtime.py", include_str!("../python/kitchen_runtime.py")),
            ("kitchen_probe.py", include_str!("../python/kitchen_probe.py")),
            ("kitchen_worker.py", include_str!("../python/kitchen_worker.py")),
        ];
        let all: String = files.iter().map(|(_, t)| *t).collect();
        let dir = std::env::temp_dir().join(format!("kitchen-pipeline-{}", &hash(all.as_bytes())[7..19]));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (name, text) in files {
            std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))?;
        }
        Ok(dir)
    })
    .clone()
}

/// Where calls get their folders: memory when the system offers it (`/dev/shm`), else the
/// temporary folder.
pub fn scratch_root() -> PathBuf {
    let shm = Path::new("/dev/shm");
    let base = if cfg!(target_os = "linux") && shm.is_dir() { shm.to_path_buf() } else { std::env::temp_dir() };
    base.join(format!("kitchen-scripts-{}", std::process::id()))
}

/// A fresh, empty folder for one call.
fn call_dir() -> Result<PathBuf, String> {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = scratch_root().join(format!("call-{}", N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default)]
    outputs: BTreeMap<String, ExchangeFile>,
    #[serde(default)]
    error: Option<String>,
}

/// A warm Python worker.
pub struct PythonWorker {
    python: PathBuf,
    aliases: Vec<String>,
    process: Option<(Child, ChildStdin, BufReader<ChildStdout>)>,
}

impl PythonWorker {
    /// A worker running `tools.python`, giving scripts the app's decorator names. It starts on
    /// its first call.
    pub fn new(tools: &Tools, opts: &Options) -> Self {
        Self { python: tools.python.clone(), aliases: opts.aliases.clone(), process: None }
    }

    fn start(&mut self) -> Result<(), String> {
        let support = support_dir()?;
        let mut child = Command::new(&self.python)
            .arg("-u")
            .arg(support.join("kitchen_worker.py"))
            .arg(self.aliases.join(","))
            .env("PYTHONPATH", crate::probe::python_path(&support))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("{}: {e} (set the Python program in Settings › Tools)", self.python.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        self.process = Some((child, stdin, stdout));
        Ok(())
    }

    /// Runs `script` on the call in `dir`; the outputs it wrote. Starts (or restarts) the worker.
    pub fn call(&mut self, script: &Path, dir: &Path) -> Result<BTreeMap<String, ExchangeFile>, String> {
        let request = json!({"script": script, "dir": dir}).to_string();
        for attempt in 0..2 {
            if self.process.is_none() {
                self.start()?;
            }
            let (_, stdin, stdout) = self.process.as_mut().expect("started");
            let mut line = String::new();
            let sent = writeln!(stdin, "{request}").and_then(|_| stdin.flush());
            let read = sent.and_then(|_| stdout.read_line(&mut line));
            match read {
                Ok(n) if n > 0 => {
                    let reply: Reply = serde_json::from_str(line.trim()).map_err(|e| format!("the worker answered {line:?}: {e}"))?;
                    return if reply.ok { Ok(reply.outputs) } else { Err(reply.error.unwrap_or_else(|| "the script failed".into())) };
                }
                // The worker died (the script crashed Python): start a fresh one, once
                _ => {
                    self.stop();
                    if attempt == 1 {
                        return Err("the Python worker stopped while running the script".into());
                    }
                }
            }
        }
        unreachable!()
    }

    /// Stops the worker (it starts again on the next call).
    pub fn stop(&mut self) {
        if let Some((mut child, ..)) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for PythonWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Compiles Rust scripts into programs, once per version, in a cache folder.
pub struct RustBuild {
    rustc: PathBuf,
    cache: PathBuf,
}

/// What a Rust script's `run` gets: the harness, compiled around the script.
pub const RUST_HARNESS: &str = include_str!("harness.rs");

impl RustBuild {
    /// Builds with `tools.rustc` into the user's cache folder.
    pub fn new(tools: &Tools) -> Self {
        let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).unwrap_or_else(std::env::temp_dir);
        Self { rustc: tools.rustc.clone(), cache: base.join("kitchen-pipeline/rust") }
    }

    /// The program for the script at `path` (compiled if this version is new); compiler errors
    /// as they are.
    pub fn build(&self, path: &Path) -> Result<PathBuf, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let key = &hash(format!("{RUST_HARNESS}{text}").as_bytes())[7..23];
        let exe = self.cache.join(if cfg!(windows) { format!("{key}.exe") } else { key.to_string() });
        if exe.is_file() {
            return Ok(exe);
        }
        std::fs::create_dir_all(&self.cache).map_err(|e| format!("{}: {e}", self.cache.display()))?;
        let main = self.cache.join(format!("{key}.rs"));
        let source = format!("{RUST_HARNESS}\nmod user {{\n    #![allow(unused_imports)]\n    use crate::*;\n    include!({:?});\n}}\n", path.display().to_string());
        std::fs::write(&main, source).map_err(|e| e.to_string())?;
        let out = Command::new(&self.rustc).args(["--edition", "2024", "-O", "-o"]).arg(&exe).arg(&main).output().map_err(|e| format!("{}: {e}", self.rustc.display()))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(exe)
    }
}

/// A confirmed script as an operation of the engine.
pub struct ScriptOp {
    spec: OpSpec,
    path: PathBuf,
    language: Language,
    function: String,
    takes_piece: bool,
    python: Option<Arc<Mutex<PythonWorker>>>,
    rust: Option<PathBuf>,
}

impl ScriptOp {
    /// `script` as an operation, run by `python` (Python scripts) or compiled with `rust`.
    /// The caller has checked that its status allows it ([`crate::Approvals::can_run`]).
    pub fn new(script: &Script, python: Option<Arc<Mutex<PythonWorker>>>, rust: Option<&RustBuild>) -> Result<Self, String> {
        let spec = script.spec().ok_or_else(|| format!("{}: {}", script.path.display(), script.problems.join("; ")))?;
        let interface = script.interface.as_ref().expect("a spec has an interface");
        let rust = match (script.language, rust) {
            (Language::Rust, Some(b)) => Some(b.build(&script.path)?),
            (Language::Rust, None) => return Err("Rust scripts need a RustBuild".into()),
            _ => None,
        };
        if script.language == Language::Python && python.is_none() {
            return Err("Python scripts need a worker".into());
        }
        Ok(Self { spec, path: script.path.clone(), language: script.language, function: interface.function.clone(), takes_piece: interface.takes_piece, python, rust })
    }
}

impl<D: Domain> Operation<D> for ScriptOp {
    fn spec(&self) -> &OpSpec {
        &self.spec
    }

    fn run(&self, call: Call<'_, D>) -> Result<BTreeMap<String, D::Data>, String> {
        let exchange = call.domain.exchange().ok_or("this app does not run scripts on its data")?;
        let dir = call_dir()?;
        let mut files = BTreeMap::new();
        for p in &self.spec.inputs {
            if let Some(data) = call.inputs.get(&p.name) {
                let f = exchange.put(data, &p.ty, &dir, &p.name).map_err(|e| e.to_string())?;
                files.insert(p.name.clone(), f);
            }
        }
        let params: BTreeMap<&String, Value> = call.params.iter().map(|(k, v)| (k, v.to_json())).collect();
        let outputs: Vec<&str> = self.spec.outputs.iter().map(|p| p.name.as_str()).collect();
        let request = json!({
            "function": self.function,
            "inputs": files,
            "params": params,
            "piece": call.piece,
            "takes_piece": self.takes_piece,
            "outputs": outputs,
        });
        std::fs::write(dir.join("request.json"), request.to_string()).map_err(|e| e.to_string())?;
        let written = match self.language {
            Language::Python => {
                let worker = self.python.as_ref().expect("checked in new");
                let mut worker = worker.lock().map_err(|_| "the worker is unusable after a crash")?;
                worker.call(&self.path, &dir)?
            }
            Language::Rust => {
                // meta.txt: what the harness reads without a JSON parser
                let mut meta = String::new();
                for (name, f) in &files {
                    meta.push_str(&format!("input {name} {}\n", f.file));
                }
                for (name, v) in call.params {
                    match v.as_f64() {
                        Some(x) => meta.push_str(&format!("param {name} {x}\n")),
                        None => meta.push_str(&format!("text {name} {}\n", v.to_json().as_str().map(str::to_string).unwrap_or_else(|| v.to_json().to_string()))),
                    }
                }
                let first = outputs.first().copied().unwrap_or("out");
                meta.push_str(&format!("output {first} {first}.out.npy\n"));
                std::fs::write(dir.join("meta.txt"), meta).map_err(|e| e.to_string())?;
                let exe = self.rust.as_ref().expect("built in new");
                let out = Command::new(exe).arg(&dir).output().map_err(|e| format!("{}: {e}", exe.display()))?;
                if !out.status.success() {
                    return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
                }
                BTreeMap::from([(first.to_string(), ExchangeFile { file: format!("{first}.out.npy"), format: "npy".into() })])
            }
        };
        let like = self.spec.inputs.first().and_then(|p| call.inputs.get(&p.name));
        let mut result = BTreeMap::new();
        for p in &self.spec.outputs {
            let f = written.get(&p.name).ok_or_else(|| format!("the script gave no {}", p.name))?;
            result.insert(p.name.clone(), exchange.take(f, &p.ty, &dir, like).map_err(|e| e.to_string())?);
        }
        let _ = std::fs::remove_dir_all(&dir);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_files_are_written_once() {
        let a = support_dir().unwrap();
        assert_eq!(a, support_dir().unwrap());
        for f in ["kitchen_pipeline.py", "kitchen_runtime.py", "kitchen_probe.py", "kitchen_worker.py"] {
            assert!(a.join(f).is_file(), "{f}");
        }
    }

    /// The real worker, when Python with NumPy is here (skipped otherwise).
    #[test]
    fn the_python_worker_runs_a_script_and_survives_a_crash() {
        let tools = Tools::detect();
        let has_numpy = Command::new(&tools.python).args(["-c", "import numpy"]).output().is_ok_and(|o| o.status.success());
        if !has_numpy {
            return;
        }
        let opts = Options::new("dspk", &["signal", "table"], "signal");
        let dir = std::env::temp_dir().join(format!("kitchen-worker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("gain.py");
        std::fs::write(&script, "import dspk\nimport numpy as np\n\n@dspk.op\ndef gain(signal, g: float = 2.0):\n    return np.asarray(signal) * g\n").unwrap();
        let call = call_dir().unwrap();
        std::fs::write(call.join("request.json"), r#"{"function":"gain","inputs":{"signal":{"file":"signal.json","format":"json"}},"params":{"g":3.0},"piece":null,"outputs":["signal"]}"#).unwrap();
        std::fs::write(call.join("signal.json"), "[1, 2]").unwrap();
        let mut w = PythonWorker::new(&tools, &opts);
        let out = w.call(&script, &call).unwrap();
        assert_eq!(out["signal"].format, "npy");
        // A script that kills Python: the error, then a fresh worker for the next call
        let crash = dir.join("crash.py");
        std::fs::write(&crash, "import os\nimport dspk\n\n@dspk.op\ndef crash(signal):\n    os._exit(3)\n").unwrap();
        assert!(w.call(&crash, &call).is_err());
        assert!(w.call(&script, &call).is_ok(), "started again");
    }
}
