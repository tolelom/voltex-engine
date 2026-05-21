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

    /// Default MeshHandle = sentinel (id=0, gen=0). Renderer treats stale
    /// handles as "skip this entity" so AddComponent without a real mesh
    /// is non-fatal — user fills it in via Inspector later.
    fn default_bytes() -> Vec<u8> {
        vec![0u8; 8]
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

    /// Default MaterialHandle = sentinel (id=0, gen=0). Same rationale as MeshHandle.
    fn default_bytes() -> Vec<u8> {
        vec![0u8; 8]
    }
}

impl Component for Bounds {
    const NAME: &'static str = "bounds";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let b = world.get::<Bounds>(entity)?;
        let mut data = Vec::with_capacity(24);
        for &v in &[
            b.0.min.x, b.0.min.y, b.0.min.z,
            b.0.max.x, b.0.max.y, b.0.max.z,
        ] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 24 {
            return Err("Bounds data too short".into());
        }
        let f = |off: usize| {
            f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
        };
        let aabb = AABB::new(
            voltex_math::Vec3::new(f(0), f(4), f(8)),
            voltex_math::Vec3::new(f(12), f(16), f(20)),
        );
        world.add(entity, Bounds(aabb));
        Ok(())
    }

    /// Default Bounds = AABB { min: zero, max: zero } (degenerate). Inspector
    /// users set real bounds explicitly via mesh import / dedicated commands.
    fn default_bytes() -> Vec<u8> {
        vec![0u8; 24]
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

    #[test]
    fn test_bounds_roundtrip() {
        use voltex_math::Vec3;
        let mut world = World::new();
        let e = world.spawn();
        let aabb = AABB::new(Vec3::new(-1.5, -2.0, -3.25), Vec3::new(4.0, 5.5, 6.0));
        world.add(e, Bounds(aabb));

        let data = <Bounds as Component>::serialize(&world, e)
            .expect("serialize Bounds should return Some");
        assert_eq!(data.len(), 24);

        let mut world2 = World::new();
        let e2 = world2.spawn();
        <Bounds as Component>::deserialize(&mut world2, e2, &data).unwrap();

        let b = world2.get::<Bounds>(e2).expect("Bounds present");
        assert!((b.0.min.x - (-1.5)).abs() < 1e-6);
        assert!((b.0.min.y - (-2.0)).abs() < 1e-6);
        assert!((b.0.min.z - (-3.25)).abs() < 1e-6);
        assert!((b.0.max.x - 4.0).abs() < 1e-6);
        assert!((b.0.max.y - 5.5).abs() < 1e-6);
        assert!((b.0.max.z - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_bounds_deserialize_too_short() {
        let mut world = World::new();
        let e = world.spawn();
        let result = <Bounds as Component>::deserialize(&mut world, e, &[0u8; 23]);
        assert!(result.is_err());
    }
}
