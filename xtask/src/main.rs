//! `cargo xtask <command>`: kitchen's own tasks.
//!
//! | Command | Does |
//! |---|---|
//! | `check-layers` | every crate in its layer, dependencies pointing down only (kitchen.toml) |
//!
//! Ported from kitchen-template's xtask (its build, test and new-op are a project's, not kitchen's).

mod config;
mod layers;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let result = match args.first().map(String::as_str) {
        Some("check-layers") => layers::check(&root),
        _ => Err("usage: cargo xtask check-layers".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

/// A task's failure, said in one message.
pub type TaskResult = Result<(), String>;

/// The workspace root: xtask's own manifest is one folder below it.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask sits in the workspace root").to_path_buf()
}
