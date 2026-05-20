//! voltex — Rust 게임 엔진 facade crate.
//!
//! 모든 sub-crate를 모듈 경로로 노출하며, 자주 쓰는 타입은 [`prelude`]에 모아둔다.
//!
//! # 사용 예
//!
//! ```ignore
//! use voltex::prelude::*;             // Vec3, World, Entity, Transform, ...
//! use voltex::renderer::GpuContext;   // 도메인 특화는 명시 경로
//! ```

pub use voltex_ai as ai;
pub use voltex_asset as asset;
pub use voltex_audio as audio;
pub use voltex_ecs as ecs;
pub use voltex_editor as editor;
pub use voltex_math as math;
pub use voltex_net as net;
pub use voltex_physics as physics;
pub use voltex_platform as platform;
pub use voltex_renderer as renderer;
pub use voltex_script as script;

/// 자주 쓰는 타입을 한 번에 가져온다.
///
/// 도메인 특화 타입(`GpuContext`, `RigidBody`, `Collider`, `LuaState` 등)은
/// 의도적으로 포함하지 않는다. 해당 도메인을 쓰는 코드는 `voltex::<domain>::Type`
/// 명시 경로로 import.
pub mod prelude {
    pub use voltex_math::{Vec2, Vec3, Vec4, Mat4, AABB, Ray};

    pub use voltex_ecs::{
        Children, Component, ComponentRegistry, Entity, Parent, Transform, World,
    };

    pub use voltex_platform::{GameTimer, InputState, VoltexWindow, WindowConfig};

    pub use voltex_asset::{Assets, Handle};
}

#[cfg(test)]
mod tests {
    /// prelude의 핵심 타입이 모두 import 가능한지 컴파일 타임 검증.
    #[test]
    fn prelude_imports_compile() {
        use crate::prelude::*;
        let _v: Vec3 = Vec3::ZERO;
        let _w = World::new();
        let _r = ComponentRegistry::new();
    }

    /// 모듈 경로 re-export가 작동하는지 검증.
    #[test]
    fn module_paths_resolve() {
        let _ = crate::math::Vec3::ZERO;
        let _ = crate::ecs::World::new();
    }
}
