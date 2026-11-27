//! Which compute runtime runs the kernels.
//!
//! [`ComputeTarget`] is the one name for a runtime across a project: the CLI flag, Python argument,
//! app setting and the `<PREFIX>_RUNTIME` environment variable all resolve to it. Choosing a target
//! needs no GPU libraries; running work on it needs the `compute` feature. Which targets can run
//! depends on the enabled runtime features (`wgpu`, `cpu`, `cuda`, `hip`) and, for CUDA and HIP, on
//! the system's driver.

use serde::{Deserialize, Serialize};

use crate::ComputeError;
use crate::env::{RUNTIME, TEST_RUNTIMES, env_name, env_value};

/// A CubeCL runtime and its default device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComputeTarget {
    /// WebGPU (Vulkan / Metal / DX12).
    Wgpu,
    /// Multi-threaded CPU (MLIR JIT).
    Cpu,
    /// NVIDIA CUDA.
    Cuda,
    /// AMD ROCm HIP.
    Hip,
}

impl ComputeTarget {
    /// Every runtime, in preference order (GPU runtimes first).
    pub const ALL: [ComputeTarget; 4] = [Self::Wgpu, Self::Cuda, Self::Hip, Self::Cpu];

    pub fn name(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu",
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Hip => "hip",
        }
    }

    /// The runtime called `name` (case-insensitive; `webgpu`, `vulkan`, `metal`, `gpu` → wgpu,
    /// `rocm` → hip).
    pub fn parse(name: &str) -> Result<Self, ComputeError> {
        match name.trim().to_ascii_lowercase().as_str() {
            "wgpu" | "webgpu" | "vulkan" | "metal" | "gpu" => Ok(Self::Wgpu),
            "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda),
            "hip" | "rocm" => Ok(Self::Hip),
            other => Err(ComputeError::Unknown(other.to_string())),
        }
    }

    /// Whether this runtime is compiled in.
    pub fn is_available(self) -> bool {
        match self {
            Self::Wgpu => cfg!(feature = "wgpu"),
            Self::Cpu => cfg!(feature = "cpu"),
            Self::Cuda => cfg!(feature = "cuda"),
            Self::Hip => cfg!(feature = "hip"),
        }
    }

    /// Compiled-in runtimes in preference order.
    pub fn available() -> Vec<Self> {
        Self::ALL.into_iter().filter(|t| t.is_available()).collect()
    }

    /// Compiled in and its driver there. Opening CUDA or HIP without the driver panics on the
    /// runtime's device thread and takes the caller down, so they are asked of the system first:
    /// the NVIDIA kernel module (`/proc/driver/nvidia/version`, Linux) or AMD's (`/dev/kfd`).
    pub fn is_usable(self) -> bool {
        self.is_available() && self.unusable_reason().is_none()
    }

    /// Why a compiled-in runtime cannot run here (`None`: it can, as far as the system tells).
    pub fn unusable_reason(self) -> Option<&'static str> {
        match self {
            Self::Cuda
                if cfg!(target_os = "linux") && !std::path::Path::new("/proc/driver/nvidia/version").exists() =>
            {
                Some("the NVIDIA driver is not loaded")
            }
            Self::Hip if cfg!(target_os = "linux") && !std::path::Path::new("/dev/kfd").exists() => {
                Some("the AMD GPU driver (/dev/kfd) is not there")
            }
            _ => None,
        }
    }

    /// Usable runtimes in preference order.
    pub fn usable() -> Vec<Self> {
        Self::ALL.into_iter().filter(|t| t.is_usable()).collect()
    }

    /// The runtimes tests run on: [`Self::usable`], limited to the names in
    /// `<PREFIX>_TEST_RUNTIMES` when it is set. Compiled in is not the same as usable: a build with
    /// CUDA can run on a machine without an NVIDIA driver; those runtimes are skipped, and said so.
    ///
    /// # Panics
    /// If `<PREFIX>_TEST_RUNTIMES` names an unknown runtime (a typo would silently skip tests).
    pub fn for_tests() -> Vec<Self> {
        match env_value(TEST_RUNTIMES) {
            Some(list) => {
                let wanted: Vec<Self> = list
                    .split(',')
                    .map(|name| Self::parse(name).unwrap_or_else(|e| panic!("{}: {e}", env_name(TEST_RUNTIMES))))
                    .collect();
                Self::usable().into_iter().filter(|t| wanted.contains(t)).collect()
            }
            None => {
                let skipped: Vec<&str> =
                    Self::available().into_iter().filter_map(|t| t.unusable_reason().map(|_| t.name())).collect();
                if !skipped.is_empty() {
                    eprintln!("tests: skipping {} (no driver)", skipped.join(", "));
                }
                Self::usable()
            }
        }
    }

    /// The runtime this thread was told to use ([`with_target`]), else `<PREFIX>_RUNTIME` when
    /// set, else the first usable runtime.
    pub fn from_env() -> Result<Self, ComputeError> {
        if let Some(t) = THREAD_TARGET.with(|t| t.get()) {
            return t.checked();
        }
        match env_value(RUNTIME) {
            Some(name) => Self::parse(&name)?.checked(),
            None => Self::usable().first().copied().ok_or(ComputeError::NoneAvailable),
        }
    }

    /// `self` if it is compiled in and its driver is there, else an error saying which.
    pub fn checked(self) -> Result<Self, ComputeError> {
        if !self.is_available() {
            return Err(ComputeError::Unavailable(self, self.name()));
        }
        match self.unusable_reason() {
            Some(why) => Err(ComputeError::NoDriver(self, why)),
            None => Ok(self),
        }
    }
}

impl std::fmt::Display for ComputeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for ComputeTarget {
    type Err = ComputeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

thread_local! {
    /// The runtime [`with_target`] set for this thread.
    static THREAD_TARGET: std::cell::Cell<Option<ComputeTarget>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with [`ComputeTarget::from_env`] answering `target` on this thread: e.g. a server's
/// view processed on the CPU while a long job holds the GPU.
pub fn with_target<T>(target: ComputeTarget, f: impl FnOnce() -> T) -> T {
    let before = THREAD_TARGET.with(|t| t.replace(Some(target)));
    struct Restore(Option<ComputeTarget>);
    impl Drop for Restore {
        fn drop(&mut self) {
            THREAD_TARGET.with(|t| t.set(self.0));
        }
    }
    let _restore = Restore(before);
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_reports_missing_runtimes() {
        assert_eq!(ComputeTarget::parse("ROCm").unwrap(), ComputeTarget::Hip);
        assert_eq!(ComputeTarget::parse(" Vulkan ").unwrap(), ComputeTarget::Wgpu);
        assert!(ComputeTarget::parse("tpu").is_err());
        for target in ComputeTarget::ALL.into_iter().filter(|t| !t.is_available()) {
            assert!(matches!(target.checked(), Err(ComputeError::Unavailable(..))));
        }
    }

    #[test]
    fn a_thread_target_overrides_and_is_restored() {
        let Some(&target) = ComputeTarget::usable().first() else { return };
        with_target(target, || assert_eq!(ComputeTarget::from_env().unwrap(), target));
        assert!(THREAD_TARGET.with(|t| t.get()).is_none());
    }
}
