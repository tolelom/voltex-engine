//! Crate-wide error type and `Result` alias for `voltex_audio`.

/// All errors produced by `voltex_audio` operations (decode, mix, backend IO).
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for AudioError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for AudioError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, AudioError>`.
pub type Result<T> = std::result::Result<T, AudioError>;
