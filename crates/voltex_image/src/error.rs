//! Crate-wide error type and `Result` alias for `voltex_image`.

/// All errors produced by `voltex_image` (PNG/JPG/glTF/OBJ/JSON parsing,
/// deflate inflation, PNG encoding).
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for ImageError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for ImageError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, ImageError>`.
pub type Result<T> = std::result::Result<T, ImageError>;
