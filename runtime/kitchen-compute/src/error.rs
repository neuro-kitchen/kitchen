use thiserror::Error;

use crate::ComputeTarget;

/// What can go wrong choosing a runtime or sizing a device buffer. A project's own error type wraps
/// it (`#[error(transparent)] Compute(#[from] kitchen_compute::ComputeError)`).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ComputeError {
    #[error("unknown compute runtime '{0}' (expected wgpu, cpu, cuda or hip)")]
    Unknown(String),
    #[error("compute runtime {0} is not compiled in (enable the `{1}` feature)")]
    Unavailable(ComputeTarget, &'static str),
    /// Compiled in, but the system lacks its driver (opening it would panic on the runtime's
    /// device thread and take the caller down).
    #[error("compute runtime {0} cannot run here: {1}")]
    NoDriver(ComputeTarget, &'static str),
    #[error("no compute runtime is compiled in (enable one of the wgpu, cpu, cuda, hip features)")]
    NoneAvailable,
    /// A buffer past the 32-bit index range of kernels ([`crate::limits`]).
    #[error(
        "{what} {dims:?} needs more than {limit} elements in one device buffer (kernels use 32-bit indices): use smaller batches"
    )]
    TooManyElements { what: String, dims: Vec<usize>, limit: usize },
}
