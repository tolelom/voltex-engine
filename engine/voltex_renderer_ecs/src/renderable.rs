//! Declarative renderables — describe an entity's *shape* and *material* by
//! value in the GPU-less scene builder, and let the runtime materialize them
//! into real GPU meshes/materials once the device exists.
//!
//! This removes the two-phase friction (logical `scene_builder` vs GPU
//! `gpu_init`) for the most common operation in any engine: putting a visible
//! object in the world. Instead of spawning an entity, then re-finding it by
//! `Tag` in a separate GPU phase to attach `MeshHandle`/`MaterialHandle`, a
//! game writes everything in one place:
//!
//! ```ignore
//! let cube = world.spawn();
//! world.add(cube, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
//! world.add(cube, PrimitiveMesh::cube(1.0));
//! world.add(cube, MaterialDesc::color([0.2, 0.5, 1.0, 1.0]));
//! // …renders. No device, no MeshHandle, no Tag round-trip.
//! ```
//!
//! [`materialize_renderables`] (called once by `voltex_runtime` after the GPU
//! is ready, and idempotent on every later call) resolves these into GPU
//! resources, **deduplicating identical primitives and materials** so a
//! thousand identical cubes share one vertex buffer. It also attaches a
//! [`Bounds`] computed from the primitive so viewport picking works.

use std::collections::HashMap;

use voltex_ecs::{Bounds, Component, Entity, MaterialHandle, MeshHandle, World};
use voltex_math::{Vec3, AABB};
use voltex_renderer::material::MaterialUniform;
use voltex_renderer::mesh::Mesh;
use voltex_renderer::{
    generate_cube, generate_plane, generate_quad, generate_sphere, MeshVertex,
};

use crate::assets::RenderAssets;

/// A built-in primitive shape, described purely by value (no GPU). Attach this
/// to an entity and the runtime will create (and deduplicate) the GPU mesh for
/// you, plus a matching [`Bounds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveMesh {
    /// Axis-aligned cube of edge length `size`, centered at the origin.
    Cube {
        /// Edge length.
        size: f32,
    },
    /// Flat ground quad on the XZ plane (`+Y` normal), `size`×`size`.
    Plane {
        /// Side length.
        size: f32,
    },
    /// Upright quad on the XY plane (`+Z` normal) — walls, mesh billboards.
    Quad {
        /// Width along X.
        width: f32,
        /// Height along Y.
        height: f32,
    },
    /// UV sphere of the given `radius` and tessellation.
    Sphere {
        /// Sphere radius.
        radius: f32,
        /// Longitudinal segments.
        sectors: u32,
        /// Latitudinal segments.
        stacks: u32,
    },
}

impl PrimitiveMesh {
    /// A unit-ish cube of edge length `size`.
    pub fn cube(size: f32) -> Self {
        Self::Cube { size }
    }

    /// A flat XZ ground plane of side `size`.
    pub fn plane(size: f32) -> Self {
        Self::Plane { size }
    }

    /// An upright XY quad.
    pub fn quad(width: f32, height: f32) -> Self {
        Self::Quad { width, height }
    }

    /// A sphere with explicit tessellation.
    pub fn sphere(radius: f32, sectors: u32, stacks: u32) -> Self {
        Self::Sphere {
            radius,
            sectors,
            stacks,
        }
    }

    /// A sphere with a sensible default tessellation (32×16).
    pub fn sphere_default(radius: f32) -> Self {
        Self::sphere(radius, 32, 16)
    }

    /// The local-space axis-aligned bounding box of this primitive — used for
    /// the auto-attached [`Bounds`] (viewport picking).
    pub fn aabb(&self) -> AABB {
        match *self {
            PrimitiveMesh::Cube { size } => {
                let h = size * 0.5;
                AABB::new(Vec3::new(-h, -h, -h), Vec3::new(h, h, h))
            }
            PrimitiveMesh::Plane { size } => {
                let h = size * 0.5;
                AABB::new(Vec3::new(-h, 0.0, -h), Vec3::new(h, 0.0, h))
            }
            PrimitiveMesh::Quad { width, height } => {
                let hw = width * 0.5;
                let hh = height * 0.5;
                AABB::new(Vec3::new(-hw, -hh, 0.0), Vec3::new(hw, hh, 0.0))
            }
            PrimitiveMesh::Sphere { radius, .. } => {
                AABB::new(Vec3::new(-radius, -radius, -radius), Vec3::new(radius, radius, radius))
            }
        }
    }

    /// CPU geometry (vertices + indices) for this primitive, via the built-in
    /// generators in `voltex_renderer`.
    pub fn geometry(&self) -> (Vec<MeshVertex>, Vec<u32>) {
        match *self {
            PrimitiveMesh::Cube { size } => generate_cube(size),
            PrimitiveMesh::Plane { size } => generate_plane(size),
            PrimitiveMesh::Quad { width, height } => generate_quad(width, height),
            PrimitiveMesh::Sphere {
                radius,
                sectors,
                stacks,
            } => generate_sphere(radius, sectors, stacks),
        }
    }

    /// A hashable, exact key for deduplication. Two primitives that would
    /// generate byte-identical geometry share a key (floats compared by bit
    /// pattern, so e.g. `cube(1.0)` and `cube(1.0)` collapse to one mesh).
    fn dedup_key(&self) -> PrimitiveKey {
        match *self {
            PrimitiveMesh::Cube { size } => PrimitiveKey(0, size.to_bits(), 0, 0, 0),
            PrimitiveMesh::Plane { size } => PrimitiveKey(1, size.to_bits(), 0, 0, 0),
            PrimitiveMesh::Quad { width, height } => {
                PrimitiveKey(2, width.to_bits(), height.to_bits(), 0, 0)
            }
            PrimitiveMesh::Sphere {
                radius,
                sectors,
                stacks,
            } => PrimitiveKey(3, radius.to_bits(), sectors, stacks, 0),
        }
    }
}

/// Opaque dedup key (discriminant + up to four 32-bit param words).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PrimitiveKey(u8, u32, u32, u32, u32);

impl Component for PrimitiveMesh {
    const NAME: &'static str = "primitive_mesh";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let p = world.get::<PrimitiveMesh>(entity)?;
        let mut data = Vec::with_capacity(13);
        match *p {
            PrimitiveMesh::Cube { size } => {
                data.push(0);
                data.extend_from_slice(&size.to_le_bytes());
            }
            PrimitiveMesh::Plane { size } => {
                data.push(1);
                data.extend_from_slice(&size.to_le_bytes());
            }
            PrimitiveMesh::Quad { width, height } => {
                data.push(2);
                data.extend_from_slice(&width.to_le_bytes());
                data.extend_from_slice(&height.to_le_bytes());
            }
            PrimitiveMesh::Sphere {
                radius,
                sectors,
                stacks,
            } => {
                data.push(3);
                data.extend_from_slice(&radius.to_le_bytes());
                data.extend_from_slice(&sectors.to_le_bytes());
                data.extend_from_slice(&stacks.to_le_bytes());
            }
        }
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> voltex_ecs::Result<()> {
        let tag = *data.first().ok_or("PrimitiveMesh data empty")?;
        let f = |o: usize| -> voltex_ecs::Result<f32> {
            let b = data
                .get(o..o + 4)
                .ok_or("PrimitiveMesh data too short")?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let u = |o: usize| -> voltex_ecs::Result<u32> {
            let b = data
                .get(o..o + 4)
                .ok_or("PrimitiveMesh data too short")?;
            Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let prim = match tag {
            0 => PrimitiveMesh::Cube { size: f(1)? },
            1 => PrimitiveMesh::Plane { size: f(1)? },
            2 => PrimitiveMesh::Quad {
                width: f(1)?,
                height: f(5)?,
            },
            3 => PrimitiveMesh::Sphere {
                radius: f(1)?,
                sectors: u(5)?,
                stacks: u(9)?,
            },
            _ => return Err("PrimitiveMesh unknown tag".into()),
        };
        world.add(entity, prim);
        Ok(())
    }
}

/// A material described by value (a [`MaterialUniform`]). Attach this and the
/// runtime resolves it into a deduplicated GPU material + [`MaterialHandle`].
#[derive(Debug, Clone, Copy)]
pub struct MaterialDesc(pub MaterialUniform);

impl MaterialDesc {
    /// An opaque, fully-rough material of the given linear base color.
    pub fn color(rgba: [f32; 4]) -> Self {
        MaterialDesc(MaterialUniform::with_params(rgba, 0.0, 1.0))
    }

    /// A material with explicit metallic/roughness.
    pub fn pbr(rgba: [f32; 4], metallic: f32, roughness: f32) -> Self {
        MaterialDesc(MaterialUniform::with_params(rgba, metallic, roughness))
    }

    /// A hashable key over the material's parameters (bit-exact float compare).
    fn dedup_key(&self) -> [u32; 7] {
        let m = self.0;
        [
            m.base_color[0].to_bits(),
            m.base_color[1].to_bits(),
            m.base_color[2].to_bits(),
            m.base_color[3].to_bits(),
            m.metallic.to_bits(),
            m.roughness.to_bits(),
            m.ao.to_bits(),
        ]
    }
}

/// Serialized `MaterialDesc`: base_color(4) + metallic + roughness + ao, f32 LE.
const MATERIAL_DESC_FLOATS: usize = 7;
const MATERIAL_DESC_BYTES: usize = MATERIAL_DESC_FLOATS * 4;

impl Component for MaterialDesc {
    const NAME: &'static str = "material_desc";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let d = world.get::<MaterialDesc>(entity)?;
        let m = d.0;
        let mut data = Vec::with_capacity(MATERIAL_DESC_BYTES);
        for v in [
            m.base_color[0],
            m.base_color[1],
            m.base_color[2],
            m.base_color[3],
            m.metallic,
            m.roughness,
            m.ao,
        ] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> voltex_ecs::Result<()> {
        if data.len() < MATERIAL_DESC_BYTES {
            return Err("MaterialDesc data too short".into());
        }
        let mut f = [0.0f32; MATERIAL_DESC_FLOATS];
        for (i, slot) in f.iter_mut().enumerate() {
            let at = i * 4;
            *slot = f32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        }
        world.add(
            entity,
            MaterialDesc(MaterialUniform {
                base_color: [f[0], f[1], f[2], f[3]],
                metallic: f[4],
                roughness: f[5],
                ao: f[6],
                _padding: 0.0,
            }),
        );
        Ok(())
    }
}

/// Register the declarative renderable components ([`PrimitiveMesh`],
/// [`MaterialDesc`], [`TexturePath`](crate::TexturePath)) so they survive
/// scene serialization. `voltex_runtime` calls this automatically after
/// `register_defaults()`.
pub fn register_renderable_components(registry: &mut voltex_ecs::ComponentRegistry) {
    registry.register::<PrimitiveMesh>();
    registry.register::<MaterialDesc>();
    registry.register::<crate::TexturePath>();
    registry.register::<crate::MeshPath>();
}

/// Entities that declare a [`PrimitiveMesh`] but don't yet have a
/// [`MeshHandle`] — i.e. the ones [`materialize_renderables`] still needs to
/// build a GPU mesh for. Pure (no GPU); the idempotency contract is testable
/// here without a device.
pub fn plan_primitive_meshes(world: &World) -> Vec<(Entity, PrimitiveMesh)> {
    world
        .query::<PrimitiveMesh>()
        .map(|(e, p)| (e, *p))
        .filter(|(e, _)| world.get::<MeshHandle>(*e).is_none())
        .collect()
}

/// Entities that declare a [`MaterialDesc`] but don't yet have a
/// [`MaterialHandle`].
pub fn plan_materials(world: &World) -> Vec<(Entity, MaterialUniform)> {
    world
        .query::<MaterialDesc>()
        .map(|(e, d)| (e, d.0))
        .filter(|(e, _)| world.get::<MaterialHandle>(*e).is_none())
        .collect()
}

/// Resolve every declarative renderable in `world` into real GPU resources.
///
/// For each entity with a [`PrimitiveMesh`] and no [`MeshHandle`], builds (or
/// reuses, by dedup key) a GPU mesh in `assets`, then attaches the resulting
/// [`MeshHandle`] and a [`Bounds`]. For each entity with a [`MaterialDesc`] and
/// no [`MaterialHandle`], does the same for the material.
///
/// Idempotent: entities that already carry the resolved handle are skipped, so
/// calling this every frame (or after hot-reloading a scene) is safe and cheap.
pub fn materialize_renderables(world: &mut World, assets: &mut RenderAssets, device: &wgpu::Device) {
    // --- meshes ---
    let pending_meshes = plan_primitive_meshes(world);
    let mut mesh_cache: HashMap<PrimitiveKey, MeshHandle> = HashMap::new();
    for (entity, prim) in pending_meshes {
        let handle = *mesh_cache.entry(prim.dedup_key()).or_insert_with(|| {
            let (verts, indices) = prim.geometry();
            assets.add_mesh(Mesh::new(device, &verts, &indices))
        });
        world.add(entity, handle);
        world.add(entity, Bounds(prim.aabb()));
    }

    // --- materials ---
    let pending_materials = plan_materials(world);
    let mut mat_cache: HashMap<[u32; 7], MaterialHandle> = HashMap::new();
    for (entity, uniform) in pending_materials {
        let desc = MaterialDesc(uniform);
        let handle = *mat_cache
            .entry(desc.dedup_key())
            .or_insert_with(|| assets.add_material(uniform));
        world.add(entity, handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::ComponentRegistry;

    #[test]
    fn cube_aabb_is_centered_half_size() {
        let bb = PrimitiveMesh::cube(2.0).aabb();
        assert_eq!(bb.min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(bb.max, Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn sphere_aabb_spans_radius() {
        let bb = PrimitiveMesh::sphere_default(3.0).aabb();
        assert_eq!(bb.min, Vec3::new(-3.0, -3.0, -3.0));
        assert_eq!(bb.max, Vec3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn quad_aabb_is_flat_on_z() {
        let bb = PrimitiveMesh::quad(4.0, 2.0).aabb();
        assert_eq!(bb.min, Vec3::new(-2.0, -1.0, 0.0));
        assert_eq!(bb.max, Vec3::new(2.0, 1.0, 0.0));
    }

    #[test]
    fn plane_aabb_is_flat_on_y() {
        let bb = PrimitiveMesh::plane(10.0).aabb();
        assert_eq!(bb.min, Vec3::new(-5.0, 0.0, -5.0));
        assert_eq!(bb.max, Vec3::new(5.0, 0.0, 5.0));
    }

    #[test]
    fn cube_geometry_has_24_vertices() {
        let (verts, indices) = PrimitiveMesh::cube(1.0).geometry();
        assert_eq!(verts.len(), 24);
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn sphere_geometry_is_nonempty() {
        let (verts, indices) = PrimitiveMesh::sphere_default(1.0).geometry();
        assert!(!verts.is_empty());
        assert!(!indices.is_empty());
    }

    #[test]
    fn equal_primitives_share_dedup_key() {
        assert_eq!(
            PrimitiveMesh::cube(1.0).dedup_key(),
            PrimitiveMesh::cube(1.0).dedup_key()
        );
    }

    #[test]
    fn different_primitives_differ_in_dedup_key() {
        assert_ne!(
            PrimitiveMesh::cube(1.0).dedup_key(),
            PrimitiveMesh::cube(2.0).dedup_key()
        );
        assert_ne!(
            PrimitiveMesh::cube(1.0).dedup_key(),
            PrimitiveMesh::plane(1.0).dedup_key()
        );
    }

    #[test]
    fn plan_skips_entities_that_already_have_a_mesh_handle() {
        let mut world = World::new();
        let needs = world.spawn();
        world.add(needs, PrimitiveMesh::cube(1.0));
        let has = world.spawn();
        world.add(has, PrimitiveMesh::cube(1.0));
        world.add(has, MeshHandle(voltex_asset::Handle::from_raw_parts(5, 0)));

        let planned = plan_primitive_meshes(&world);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].0, needs);
    }

    #[test]
    fn plan_materials_skips_entities_that_already_have_a_material_handle() {
        let mut world = World::new();
        let needs = world.spawn();
        world.add(needs, MaterialDesc::color([1.0, 0.0, 0.0, 1.0]));
        let has = world.spawn();
        world.add(has, MaterialDesc::color([0.0, 1.0, 0.0, 1.0]));
        world.add(
            has,
            MaterialHandle(voltex_asset::Handle::from_raw_parts(9, 0)),
        );

        let planned = plan_materials(&world);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].0, needs);
    }

    #[test]
    fn primitive_mesh_serialize_round_trips() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, PrimitiveMesh::sphere(2.5, 24, 12));
        let bytes = PrimitiveMesh::serialize(&world, e).unwrap();

        let mut w2 = World::new();
        let e2 = w2.spawn();
        PrimitiveMesh::deserialize(&mut w2, e2, &bytes).unwrap();
        assert_eq!(*w2.get::<PrimitiveMesh>(e2).unwrap(), PrimitiveMesh::sphere(2.5, 24, 12));
    }

    #[test]
    fn material_desc_serialize_round_trips() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, MaterialDesc::pbr([0.2, 0.4, 0.8, 1.0], 0.3, 0.7));
        let bytes = MaterialDesc::serialize(&world, e).unwrap();

        let mut w2 = World::new();
        let e2 = w2.spawn();
        MaterialDesc::deserialize(&mut w2, e2, &bytes).unwrap();
        let got = w2.get::<MaterialDesc>(e2).unwrap();
        assert_eq!(got.0.base_color, [0.2, 0.4, 0.8, 1.0]);
        assert_eq!(got.0.metallic, 0.3);
        assert_eq!(got.0.roughness, 0.7);
    }

    #[test]
    fn register_renderable_components_does_not_panic() {
        let mut registry = ComponentRegistry::new();
        register_renderable_components(&mut registry);
    }

    #[test]
    fn declarative_renderables_round_trip_through_a_data_scene() {
        use voltex_ecs::{
            deserialize_scene_json, serialize_scene_json, Transform,
        };
        use voltex_math::Vec3;

        // A registry that knows the engine built-ins plus the renderable
        // components — exactly what RuntimeApp sets up on boot.
        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        register_renderable_components(&mut registry);

        // Author a "scene" in code, then serialize it to JSON (the on-disk
        // data format) — a cube + a sphere, each with a color.
        // Renderables carry a Transform (they need a position) — which is also
        // what `serialize_scene_json` uses as the entity anchor.
        let mut world = World::new();
        let cube = world.spawn();
        world.add(cube, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
        world.add(cube, PrimitiveMesh::cube(2.0));
        world.add(cube, MaterialDesc::color([0.9, 0.1, 0.1, 1.0]));
        let sphere = world.spawn();
        world.add(sphere, Transform::from_position(Vec3::new(-1.0, 0.0, 0.0)));
        world.add(sphere, PrimitiveMesh::sphere(0.5, 16, 8));
        world.add(sphere, MaterialDesc::pbr([0.1, 0.8, 0.2, 1.0], 0.5, 0.3));

        let json = serialize_scene_json(&world, &registry);

        // Load it into a fresh world purely from the data — no code attaches
        // the shapes/materials.
        let mut loaded = World::new();
        let entities = deserialize_scene_json(&mut loaded, &json, &registry).unwrap();
        assert_eq!(entities.len(), 2);

        // Both entities carry their declarative renderable through the data
        // round-trip; materialize_renderables would then make them visible
        // with zero game code.
        let prims: Vec<PrimitiveMesh> =
            loaded.query::<PrimitiveMesh>().map(|(_, p)| *p).collect();
        assert!(prims.contains(&PrimitiveMesh::cube(2.0)));
        assert!(prims.contains(&PrimitiveMesh::sphere(0.5, 16, 8)));

        let pending = plan_primitive_meshes(&loaded);
        assert_eq!(pending.len(), 2, "loaded scene primitives still need GPU meshes");
        assert_eq!(plan_materials(&loaded).len(), 2);
    }

    #[test]
    fn renderable_without_a_transform_is_not_serialized() {
        // Documents the scene format's Transform-anchor rule: an entity with a
        // renderable but no Transform is dropped entirely on save. Renderables
        // need a position anyway, so this only bites pathological scenes — but
        // it must not be silent, hence this guard.
        use voltex_ecs::{deserialize_scene_json, serialize_scene_json};

        let mut registry = ComponentRegistry::new();
        registry.register_defaults();
        register_renderable_components(&mut registry);

        let mut world = World::new();
        let e = world.spawn();
        world.add(e, PrimitiveMesh::cube(1.0)); // no Transform
        world.add(e, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));

        let json = serialize_scene_json(&world, &registry);
        let mut loaded = World::new();
        let entities = deserialize_scene_json(&mut loaded, &json, &registry).unwrap();
        assert!(
            entities.is_empty(),
            "a renderable without a Transform is not anchored in the scene"
        );
    }
}
