//! The neuro-kitchen compute layer: the single entry point through which every project crate
//! reaches a CubeCL runtime.
//!
//! - [`ComputeTarget`]: the one name of a runtime across a project (CLI flag, Python argument, app
//!   setting, `<PREFIX>_RUNTIME`). Needs no GPU library; always available.
//! - [`limits`]: 32-bit indexing limits of device buffers (checked where sizes are chosen).
//! - [`env`]: the project's environment-variable prefix (`DSPK`, `LUCEM`, …), set once at start-up
//!   with [`set_env_prefix`].

pub mod env;
mod error;
pub mod limits;
mod target;

pub use env::{env_name, env_prefix, set_env_prefix};
pub use error::ComputeError;
pub use limits::{MAX_DEVICE_ELEMENTS, device_elements};
pub use target::{ComputeTarget, with_target};
