//! voltex — Rust 게임 엔진 facade crate.
//!
//! 모든 sub-crate 를 모듈 경로로 노출하며, 자주 쓰는 타입은 [`prelude`] 에
//! 모아둔다. 새 코드는 거의 항상 `use voltex::prelude::*;` 한 줄로 시작한다.
//!
//! # 사용 예
//!
//! ```ignore
//! use voltex::prelude::*;             // Vec3, World, Entity, Transform, ...
//! use voltex::renderer::GpuContext;   // 도메인 특화는 명시 경로
//! use voltex::runtime::RuntimeApp;    // 부트스트랩 + 메인 루프
//! ```
//!
//! # 도메인 별 진입점
//!
//! - 수학: [`math`] — `Vec3`, `Mat4`, `AABB`, `Ray` 등
//! - ECS: [`ecs`] — `World`, `Entity`, `Component`, `Transform`
//! - 자원: [`asset`] — `Handle`, `Assets`, `Vfs`
//! - 렌더링: [`renderer`] + [`renderer_ecs`] — GPU + ECS 브릿지
//! - 런타임: [`runtime`] — `RuntimeApp` 부트스트랩
//! - 에디터: [`editor`] — `EditorSession`, `EditorCommand`
//! - 이미지 코덱: [`image`] — `parse_png`, `parse_jpg`, `parse_gltf`, `MeshVertex` (GPU 무의존 L0)
//! - 물리/오디오/AI/네트워킹/스크립트: [`physics`], [`audio`], [`ai`],
//!   [`net`], [`script`]

#![warn(missing_docs)]

pub use voltex_ai as ai;
pub use voltex_asset as asset;
pub use voltex_audio as audio;
pub use voltex_ecs as ecs;
pub use voltex_editor as editor;
pub use voltex_image as image;
pub use voltex_math as math;
pub use voltex_net as net;
pub use voltex_physics as physics;
pub use voltex_platform as platform;
pub use voltex_renderer as renderer;
pub use voltex_renderer_ecs as renderer_ecs;
pub use voltex_runtime as runtime;
pub use voltex_script as script;

/// Vfs 는 facade 루트에서도 직접 접근 가능 (assets 외 runtime 도 같은 타입 사용).
pub use voltex_asset::Vfs;

/// 자주 쓰는 타입을 한 번에 가져온다.
///
/// 도메인 특화 타입(`GpuContext`, `RigidBody`, `Collider`, `LuaState` 등)은
/// 의도적으로 포함하지 않는다. 해당 도메인을 쓰는 코드는 `voltex::<domain>::Type`
/// 명시 경로로 import.
///
/// 각 sub-crate 의 `Result<T>` alias 는 이름 충돌 회피를 위해 prelude 에서
/// 제외 — `voltex::ecs::Result`, `voltex::editor::Result` 등 명시 경로 사용.
pub mod prelude {
    // 수학 — 거의 모든 코드가 필요
    pub use voltex_math::{Mat4, Quat, Ray, Vec2, Vec3, Vec4, AABB};

    // ECS 핵심 + C0a 의 렌더링 컴포넌트 + S1 의 스크립트 컴포넌트
    pub use voltex_ecs::{
        Bounds, Children, Component, ComponentRegistry, Entity, MaterialHandle, MeshHandle, Parent,
        ScriptComponent, Transform, World,
    };

    // 플랫폼 (윈도우, 입력, 타이머)
    pub use voltex_platform::{GameTimer, InputState, VoltexWindow, WindowConfig};

    // 자원 핸들 + VFS (B2)
    pub use voltex_asset::{Assets, Handle, Vfs};

    // 게임에서 자주 붙이는 렌더링/게임플레이 컴포넌트
    // (2D 스택 + 자기정리 엔티티). 도메인 특화 타입과 달리 거의 모든 게임이 쓴다.
    pub use voltex_renderer_ecs::{Particle, Sprite, SpriteAnimation, TextureHandle};
    pub use voltex_runtime::{DebugLines, Lifetime};

    // 런타임 부트스트랩 — main 본문에서 가장 흔하게 쓰인다
    pub use voltex_runtime::{RuntimeApp, RuntimeConfig};
}

#[cfg(test)]
mod tests {
    /// prelude의 핵심 타입이 모두 import 가능한지 컴파일 타임 검증.
    #[test]
    fn prelude_imports_compile() {
        use crate::prelude::*;
        let _v: Vec3 = Vec3::ZERO;
        let _q: Quat = Quat::IDENTITY;
        let _w = World::new();
        let _r = ComponentRegistry::new();
        // E3 / C0a / S1 / B0 / B2 additions reachable via prelude:
        let _bounds: Option<Bounds> = None;
        let _mesh: Option<MeshHandle> = None;
        let _mat: Option<MaterialHandle> = None;
        let _script: Option<ScriptComponent> = None;
        let _cfg = RuntimeConfig::new();
        let _vfs: Option<Vfs> = None;
        // 2D stack + gameplay components reachable via prelude:
        let _sprite: Option<Sprite> = None;
        let _part: Option<Particle> = None;
        let _anim: Option<SpriteAnimation> = None;
        let _tex: Option<TextureHandle> = None;
        let _life: Option<Lifetime> = None;
        let _dbg: Option<DebugLines> = None;
    }

    /// 모듈 경로 re-export가 작동하는지 검증.
    #[test]
    fn module_paths_resolve() {
        let _ = crate::math::Vec3::ZERO;
        let _ = crate::ecs::World::new();
        let _ = crate::runtime::RuntimeConfig::new();
        // voltex_image facade re-export
        let _: Option<crate::image::MeshVertex> = None;
    }
}
