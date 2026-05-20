//! voltex_renderer_ecs error type — M4 표준 패턴 (thiserror + Generic + Io).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RendererEcsError {
    #[error("{0}")]
    Generic(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

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
