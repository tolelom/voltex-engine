use voltex_ecs::Entity;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditorError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// 이미 despawn된 entity에 대한 명령.
    #[error("entity not alive: id={} gen={}", .0.id, .0.generation)]
    EntityNotAlive(Entity),

    /// IO 에러 (파일 읽기/쓰기 실패 등).
    #[error("io: {0}")]
    Io(String),

    /// Parse 에러 (씬 파일 파싱 실패 등).
    #[error("parse: {0}")]
    Parse(String),
}

impl From<String> for EditorError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for EditorError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, EditorError>`.
pub type Result<T> = std::result::Result<T, EditorError>;
