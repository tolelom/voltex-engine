//! voltex_renderer_ecs — ECS World ↔ voltex_renderer 브릿지.
//!
//! - [`RenderAssets`] : `voltex_renderer::Mesh` 와 `MaterialUniform` 보관.
//!   C0a 의 phantom-typed `MeshHandle` / `MaterialHandle` 과 lockstep id/generation 유지.
//! - [`collect_render_items`] : World 를 iterate 하여 `RenderItem` 리스트 생성. 자체
//!   pipeline 을 쓰는 호출자가 평행 Vec 대신 사용.
//! - [`BasicRenderer`] : mesh_shader.wgsl 기반 pipeline + per-entity dynamic UBO 를 소유.
//!   `render(...)` 한 메서드로 World 를 그린다.

#![warn(missing_docs)]

pub mod error;
pub mod assets;
pub mod items;
pub mod basic_renderer;
pub mod skinned;

pub use error::{RendererEcsError, Result};
pub use assets::{RenderAssets, TextureHandle};
pub use items::{RenderItem, collect_render_items};
pub use basic_renderer::BasicRenderer;
pub use skinned::{animation_duration, update_skinned_poses, SkinnedMeshHandle, SkinnedPose};
