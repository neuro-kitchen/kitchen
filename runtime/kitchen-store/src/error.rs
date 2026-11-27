//! Errors of the artifact layer.

/// What went wrong.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    /// Reading or writing an artifact or its provenance.
    #[error("{path}: {source}")]
    Io {
        /// The file or store.
        path: String,
        /// What the system said.
        source: std::io::Error,
    },
    /// A kind, identity or provenance that is not well formed.
    #[error("{0}")]
    Invalid(String),
}

impl ArtifactError {
    pub(crate) fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::Io { path: path.display().to_string(), source }
    }
}

/// A result of the artifact layer.
pub type ArtifactResult<T> = Result<T, ArtifactError>;
