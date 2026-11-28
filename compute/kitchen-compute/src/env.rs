//! Environment variables of the compute layer, under the project's prefix.
//!
//! dsp-kitchen reads `DSPK_RUNTIME`, lucem `LUCEM_RUNTIME`: the same variables under each
//! project's name. A project's `<px>-core` calls [`set_env_prefix`] once, before the first target
//! is chosen (its own `init`, called by every front end: the CLI, the app, the Python module);
//! until then, and in a program that never sets one, the prefix is `KITCHEN`.
//!
//! | Variable | Read by | Meaning |
//! |---|---|---|
//! | `<PREFIX>_RUNTIME` | [`ComputeTarget::from_env`](crate::ComputeTarget::from_env) | `wgpu`, `cpu`, `cuda` or `hip` |
//! | `<PREFIX>_TEST_RUNTIMES` | [`ComputeTarget::for_tests`](crate::ComputeTarget::for_tests) | comma-separated runtimes tests run on |
//! | `<PREFIX>_KERNEL_CACHE` | `configure_runtime` | `0` turns off the compiled-kernel cache |
//! | `<PREFIX>_CACHE_DIR` | `configure_runtime` | directory of CubeCL's caches |

use std::sync::OnceLock;

/// The prefix used when no project set one.
pub const DEFAULT_PREFIX: &str = "KITCHEN";

/// Suffix of the variable that picks the runtime.
pub const RUNTIME: &str = "RUNTIME";
/// Suffix of the variable that limits the runtimes tests run on.
pub const TEST_RUNTIMES: &str = "TEST_RUNTIMES";
/// Suffix of the variable that turns the compiled-kernel cache off (`0`).
pub const KERNEL_CACHE: &str = "KERNEL_CACHE";
/// Suffix of the variable that moves CubeCL's caches.
pub const CACHE_DIR: &str = "CACHE_DIR";

static PREFIX: OnceLock<&'static str> = OnceLock::new();

/// Sets the project's prefix (e.g. `"DSPK"`). The first call wins; returns whether `prefix` is now
/// the prefix (a second call with the same prefix is harmless, a different one is refused, so two
/// projects loaded into one process cannot silently rename each other's variables).
pub fn set_env_prefix(prefix: &'static str) -> bool {
    set_in(&PREFIX, prefix)
}

fn set_in(cell: &OnceLock<&'static str>, prefix: &'static str) -> bool {
    *cell.get_or_init(|| prefix) == prefix
}

/// The project's prefix, or [`DEFAULT_PREFIX`].
pub fn env_prefix() -> &'static str {
    PREFIX.get().copied().unwrap_or(DEFAULT_PREFIX)
}

/// The full name of the variable with `suffix`, e.g. `env_name(RUNTIME)` → `"DSPK_RUNTIME"`.
pub fn env_name(suffix: &str) -> String {
    format!("{}_{suffix}", env_prefix())
}

/// The value of the variable with `suffix`, when it is set and not blank. In a browser there is no
/// environment: always `None`.
pub fn env_value(suffix: &str) -> Option<String> {
    if cfg!(target_family = "wasm") {
        return None;
    }
    std::env::var(env_name(suffix)).ok().filter(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_prefix_wins() {
        // A cell of its own: the process-wide prefix is read by other tests
        let cell = OnceLock::new();
        assert!(set_in(&cell, "KTEST"));
        assert!(set_in(&cell, "KTEST"), "the same prefix again is fine");
        assert!(!set_in(&cell, "OTHER"), "a different one is refused");
    }

    #[test]
    fn names_carry_the_prefix() {
        assert_eq!(env_name(RUNTIME), format!("{}_RUNTIME", env_prefix()));
    }
}
