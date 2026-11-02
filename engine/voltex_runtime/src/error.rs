//! Errors surfaced by `voltex_runtime` boot/run paths.

use thiserror::Error;

/// Errors produced by [`RuntimeApp`](crate::RuntimeApp) construction or main loop.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// `winit` window creation failed; carries the underlying message.
    #[error("window init: {0}")]
    WindowInit(String),
    /// `wgpu` adapter/device init failed.
    #[error("gpu init: {0}")]
    GpuInit(String),
    /// Scene file could not be read or parsed.
    #[error("scene load: {0}")]
    SceneLoad(String),
    /// Lua script subsystem error (forwarded from `voltex_script`).
    #[error("script error: {0}")]
    Script(#[from] voltex_script::ScriptError),
    /// IO error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Catch-all string error.
    #[error("{0}")]
    Generic(String),
}

impl From<String> for RuntimeError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for RuntimeError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, RuntimeError>`.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_error_from_string() {
        let e: RuntimeError = "oops".to_string().into();
        assert!(matches!(e, RuntimeError::Generic(_)));
        assert_eq!(format!("{}", e), "oops");
    }

    #[test]
    fn runtime_error_from_str() {
        let e: RuntimeError = "boom".into();
        assert!(matches!(e, RuntimeError::Generic(_)));
    }
}
