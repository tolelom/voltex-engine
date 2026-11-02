//! Crate-wide error type and `Result` alias for `voltex_editor`.

use voltex_ecs::Entity;

/// All errors produced by `voltex_editor` operations (command dispatch,
/// scene IO, mode validation).
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

    /// Dispatch was attempted while the editor is in Play mode.
    /// Policy: Play mode is read-only from the user's command surface; only
    /// systems registered via lifecycle callbacks may mutate the World.
    #[error("dispatch rejected: current mode is {current:?}")]
    ModeMismatch {
        /// The editor mode that rejected the dispatch.
        current: crate::session::EditorMode,
    },
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
