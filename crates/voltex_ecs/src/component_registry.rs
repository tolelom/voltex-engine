//! Component registration for scene formats.
//!
//! **표준 경로 (M5 이후)**: `Component` trait를 구현한 타입을 `register::<T>()` 로 등록.
//! 타입과 직렬화 함수가 분리될 수 없도록 묶인다.
//!
//! **Escape hatch**: trait 구현이 불가능한 외부 타입은 `register_raw(name, ser, deser)` 사용.

use crate::entity::Entity;
use crate::world::World;

/// Function pointer that serialises one component to bytes.
pub type SerializeFn = fn(&World, Entity) -> Option<Vec<u8>>;
/// Function pointer that deserialises bytes back into a component on `entity`.
pub type DeserializeFn = fn(&mut World, Entity, &[u8]) -> crate::Result<()>;
/// Function pointer that removes the component from `entity`. Returns `true`
/// if a component was actually present.
pub type RemoveFn = fn(&mut World, Entity) -> bool;
/// Function pointer that produces a "sensible default" byte payload used by
/// `ComponentRegistry`'s Add Component flows.
pub type DefaultBytesFn = fn() -> Vec<u8>;

/// 직렬화 가능한 컴포넌트가 구현하는 trait.
///
/// 이름과 ser/deser/remove/default_bytes 함수를 컴포넌트 타입 자체에 묶어
/// 단일 정의 위치를 만든다. `ComponentRegistry::register::<T>()` 로 한 줄에 등록할 수 있다.
///
/// 변경 시 주의: `NAME` 변경은 기존 .vscn / 바이너리 씬 파일 호환을 깨뜨린다.
pub trait Component: 'static + Sized {
    /// 직렬화 포맷에서 사용할 컴포넌트 이름.
    const NAME: &'static str;

    /// World에서 본 entity의 해당 컴포넌트를 바이트로 직렬화. 컴포넌트가 없으면 None.
    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>>;

    /// 바이트를 디시리얼라이즈해 entity에 add.
    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()>;

    /// Default impl: TypeId 기반으로 `Self` 컴포넌트를 entity 에서 제거. 있었으면 true.
    /// 특수한 cleanup 이 필요한 컴포넌트만 오버라이드 (E3).
    fn remove(world: &mut World, entity: Entity) -> bool {
        world.remove::<Self>(entity).is_some()
    }

    /// "Sensible default" 바이트 — `deserialize` 에 넣었을 때 의미 있는 default
    /// 값으로 복원되어야 한다. Editor 의 "Add Component" UI 가 사용 (E3).
    /// Default impl 은 빈 Vec — 의미 있는 default 가 필요한 타입은 오버라이드.
    fn default_bytes() -> Vec<u8> {
        Vec::new()
    }
}

/// Registered metadata for one component type.
pub struct ComponentEntry {
    /// Name used in the on-disk scene format.
    pub name: String,
    /// Serialiser function pointer.
    pub serialize: SerializeFn,
    /// Deserialiser function pointer.
    pub deserialize: DeserializeFn,
    /// Remove function pointer.
    pub remove: RemoveFn,
    /// Default-bytes function pointer.
    pub default_bytes: DefaultBytesFn,
}

/// Holds every component type the runtime knows how to (de)serialise.
pub struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

impl ComponentRegistry {
    /// Creates an empty registry (no components registered yet).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 표준 경로: `Component` trait를 구현한 타입을 한 줄로 등록.
    pub fn register<T: Component>(&mut self) {
        self.entries.push(ComponentEntry {
            name: T::NAME.to_string(),
            serialize: T::serialize,
            deserialize: T::deserialize,
            remove: T::remove,
            default_bytes: T::default_bytes,
        });
    }

    /// Escape hatch: `Component` 구현이 불가능한 외부 타입을 위해 보존.
    /// `remove` 와 `default_bytes` 는 no-op stub 으로 채워지므로 Editor 의
    /// AddComponent UI 에는 노출되지 않는다.
    pub fn register_raw(&mut self, name: &str, ser: SerializeFn, deser: DeserializeFn) {
        self.entries.push(ComponentEntry {
            name: name.to_string(),
            serialize: ser,
            deserialize: deser,
            remove: |_, _| false,
            default_bytes: Vec::new,
        });
    }

    /// Finds the entry registered under `name`.
    pub fn find(&self, name: &str) -> Option<&ComponentEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Borrows the slice of every registered entry.
    pub fn entries(&self) -> &[ComponentEntry] {
        &self.entries
    }

    /// Engine built-in 컴포넌트(Transform, Tag, MeshHandle, MaterialHandle, Bounds, ScriptComponent) 등록.
    pub fn register_defaults(&mut self) {
        self.register::<crate::Transform>();
        self.register::<crate::scene::Tag>();
        self.register::<crate::MeshHandle>();
        self.register::<crate::MaterialHandle>();
        self.register::<crate::Bounds>();
        self.register::<crate::ScriptComponent>();
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Transform: 9 f32s in little-endian (pos.xyz, rot.xyz, scale.xyz) ─

impl Component for crate::Transform {
    const NAME: &'static str = "transform";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let t = world.get::<crate::Transform>(entity)?;
        let mut data = Vec::with_capacity(36);
        for &v in &[
            t.position.x,
            t.position.y,
            t.position.z,
            t.rotation.x,
            t.rotation.y,
            t.rotation.z,
            t.scale.x,
            t.scale.y,
            t.scale.z,
        ] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 36 {
            return Err("Transform data too short".into());
        }
        let f = |off: usize| {
            f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
        };
        let t = crate::Transform {
            position: voltex_math::Vec3::new(f(0), f(4), f(8)),
            rotation: voltex_math::Vec3::new(f(12), f(16), f(20)),
            scale: voltex_math::Vec3::new(f(24), f(28), f(32)),
        };
        world.add(entity, t);
        Ok(())
    }

    /// Default Transform = identity (pos/rot zero, scale = 1).
    fn default_bytes() -> Vec<u8> {
        let mut out = Vec::with_capacity(36);
        // position (3 × f32 = 0)
        out.extend_from_slice(&[0u8; 12]);
        // rotation (3 × f32 = 0)
        out.extend_from_slice(&[0u8; 12]);
        // scale (3 × f32 = 1)
        for _ in 0..3 {
            out.extend_from_slice(&1.0f32.to_le_bytes());
        }
        out
    }
}

// ── Tag: UTF-8 string bytes ─────────────────────────────────────────

impl Component for crate::scene::Tag {
    const NAME: &'static str = "tag";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let tag = world.get::<crate::scene::Tag>(entity)?;
        Some(tag.0.as_bytes().to_vec())
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        let s =
            std::str::from_utf8(data).map_err(|_| crate::EcsError::from("Invalid UTF-8 in tag"))?;
        world.add(entity, crate::scene::Tag(s.to_string()));
        Ok(())
    }

    /// Default Tag = empty string (deserialize handles 0-byte input).
    fn default_bytes() -> Vec<u8> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Transform, World};
    use voltex_math::Vec3;

    #[test]
    fn test_register_and_find() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert!(registry.find("transform").is_some());
        assert!(registry.find("tag").is_some());
        assert!(registry.find("nonexistent").is_none());
    }

    #[test]
    fn test_entries_count() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        // Transform + Tag + MeshHandle + MaterialHandle + Bounds + ScriptComponent = 6
        assert_eq!(registry.entries().len(), 6);
    }

    #[test]
    fn test_register_defaults_includes_render_components() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert!(registry.find("mesh_handle").is_some());
        assert!(registry.find("material_handle").is_some());
        assert!(registry.find("bounds").is_some());
    }

    #[test]
    fn test_register_defaults_includes_script() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        assert!(registry.find("script").is_some());
    }

    #[test]
    fn test_serialize_transform() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));

        let entry = registry.find("transform").unwrap();
        let data = (entry.serialize)(&world, e);
        assert!(data.is_some());
        assert_eq!(data.unwrap().len(), 36); // 9 f32s * 4 bytes
    }

    #[test]
    fn test_serialize_missing_component() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        // no Transform added

        let entry = registry.find("transform").unwrap();
        assert!((entry.serialize)(&world, e).is_none());
    }

    #[test]
    fn test_roundtrip_transform() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(
            e,
            Transform {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Vec3::new(0.1, 0.2, 0.3),
                scale: Vec3::new(4.0, 5.0, 6.0),
            },
        );

        let entry = registry.find("transform").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        let t = world2.get::<Transform>(e2).unwrap();
        assert!((t.position.x - 1.0).abs() < 1e-6);
        assert!((t.position.y - 2.0).abs() < 1e-6);
        assert!((t.position.z - 3.0).abs() < 1e-6);
        assert!((t.rotation.x - 0.1).abs() < 1e-6);
        assert!((t.scale.x - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_roundtrip_tag() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, crate::scene::Tag("hello world".to_string()));

        let entry = registry.find("tag").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        let tag = world2.get::<crate::scene::Tag>(e2).unwrap();
        assert_eq!(tag.0, "hello world");
    }

    #[test]
    fn test_deserialize_transform_too_short() {
        let mut world = World::new();
        let e = world.spawn();
        let result = <Transform as Component>::deserialize(&mut world, e, &[0u8; 10]);
        assert!(result.is_err());
    }

    // ── M5: 새 제네릭 경로 테스트 ──────────────────────────────────────

    /// 게임 측 커스텀 컴포넌트가 Component trait를 구현해 등록되는 시나리오.
    #[derive(Debug, Clone, PartialEq)]
    struct Health(u32);

    impl Component for Health {
        const NAME: &'static str = "health";

        fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
            let h = world.get::<Health>(entity)?;
            Some(h.0.to_le_bytes().to_vec())
        }

        fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
            if data.len() < 4 {
                return Err("Health data too short".into());
            }
            let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            world.add(entity, Health(val));
            Ok(())
        }
    }

    #[test]
    fn test_generic_register_user_component() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Health>();
        let entry = registry
            .find("health")
            .expect("health should be registered");
        assert_eq!(entry.name, "health");
    }

    #[test]
    fn test_generic_register_roundtrip() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Health>();

        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Health(42));

        let entry = registry.find("health").unwrap();
        let data = (entry.serialize)(&world, e).unwrap();

        let mut world2 = World::new();
        let e2 = world2.spawn();
        (entry.deserialize)(&mut world2, e2, &data).unwrap();

        assert_eq!(world2.get::<Health>(e2), Some(&Health(42)));
    }

    #[test]
    fn default_bytes_roundtrip_transform() {
        let bytes = <crate::Transform as Component>::default_bytes();
        assert_eq!(bytes.len(), 36);
        let mut world = World::new();
        let e = world.spawn();
        <crate::Transform as Component>::deserialize(&mut world, e, &bytes).unwrap();
        let t = world.get::<crate::Transform>(e).unwrap();
        // default scale = (1, 1, 1) — must be deserialised faithfully so AddComponent
        // produces a usable identity transform, not a degenerate (scale = 0) one.
        assert_eq!(t.scale, Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(t.position, Vec3::ZERO);
        assert_eq!(t.rotation, Vec3::ZERO);
    }

    #[test]
    fn default_bytes_roundtrip_tag() {
        let bytes = <crate::scene::Tag as Component>::default_bytes();
        let mut world = World::new();
        let e = world.spawn();
        <crate::scene::Tag as Component>::deserialize(&mut world, e, &bytes).unwrap();
        let tag = world.get::<crate::scene::Tag>(e).unwrap();
        assert_eq!(tag.0, "");
    }

    #[test]
    fn registry_remove_function_drops_component() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::new());
        assert!(world.get::<Transform>(e).is_some());

        let entry = registry.find("transform").unwrap();
        let removed = (entry.remove)(&mut world, e);
        assert!(removed);
        assert!(
            world.get::<Transform>(e).is_none(),
            "Transform must be gone after remove fn"
        );

        // Second remove is a no-op (returns false).
        let removed_again = (entry.remove)(&mut world, e);
        assert!(!removed_again);
    }

    #[test]
    fn registry_default_bytes_function_invokable() {
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        let entry = registry.find("transform").unwrap();
        let bytes = (entry.default_bytes)();
        assert_eq!(bytes.len(), 36);
    }

    #[test]
    fn register_raw_default_bytes_is_empty_and_remove_is_noop() {
        fn dummy_ser(_: &World, _: Entity) -> Option<Vec<u8>> {
            Some(vec![])
        }
        fn dummy_deser(_: &mut World, _: Entity, _: &[u8]) -> crate::Result<()> {
            Ok(())
        }
        let mut registry = ComponentRegistry::new();
        registry.register_raw("custom", dummy_ser, dummy_deser);
        let entry = registry.find("custom").unwrap();
        assert!((entry.default_bytes)().is_empty());
        let mut world = World::new();
        let e = world.spawn();
        assert!(!(entry.remove)(&mut world, e));
    }

    #[test]
    fn test_register_raw_still_works() {
        // Escape hatch가 여전히 동작하는지 검증.
        fn dummy_ser(_: &World, _: Entity) -> Option<Vec<u8>> {
            Some(vec![])
        }
        fn dummy_deser(_: &mut World, _: Entity, _: &[u8]) -> crate::Result<()> {
            Ok(())
        }

        let mut registry = ComponentRegistry::new();
        registry.register_raw("custom", dummy_ser, dummy_deser);
        assert!(registry.find("custom").is_some());
    }
}
