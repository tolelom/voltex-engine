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

use crate::component_registry::Component;
use crate::entity::Entity;
use crate::world::World;

impl Component for MeshHandle {
    const NAME: &'static str = "mesh_handle";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let h = world.get::<MeshHandle>(entity)?;
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&h.0.id().to_le_bytes());
        data.extend_from_slice(&h.0.generation().to_le_bytes());
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 8 {
            return Err("MeshHandle data too short".into());
        }
        let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let gen = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        world.add(entity, MeshHandle(Handle::from_raw_parts(id, gen)));
        Ok(())
    }
}

impl Component for MaterialHandle {
    const NAME: &'static str = "material_handle";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let h = world.get::<MaterialHandle>(entity)?;
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&h.0.id().to_le_bytes());
        data.extend_from_slice(&h.0.generation().to_le_bytes());
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 8 {
            return Err("MaterialHandle data too short".into());
        }
        let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let gen = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        world.add(entity, MaterialHandle(Handle::from_raw_parts(id, gen)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_registry::Component;
    use crate::world::World;

    #[test]
    fn test_mesh_handle_roundtrip() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, MeshHandle(Handle::from_raw_parts(42, 7)));

        let data = <MeshHandle as Component>::serialize(&world, e)
            .expect("serialize MeshHandle should return Some");

        let mut world2 = World::new();
        let e2 = world2.spawn();
        <MeshHandle as Component>::deserialize(&mut world2, e2, &data).unwrap();

        let h = world2.get::<MeshHandle>(e2).expect("MeshHandle present");
        assert_eq!(h.0.id(), 42);
        assert_eq!(h.0.generation(), 7);
    }

    #[test]
    fn test_mesh_handle_deserialize_too_short() {
        let mut world = World::new();
        let e = world.spawn();
        let result = <MeshHandle as Component>::deserialize(&mut world, e, &[0u8; 7]);
        assert!(result.is_err());
    }

    #[test]
    fn test_material_handle_roundtrip() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, MaterialHandle(Handle::from_raw_parts(99, 3)));

        let data = <MaterialHandle as Component>::serialize(&world, e)
            .expect("serialize MaterialHandle should return Some");

        let mut world2 = World::new();
        let e2 = world2.spawn();
        <MaterialHandle as Component>::deserialize(&mut world2, e2, &data).unwrap();

        let h = world2.get::<MaterialHandle>(e2).expect("MaterialHandle present");
        assert_eq!(h.0.id(), 99);
        assert_eq!(h.0.generation(), 3);
    }

    #[test]
    fn test_material_handle_deserialize_too_short() {
        let mut world = World::new();
        let e = world.spawn();
        let result = <MaterialHandle as Component>::deserialize(&mut world, e, &[0u8; 7]);
        assert!(result.is_err());
    }
}
