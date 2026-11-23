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
//!
//! Algorithm crates take a `Client` and use these; they never inspect or special-case the device.

pub mod env;
mod error;
pub mod limits;
#[cfg(feature = "compute")]
mod runtime;
mod target;

pub use env::{env_name, env_prefix, set_env_prefix};
pub use error::ComputeError;
pub use limits::{MAX_DEVICE_ELEMENTS, device_elements};
pub use target::{ComputeTarget, with_target};

#[cfg(all(feature = "compute", target_family = "wasm", feature = "wgpu"))]
pub use runtime::open_browser_gpu;
#[cfg(feature = "compute")]
pub use runtime::{
    ComputeTask, browser_gpu_lost, configure_runtime, exit_now, open_device, release_device_memory, shutdown_devices,
};

/// CubeCL, re-exported so every crate of a project uses the version kitchen-compute was built with.
#[cfg(feature = "compute")]
pub use cubecl;
