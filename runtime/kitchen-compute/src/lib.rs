//! The neuro-kitchen compute layer: the single entry point through which every project crate
//! reaches a CubeCL runtime.
//!
//! - [`ComputeTarget`]: the one name of a runtime across a project (CLI flag, Python argument, app
//!   setting, `<PREFIX>_RUNTIME`). Needs no GPU library; always available.
//! - [`limits`]: 32-bit indexing limits of device buffers (checked where sizes are chosen).
//! - [`env`]: the project's environment-variable prefix (`DSPK`, `LUCEM`, …), set once at start-up
//!   with [`set_env_prefix`].
//!
//! With the `compute` feature (any runtime feature turns it on):
//!
//! - [`ComputeTask`] / [`ComputeTarget::run`]: run generic work on the selected runtime;
//!   [`ComputeTarget::client`] for a client.
//! - [`configure_runtime`]: compiled kernels and autotune decisions persist across processes.
//! - [`shutdown_devices`], [`exit_now`], [`release_device_memory`]: device lifetime (GPU driver
//!   threads otherwise race the process teardown).
//! - [`LaunchGeometry`]: cube sizes and counts from the runtime's properties, with the position
//!   helpers kernels index with.
//! - [`tune`]: CubeCL autotuning for settings with no device-independent best value; [`pinning`]
//!   pins result-changing choices for reproducible runs.
//! - [`mod@bench`]: device-synchronised timing.
//!
//! Algorithm crates take a `Client` and use these; they never inspect or special-case the device.

#[cfg(feature = "compute")]
pub mod bench;
pub mod env;
mod error;
#[cfg(feature = "compute")]
pub mod launch;
pub mod limits;
#[cfg(feature = "compute")]
pub mod pinning;
#[cfg(feature = "compute")]
mod runtime;
#[cfg(feature = "compute")]
mod special;
mod target;
#[cfg(feature = "compute")]
pub mod tune;

pub use env::{env_name, env_prefix, set_env_prefix};
pub use error::ComputeError;
pub use limits::{MAX_DEVICE_ELEMENTS, device_elements};
pub use target::{ComputeTarget, with_target};

#[cfg(feature = "compute")]
pub use launch::{
    LaunchGeometry, RowShare, cube_row, element_position, line_position, row_share, tile_point, unit_col, unit_row,
};
#[cfg(feature = "compute")]
pub use pinning::{PinnedChoices, pin_tuned_choices, tuned_choices_pinned};
#[cfg(all(feature = "compute", target_family = "wasm", feature = "wgpu"))]
pub use runtime::open_browser_gpu;
#[cfg(feature = "compute")]
pub use runtime::{
    ComputeTask, browser_gpu_lost, configure_runtime, exit_now, open_device, release_device_memory, shutdown_devices,
};
#[cfg(feature = "compute")]
pub use special::{NEGATIVE_INFINITY_BITS, POSITIVE_INFINITY_BITS, negative_infinity, positive_infinity};

/// CubeCL, re-exported so every crate of a project uses the version kitchen-compute was built with.
#[cfg(feature = "compute")]
pub use cubecl;
