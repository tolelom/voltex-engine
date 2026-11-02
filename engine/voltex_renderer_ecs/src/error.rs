//! voltex_renderer_ecs error type — M4 표준 패턴 (thiserror + Generic + Io).

use thiserror::Error;

/// All errors produced by `voltex_renderer_ecs` operations.
#[derive(Debug, Error)]
pub enum RendererEcsError {
    /// Catch-all string error (asset upload misses, ECS query mismatches).
    #[error("{0}")]
    Generic(String),
    /// IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Type alias for `Result<T, RendererEcsError>`.
pub type Result<T> = std::result::Result<T, RendererEcsError>;

impl From<String> for RendererEcsError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for RendererEcsError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}
