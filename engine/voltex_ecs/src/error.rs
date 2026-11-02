//! Crate-wide error type and `Result` alias for `voltex_ecs`.

/// All errors produced by `voltex_ecs` operations (scene serialization,
/// component registration, schedule construction).
#[derive(Debug, thiserror::Error)]
pub enum EcsError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for EcsError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for EcsError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, EcsError>`.
pub type Result<T> = std::result::Result<T, EcsError>;
