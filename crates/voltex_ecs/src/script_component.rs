//! Per-entity Lua script attachment.
//!
//! `ScriptComponent` is a tiny marker that ties an entity to a Lua script
//! file. The actual interpreter lives in `voltex_script::ScriptInstancePool` —
//! this component stores just the path and the runtime instance id. When the
//! script system encounters a component whose `instance_id == UNBOUND` (or
//! whose id no longer maps to a live instance, e.g. after deserialise), it
//! re-resolves by path on the next start/update pass.

use crate::component_registry::Component;
use crate::entity::Entity;
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptComponent {
    pub script_path: String,
    pub instance_id: u32,
}

impl ScriptComponent {
    /// Sentinel for "no instance bound yet" — the script system resolves the
    /// real id on first start/update.
    pub const UNBOUND: u32 = u32::MAX;

    pub fn new(path: impl Into<String>) -> Self {
        Self {
            script_path: path.into(),
            instance_id: Self::UNBOUND,
        }
    }
}

impl Component for ScriptComponent {
    const NAME: &'static str = "script";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let s = world.get::<ScriptComponent>(entity)?;
        let path_bytes = s.script_path.as_bytes();
        let mut data = Vec::with_capacity(4 + path_bytes.len());
        data.extend_from_slice(&s.instance_id.to_le_bytes());
        data.extend_from_slice(path_bytes);
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 4 {
            return Err("ScriptComponent data too short".into());
        }
        let instance_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let script_path = std::str::from_utf8(&data[4..])
            .map_err(|_| crate::EcsError::from("Invalid UTF-8 in script path"))?
            .to_string();
        world.add(entity, ScriptComponent { script_path, instance_id });
        Ok(())
    }

    /// Default ScriptComponent = UNBOUND instance + path
    /// `"scripts/spinner.lua"`. The Inspector's "+ script" button needs a
    /// runnable placeholder so the user can verify the script system is
    /// wired up correctly — every `voltex new` scaffold ships a
    /// `scripts/spinner.lua` that just rotates the entity (v1.0.1 fix; was
    /// 4 bytes of UNBOUND-only with no path, which made the button a no-op).
    ///
    /// Users replace the path via the Inspector's path text input (v1.1) or
    /// by editing the scene file. The script system silently skips missing
    /// files, so the default is safe even if the user's project lacks
    /// `scripts/spinner.lua`.
    fn default_bytes() -> Vec<u8> {
        let path = b"scripts/spinner.lua";
        let mut out = Vec::with_capacity(4 + path.len());
        out.extend_from_slice(&Self::UNBOUND.to_le_bytes());
        out.extend_from_slice(path);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_path_and_instance_id() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, ScriptComponent {
            script_path: "scripts/spinner.lua".to_string(),
            instance_id: 42,
        });

        let data = <ScriptComponent as Component>::serialize(&world, e)
            .expect("serialize should return Some");

        let mut world2 = World::new();
        let e2 = world2.spawn();
        <ScriptComponent as Component>::deserialize(&mut world2, e2, &data).unwrap();

        let s = world2.get::<ScriptComponent>(e2).expect("ScriptComponent present");
        assert_eq!(s.script_path, "scripts/spinner.lua");
        assert_eq!(s.instance_id, 42);
    }

    #[test]
    fn roundtrip_handles_unbound_sentinel() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, ScriptComponent::new("a.lua"));

        let data = <ScriptComponent as Component>::serialize(&world, e).unwrap();
        let mut world2 = World::new();
        let e2 = world2.spawn();
        <ScriptComponent as Component>::deserialize(&mut world2, e2, &data).unwrap();
        let s = world2.get::<ScriptComponent>(e2).unwrap();
        assert_eq!(s.instance_id, ScriptComponent::UNBOUND);
        assert_eq!(s.script_path, "a.lua");
    }

    #[test]
    fn deserialize_too_short_errors() {
        let mut world = World::new();
        let e = world.spawn();
        let result = <ScriptComponent as Component>::deserialize(&mut world, e, &[0u8; 3]);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_invalid_utf8_errors() {
        let mut world = World::new();
        let e = world.spawn();
        // 4 valid bytes for instance_id, then invalid UTF-8 0xFF
        let data = [0u8, 0, 0, 0, 0xFF, 0xFE];
        let result = <ScriptComponent as Component>::deserialize(&mut world, e, &data);
        assert!(result.is_err());
    }

    // ── v1.0.1: default_bytes regression ─────────────────────────────────

    #[test]
    fn default_bytes_round_trips_to_unbound_with_placeholder_path() {
        // Prior to v1.0.1 default_bytes was just 4 UNBOUND bytes with no
        // path — Inspector's "+ script" produced a script component that
        // pointed at "" and was therefore a no-op. The fix encodes a
        // placeholder path so the user can verify the script system is wired.
        let bytes = <ScriptComponent as Component>::default_bytes();
        assert!(bytes.len() > 4, "default_bytes must carry a path payload");
        let mut world = World::new();
        let e = world.spawn();
        <ScriptComponent as Component>::deserialize(&mut world, e, &bytes).unwrap();
        let s = world.get::<ScriptComponent>(e).expect("ScriptComponent attached");
        assert_eq!(s.instance_id, ScriptComponent::UNBOUND);
        assert_eq!(s.script_path, "scripts/spinner.lua");
    }
}
