# kitchen-compute

The neuro-kitchen compute layer: how every project chooses a CubeCL runtime (wgpu, CUDA, HIP, CPU)
at run time, opens and shuts down its devices, sizes kernel launches, and keeps runs reproducible.
Merged from dsp-kitchen `dspk-core` 0.0.13 and lucem `lucem-core` 0.0.6, where the same code lived
twice.

**Status: written, not built yet.** It's merged from code that compiles in both projects; expect
small fixes on the first build.

| Module | From | Change |
|---|---|---|
| `target` (`ComputeTarget`) | lucem `device.rs` + dsp-kitchen `with_target` | union of the two |
| `runtime` | dsp-kitchen `compute/target.rs` | superset of lucem's |
| `launch` | lucem `compute/launch.rs` | domain-neutral names |
| `bench`, `limits`, `pinning`, `special`, `tune` | both | `limits` returns `ComputeError` |
| `env` | new | `<PREFIX>_RUNTIME` etc. under the project's prefix |

These stay in the projects because they're domain-specific:
- pointwise op codes
- lucem's image batches
- dsp-kitchen's leases, recordings, windows and masks

## Features

`default = []`: with no runtime feature on, `ComputeTarget` is available without CubeCL. Turn on
`wgpu`, `cuda`, `cpu` or `hip` in the project's core crate, and every crate below it inherits them
(one feature set, one build).

## Adopting it in a project

Do one pull request per project, in a session that can build and test.

1. **`<px>-core/Cargo.toml`:** depend on `kitchen-compute` and forward the features
   (`wgpu = ["kitchen-compute/wgpu"]`, …).
2. **Delete the merged code:** remove `device.rs` and the merged modules of `compute/`, and
   re-export from `compute/mod.rs`:
   ```rust
   pub use kitchen_compute::{bench, launch, limits, pinning, tune, *};
   ```
   dsp-kitchen keeps its own names as wrappers:

   | dsp-kitchen name | `kitchen_compute` name |
   |---|---|
   | `channels_samples` | `rows_cols` |
   | `per_sample` | `per_col` |
   | `per_channel` | `per_line` |
   | `sample_position` | `unit_col` |
   | `channel_position` | `unit_row` |
   | `row_position` | `cube_row` |

3. **Errors:** add `#[error(transparent)] Compute(#[from] kitchen_compute::ComputeError)` to the
   core error. dsp-kitchen's `device_elements` returned `DspError::InvalidConfig`, so callers that
   match on that variant must change.
4. **Environment variables:** call `kitchen_compute::set_env_prefix("DSPK")` (or `"LUCEM"`) in
   `<px>-core::init()`, and call `init()` first in every front end: CLI, app, Python module, web
   worker.
5. **Constants:** the old `RUNTIME_ENV`-style constants become
   `kitchen_compute::env_name(kitchen_compute::env::RUNTIME)`.
6. **Test:** run the full suite on every runtime (`<PREFIX>_TEST_RUNTIMES=cuda`, `wgpu`, `cpu`),
   including the exit tests, which re-run the test binary.
