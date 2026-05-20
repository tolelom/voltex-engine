//! Rendering data components.
//!
//! voltex_ecs (L1) 는 GPU 자원 타입 (voltex_renderer::Mesh, wgpu::Buffer) 에
//! 직접 의존할 수 없다. 대신 phantom marker 로 ID 공간만 분리하고,
//! cycle 2 의 voltex_renderer_ecs 가 marker → 실제 자원 매핑을 담당한다.

use voltex_asset::Handle;
use voltex_math::AABB;

/// Mesh asset ID 공간을 분리하는 phantom marker.
pub struct MeshAsset;

/// Material asset ID 공간을 분리하는 phantom marker.
pub struct MaterialAsset;

/// 한 엔티티가 어떤 메시 자원으로 렌더링되는지 가리키는 컴포넌트.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub Handle<MeshAsset>);

/// 한 엔티티가 어떤 머티리얼 자원을 사용하는지 가리키는 컴포넌트.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialHandle(pub Handle<MaterialAsset>);

/// 메시의 로컬 공간 AABB 캐시. 월드 공간 변환은 시스템 책임.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds(pub AABB);
