//! Errors surfaced by `voltex_runtime` boot/run paths.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("window init: {0}")]
    WindowInit(String),
    #[error("gpu init: {0}")]
    GpuInit(String),
    #[error("scene load: {0}")]
    SceneLoad(String),
    #[error("script error: {0}")]
    Script(#[from] voltex_script::ScriptError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
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
