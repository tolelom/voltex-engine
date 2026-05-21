//! RenderItem + collect_render_items — World 를 iterate 하여 frame 데이터 생성.

use voltex_ecs::{
    propagate_transforms, Entity, MaterialHandle, MeshHandle, World, WorldTransform,
};
use voltex_math::Mat4;
use voltex_renderer::MaterialUniform;

use crate::assets::RenderAssets;

/// 한 엔티티의 한 프레임 렌더 데이터.
pub struct RenderItem {
    /// Source entity for this draw.
    pub entity: Entity,
    /// World-space model matrix (`WorldTransform` snapshot).
    pub model: Mat4,
    /// Phantom-typed handle into [`RenderAssets`](crate::RenderAssets).
    pub mesh: MeshHandle,
    /// Per-entity material uniform (already resolved from `MaterialHandle`).
    pub material: MaterialUniform,
}

/// `propagate_transforms` 를 먼저 호출한 뒤 WorldTransform + MeshHandle 이 있는
/// 엔티티를 모은다. Stale MeshHandle, MeshHandle 없는 엔티티는 skip.
/// MaterialHandle 누락/스테일이면 `MaterialUniform::new()` (white default).
pub fn collect_render_items(world: &mut World, assets: &RenderAssets) -> Vec<RenderItem> {
    propagate_transforms(world);

    let pairs: Vec<(Entity, WorldTransform, MeshHandle)> = world
        .query2::<WorldTransform, MeshHandle>()
        .into_iter()
        .map(|(e, wt, mh)| (e, *wt, *mh))
        .collect();

    let mut items = Vec::new();
    for (entity, wt, mh) in pairs {
        if assets.mesh(mh).is_none() {
            continue;
        }
        let material = world
            .get::<MaterialHandle>(entity)
            .and_then(|h| assets.material(*h).copied())
            .unwrap_or_default();

        items.push(RenderItem {
            entity,
            model: wt.0,
            mesh: mh,
            material,
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_asset::Handle;
    use voltex_ecs::{add_child, Transform};
    use voltex_math::{Vec3, Vec4};

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn empty_world_no_items() {
        let mut world = World::new();
        let assets = RenderAssets::new();
        let items = collect_render_items(&mut world, &assets);
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn entity_without_mesh_skipped() {
        let mut world = World::new();
        let assets = RenderAssets::new();

        let e1 = world.spawn();
        world.add(e1, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        // No MeshHandle → skip

        let items = collect_render_items(&mut world, &assets);
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn stale_mesh_handle_skipped() {
        let mut world = World::new();
        let assets = RenderAssets::new();

        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        // MeshHandle pointing to nothing in RenderAssets → assets.mesh() = None → skip
        world.add(e, MeshHandle(Handle::from_raw_parts(42, 0)));

        let items = collect_render_items(&mut world, &assets);
        assert_eq!(items.len(), 0, "stale mesh handle should cause the entity to be skipped");
    }

    #[test]
    fn hierarchy_accumulates_into_model() {
        // 3-level: root @ (1,0,0) → mid @ (0,2,0) → leaf @ (0,0,3)
        // leaf world origin = (1, 2, 3)
        //
        // No GPU available, so we can't insert real Mesh to make collect emit items.
        // Instead verify that collect_render_items invokes propagate_transforms
        // (WorldTransform appears on the leaf) and the accumulated matrix is correct.
        let mut world = World::new();
        let root = world.spawn();
        let mid = world.spawn();
        let leaf = world.spawn();

        world.add(root, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(mid, Transform::from_position(Vec3::new(0.0, 2.0, 0.0)));
        world.add(leaf, Transform::from_position(Vec3::new(0.0, 0.0, 3.0)));

        add_child(&mut world, root, mid);
        add_child(&mut world, mid, leaf);

        let assets = RenderAssets::new();
        let _ = collect_render_items(&mut world, &assets);

        let wt = world.get::<WorldTransform>(leaf).expect("leaf WorldTransform");
        let p = wt.0 * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 1.0), "x: {}", p.x);
        assert!(approx_eq(p.y, 2.0), "y: {}", p.y);
        assert!(approx_eq(p.z, 3.0), "z: {}", p.z);
    }
}
