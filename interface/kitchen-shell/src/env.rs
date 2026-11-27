//! A project's environments for its scripts: what Python runs them, with which packages, and which
//! Rust toolchain builds them. The project folder holds them, so a project (and a pipeline built
//! in it) runs the same on another computer:
//!
//! ```text
//! scripts/
//!   pyproject.toml  uv.lock  .venv/               Python: the packages, locked; the environment
//!   Cargo.toml  Cargo.lock  rust-toolchain.toml   Rust: the crates, locked; the toolchain
//! ```
//!
//! - **Python** runs with the environment's own interpreter (`.venv/bin/python`): the project's
//!   `.venv`, else the one activated (`$VIRTUAL_ENV`), else the app's default environment. uv
//!   makes and syncs it from `pyproject.toml` and `uv.lock` ([`sync_python`]); the lock, not the
//!   `.venv`, is what travels (a `.venv` names its base interpreter, so it is made again elsewhere).
//! - **Rust** has no environment to activate: `Cargo.toml` and `Cargo.lock` are the crates, and
//!   `rust-toolchain.toml` pins the toolchain (rustup reads it from the folder Cargo runs in).
//!   [`ensure_rust`] writes them; builds share one target folder.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a Python environment comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PythonSource {
    /// The project's own (`scripts/.venv`).
    Project,
    /// The one activated in the shell the app started from (`$VIRTUAL_ENV`).
    Active,
    /// The app's default environment (for projects that have none).
    Default,
}

impl PythonSource {
    /// What Settings calls it.
    pub fn title(self) -> &'static str {
        match self {
            PythonSource::Project => "the project's",
            PythonSource::Active => "the active one ($VIRTUAL_ENV)",
            PythonSource::Default => "the app's default",
        }
    }
}

/// A Python environment (a venv).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonEnv {
    /// The environment's folder (`.venv`).
    pub root: PathBuf,
    /// Its interpreter.
    pub python: PathBuf,
    /// Where it comes from.
    pub source: PythonSource,
}

/// The interpreter inside the venv at `root`, if there is one.
pub fn venv_python(root: &Path) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) { &["Scripts/python.exe"] } else { &["bin/python", "bin/python3"] };
    candidates.iter().map(|c| root.join(c)).find(|p| p.exists())
}

impl PythonEnv {
    /// The environment for `scripts` (a project's scripts folder; `None` without a project), else
    /// the active one, else the default one in `default_root` (when it was made).
    pub fn find(scripts: Option<&Path>, default_root: &Path) -> Option<PythonEnv> {
        let at = |root: PathBuf, source| venv_python(&root).map(|python| PythonEnv { root, python, source });
        scripts
            .and_then(|d| at(d.join(".venv"), PythonSource::Project))
            .or_else(|| std::env::var_os("VIRTUAL_ENV").filter(|v| !v.is_empty()).and_then(|v| at(PathBuf::from(v), PythonSource::Active)))
            .or_else(|| at(default_root.to_path_buf(), PythonSource::Default))
    }

    /// A command running this environment's Python (as an activated venv would: `VIRTUAL_ENV`
    /// set, its `bin` first on the `PATH`).
    pub fn command(&self) -> Command {
        let mut c = Command::new(&self.python);
        c.env("VIRTUAL_ENV", &self.root);
        if let Some(bin) = self.python.parent() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let joined = std::env::join_paths(std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(&path))).unwrap_or(path);
            c.env("PATH", joined);
        }
        c.env_remove("PYTHONHOME");
        c
    }

    /// Its Python version (`3.13.1`), from `pyvenv.cfg` (no process started).
    pub fn version(&self) -> Option<String> {
        let cfg = std::fs::read_to_string(self.root.join("pyvenv.cfg")).ok()?;
        cfg.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            matches!(k.trim(), "version" | "version_info").then(|| v.trim().to_string())
        })
    }

    /// The packages installed (their names, sorted), from the `*.dist-info` folders.
    pub fn packages(&self) -> Vec<String> {
        let lib = if cfg!(windows) { vec![self.root.join("Lib").join("site-packages")] } else { std::fs::read_dir(self.root.join("lib")).into_iter().flatten().flatten().map(|e| e.path().join("site-packages")).collect() };
        let mut out: Vec<String> = lib
            .iter()
            .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let stem = name.strip_suffix(".dist-info")?;
                Some(stem.rsplit_once('-').map_or(stem, |(n, _)| n).replace('_', "-").to_lowercase())
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

/// uv, if found: `$UV`, the `PATH`, then where its installer puts it.
pub fn uv() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("UV").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let exe = if cfg!(windows) { "uv.exe" } else { "uv" };
    let on_path = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(exe)).find(|c| c.is_file()));
    on_path.or_else(|| {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)?;
        [home.join(".local/bin").join(exe), home.join(".cargo/bin").join(exe)].into_iter().find(|c| c.is_file())
    })
}

/// The `pyproject.toml` written for a new environment: a project of scripts, not a package.
fn pyproject(name: &str, packages: &[&str]) -> String {
    let deps = packages.iter().map(|p| format!("    \"{p}\",")).collect::<Vec<_>>().join("\n");
    format!("[project]\nname = \"{name}\"\nversion = \"0.0.0\"\ndescription = \"The packages this project's Python scripts use\"\nrequires-python = \">=3.10\"\ndependencies = [\n{deps}\n]\n\n[tool.uv]\npackage = false\n")
}

fn run(cmd: &mut Command, what: &str) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_string();
    if out.status.success() { Ok(text) } else { Err(format!("{what} failed: {text}")) }
}

/// Makes or updates the Python environment in `dir` with uv: writes `pyproject.toml` first when
/// there is none (with `packages`, the app's own needs such as NumPy), then `uv sync` (the
/// `.venv` matching `uv.lock`, made when missing). Returns uv's report.
pub fn sync_python(dir: &Path, name: &str, packages: &[&str]) -> Result<String, String> {
    let uv = uv().ok_or("uv is not installed (https://docs.astral.sh/uv): it makes and syncs the project's environment")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let project = dir.join("pyproject.toml");
    if !project.exists() {
        std::fs::write(&project, pyproject(name, packages)).map_err(|e| format!("{}: {e}", project.display()))?;
    }
    run(Command::new(&uv).arg("sync").arg("--project").arg(dir).env_remove("VIRTUAL_ENV"), "uv sync")
}

/// Adds `packages` to the environment in `dir` (`uv add`: `pyproject.toml`, `uv.lock` and the
/// `.venv` together).
pub fn add_python(dir: &Path, packages: &[String]) -> Result<String, String> {
    let uv = uv().ok_or("uv is not installed")?;
    run(Command::new(&uv).arg("add").arg("--project").arg(dir).args(packages).env_remove("VIRTUAL_ENV"), "uv add")
}

/// The Rust toolchain a folder pins (`channel` of its `rust-toolchain.toml`).
pub fn rust_toolchain(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("rust-toolchain.toml")).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "channel").then(|| v.trim().trim_matches('"').to_string())
    })
}

/// The version of the Rust compiler found (`1.95.0`), the one a new pin names.
pub fn rustc_version() -> Option<String> {
    let out = Command::new("rustc").arg("--version").output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().nth(1).map(str::to_string)
}

/// Makes `dir` a Rust environment when it is not one yet: a Cargo workspace whose members are the
/// scripts' crates (`members`, globs allowed), and `rust-toolchain.toml` pinning the compiler found
/// now. The lock (`Cargo.lock`) is written by the first build. Returns the toolchain pinned.
pub fn ensure_rust(dir: &Path, members: &[&str]) -> Result<Option<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let manifest = dir.join("Cargo.toml");
    if !manifest.exists() {
        let list = members.iter().map(|m| format!("\"{m}\"")).collect::<Vec<_>>().join(", ");
        std::fs::write(&manifest, format!("# The crates this project's Rust scripts use; Cargo.lock beside it locks them.\n[workspace]\nresolver = \"3\"\nmembers = [{list}]\n")).map_err(|e| format!("{}: {e}", manifest.display()))?;
    }
    let pin = dir.join("rust-toolchain.toml");
    if !pin.exists()
        && let Some(v) = rustc_version()
    {
        std::fs::write(&pin, format!("# The compiler this project's Rust scripts are built with (rustup reads it).\n[toolchain]\nchannel = \"{v}\"\n")).map_err(|e| format!("{}: {e}", pin.display()))?;
    }
    Ok(rust_toolchain(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kitchen-env-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A venv laid out by hand: found in the project, its version and packages read without
    /// running anything.
    #[test]
    fn a_project_venv_is_found_and_read() {
        let d = temp("venv");
        let venv = d.join(".venv");
        let bin = if cfg!(windows) { venv.join("Scripts") } else { venv.join("bin") };
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(if cfg!(windows) { "python.exe" } else { "python" }), "").unwrap();
        std::fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin\nversion_info = 3.13.1\n").unwrap();
        let site = if cfg!(windows) { venv.join("Lib/site-packages") } else { venv.join("lib/python3.13/site-packages") };
        std::fs::create_dir_all(site.join("numpy-2.3.1.dist-info")).unwrap();
        std::fs::create_dir_all(site.join("scikit_image-0.25.2.dist-info")).unwrap();
        let env = PythonEnv::find(Some(&d), &d.join("default")).expect("the project's venv");
        assert_eq!(env.source, PythonSource::Project);
        assert_eq!(env.version().as_deref(), Some("3.13.1"));
        assert_eq!(env.packages(), vec!["numpy".to_string(), "scikit-image".to_string()]);
        std::fs::remove_dir_all(&d).ok();
    }

    /// A Rust environment: a workspace manifest and the toolchain pinned, written once.
    #[test]
    fn a_rust_environment_is_written_once() {
        let d = temp("rust");
        let pinned = ensure_rust(&d, &["build/*"]).unwrap();
        let manifest = std::fs::read_to_string(d.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("[workspace]") && manifest.contains("\"build/*\""));
        if let Some(v) = rustc_version() {
            assert_eq!(pinned.as_deref(), Some(v.as_str()));
        }
        std::fs::write(d.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"1.90.0\"\n").unwrap();
        ensure_rust(&d, &["build/*"]).unwrap();
        assert_eq!(rust_toolchain(&d).as_deref(), Some("1.90.0"), "an existing pin stays");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_new_pyproject_is_a_project_of_scripts() {
        let p = pyproject("case-12-scripts", &["numpy"]);
        assert!(p.contains("name = \"case-12-scripts\"") && p.contains("\"numpy\",") && p.contains("package = false"));
    }
}
