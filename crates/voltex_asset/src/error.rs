#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for AssetError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for AssetError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, AssetError>`.
pub type Result<T> = std::result::Result<T, AssetError>;
