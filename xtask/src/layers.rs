//! `cargo xtask check-layers`: the architecture's rules (kitchen.toml, standard/architecture.md),
//! checked against what the manifests declare (`cargo metadata`).
//!
//! 1. Every workspace crate is in exactly one layer (a new crate must be placed).
//! 2. A crate's normal and build dependencies (optional ones too) are in its own layer or below.
//!    Dev-dependencies may reach higher: a test may drive the frame.
//! 3. An upward dependency passes only when kitchen.toml lists it under [rules] exceptions, with
//!    why; an exception that no longer happens is reported, so the list only shrinks.
//! 4. Every crate has the project's licence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::TaskResult;
use crate::config::Kitchen;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    license: Option<String>,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    /// The package's own name (a `rename` does not change it).
    name: String,
    /// `None` for normal dependencies, `"dev"` or `"build"`.
    kind: Option<String>,
}

pub fn check(root: &Path) -> TaskResult {
    let config = Kitchen::load(root)?;
    let metadata = metadata(root)?;
    let workspace: BTreeMap<&str, &Package> = metadata.packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut problems = Vec::new();
    let mut used_exceptions = BTreeSet::new();

    for layer in &config.layers {
        for name in &layer.crates {
            if !workspace.contains_key(name.as_str()) {
                problems.push(format!("kitchen.toml: layer `{}` names `{name}`, which is not a workspace crate", layer.name));
            }
        }
    }

    for package in workspace.values() {
        // 1. Placed
        let Some(own) = config.layer_of(&package.name) else {
            problems.push(format!("`{}` is in no layer of kitchen.toml: place it", package.name));
            continue;
        };

        // 4. Licence
        let declared = package.license.as_deref().unwrap_or("(none)");
        if declared != config.project.license {
            problems.push(format!("`{}` declares licence `{declared}`, kitchen.toml expects `{}`", package.name, config.project.license));
        }

        // 2. and 3. Layers
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") || !workspace.contains_key(dependency.name.as_str()) {
                continue;
            }
            let Some(theirs) = config.layer_of(&dependency.name) else { continue };
            if theirs <= own {
                continue;
            }
            if config.exception(&package.name, &dependency.name).is_some() {
                used_exceptions.insert((package.name.clone(), dependency.name.clone()));
                continue;
            }
            problems.push(format!(
                "`{}` (layer `{}`) depends on `{}` (layer `{}`), a higher layer",
                package.name, config.layers[own].name, dependency.name, config.layers[theirs].name
            ));
        }
    }

    for exception in &config.rules.exceptions {
        if !used_exceptions.contains(&(exception.name.clone(), exception.depends_on.clone())) {
            problems.push(format!(
                "kitchen.toml: the exception `{}` → `{}` no longer happens: remove it",
                exception.name, exception.depends_on
            ));
        }
    }

    if problems.is_empty() {
        eprintln!("xtask: {} crates, every one in its layer", workspace.len());
        for exception in &config.rules.exceptions {
            eprintln!("  known exception: `{}` → `{}` ({})", exception.name, exception.depends_on, exception.why);
        }
        Ok(())
    } else {
        for problem in &problems {
            eprintln!("  {problem}");
        }
        Err(format!("{} layer rule(s) broken (kitchen.toml)", problems.len()))
    }
}

/// The workspace's own packages with their declared dependencies (`--no-deps`: nothing resolved,
/// nothing downloaded).
fn metadata(root: &Path) -> Result<Metadata, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata: {e}"))
}
