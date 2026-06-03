//! Skinned-mesh ECS components + the per-frame animation system.
//!
//! - [`SkinnedMeshHandle`] — which GPU [`SkinnedMesh`] an entity draws (stored
//!   in [`RenderAssets`](crate::RenderAssets)).
//! - [`SkinnedPose`] — the entity's skeleton + animation track + playback
//!   clock. [`update_skinned_poses`] advances it each frame; the renderer reads
//!   [`SkinnedPose::palette`] to skin the mesh on the GPU.
//!
//! Unlike `MeshHandle` (a phantom-typed handle in `voltex_ecs`, which cannot
//! see `voltex_renderer`), this lives in `renderer_ecs` and wraps a concrete
//! `Handle<SkinnedMesh>`.

use voltex_asset::Handle;
use voltex_ecs::{Entity, World};
use voltex_renderer::gltf::{GltfAnimation, GltfData};
use voltex_renderer::{
    skinned_vertices_from_gltf, AnimationPlayer, BonePalette, Skeleton, SkinnedMesh,
};

use crate::assets::RenderAssets;

/// ECS component: the GPU skinned mesh an entity renders.
#[derive(Debug, Clone, Copy)]
pub struct SkinnedMeshHandle(pub Handle<SkinnedMesh>);

/// ECS component: a skeleton + the animation driving it + a playback clock.
pub struct SkinnedPose {
    /// Joint hierarchy + current pose (mutated by [`update_skinned_poses`]).
    pub skeleton: Skeleton,
    /// The animation track sampled into the skeleton each frame.
    pub animation: GltfAnimation,
    /// Playback clock (time / loop / speed).
    pub player: AnimationPlayer,
}

impl SkinnedPose {
    /// Build a pose that plays `animation` on `skeleton`, looping, from t=0.
    /// The player's duration is the animation's longest channel.
    pub fn new(skeleton: Skeleton, animation: GltfAnimation) -> Self {
        let duration = animation_duration(&animation);
        SkinnedPose {
            skeleton,
            animation,
            player: AnimationPlayer::new(duration),
        }
    }

    /// Current GPU bone palette for the skeleton's pose.
    pub fn palette(&self) -> BonePalette {
        self.skeleton.compute_palette()
    }
}

/// Longest keyframe timestamp across all of `anim`'s channels (its duration in
/// seconds). Returns 0 for an empty animation.
pub fn animation_duration(anim: &GltfAnimation) -> f32 {
    anim.channels
        .iter()
        .filter_map(|c| c.times.last().copied())
        .fold(0.0_f32, f32::max)
}

/// Build a renderable skinned entity from parsed glTF: upload the first mesh
/// primitive (with joints/weights) into `assets` and assemble a [`SkinnedPose`]
/// from the first skin + first animation. Returns the handle + pose to attach
/// to an entity, or `None` if the glTF has no mesh or no skin.
///
/// ```ignore
/// let data = voltex_renderer::parse_gltf(bytes)?;
/// if let Some((mesh, pose)) = load_skinned_gltf(device, &mut app.assets, &data) {
///     let e = app.world.spawn();
///     app.world.add(e, Transform::new());
///     app.world.add(e, mesh);
///     app.world.add(e, pose);
/// }
/// ```
pub fn load_skinned_gltf(
    device: &wgpu::Device,
    assets: &mut RenderAssets,
    data: &GltfData,
) -> Option<(SkinnedMeshHandle, SkinnedPose)> {
    let mesh = data.meshes.first()?;
    let skin = data.skins.first()?;
    let verts = skinned_vertices_from_gltf(mesh);
    let handle = assets.add_skinned_mesh(SkinnedMesh::new(device, &verts, &mesh.indices));
    let skeleton = Skeleton::from_skin(skin, &data.nodes);
    // No animation → an empty track leaves the skeleton at its bind pose.
    let animation = data.animations.first().cloned().unwrap_or(GltfAnimation {
        name: None,
        channels: Vec::new(),
    });
    Some((handle, SkinnedPose::new(skeleton, animation)))
}

/// Advance every [`SkinnedPose`] by `dt`: tick its player, then sample the
/// animation into the skeleton's local pose. No-op when `dt <= 0`.
pub fn update_skinned_poses(world: &mut World, dt: f32) {
    if dt <= 0.0 {
        return;
    }
    let entities: Vec<Entity> = world.query::<SkinnedPose>().map(|(e, _)| e).collect();
    for e in entities {
        if let Some(pose) = world.get_mut::<SkinnedPose>(e) {
            pose.player.update(dt);
            let t = pose.player.current_time;
            // Borrow-split the component: sample `animation` into `skeleton`.
            let SkinnedPose {
                skeleton,
                animation,
                ..
            } = pose;
            skeleton.apply_animation(animation, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_renderer::gltf::{
        AnimationPath, GltfAnimation, GltfChannel, GltfNode, GltfSkin, Interpolation,
    };
    use voltex_renderer::Skeleton;

    fn two_bone_skeleton() -> Skeleton {
        let nodes = vec![
            GltfNode {
                name: None,
                children: vec![1],
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                mesh: None,
                skin: None,
            },
            GltfNode {
                name: None,
                children: vec![],
                translation: [0.0, 1.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                mesh: None,
                skin: None,
            },
        ];
        let skin = GltfSkin {
            name: None,
            joints: vec![0, 1],
            inverse_bind_matrices: vec![
                [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
                [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, -1.0, 0.0, 1.0],
                ],
            ],
            skeleton: None,
        };
        Skeleton::from_skin(&skin, &nodes)
    }

    fn bend_anim() -> GltfAnimation {
        // Rotate child joint about Z over 0..1s (two keyframes).
        GltfAnimation {
            name: None,
            channels: vec![GltfChannel {
                target_node: 1,
                target_path: AnimationPath::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.383, 0.924],
            }],
        }
    }

    #[test]
    fn animation_duration_is_last_keyframe() {
        assert!((animation_duration(&bend_anim()) - 1.0).abs() < 1e-6);
        let empty = GltfAnimation { name: None, channels: vec![] };
        assert_eq!(animation_duration(&empty), 0.0);
    }

    #[test]
    fn update_advances_pose_and_changes_palette() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, SkinnedPose::new(two_bone_skeleton(), bend_anim()));

        let rest = world.get::<SkinnedPose>(e).unwrap().palette();

        // Advance halfway — the child joint should have rotated, changing the
        // child bone's palette matrix.
        update_skinned_poses(&mut world, 0.5);
        let posed = world.get::<SkinnedPose>(e).unwrap().palette();

        let mut diff = 0.0f32;
        for c in 0..4 {
            for r in 0..4 {
                diff += (rest.matrices[1][c][r] - posed.matrices[1][c][r]).abs();
            }
        }
        assert!(diff > 1e-3, "child bone palette should change after animating, diff={diff}");
    }

    #[test]
    fn zero_dt_is_noop() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, SkinnedPose::new(two_bone_skeleton(), bend_anim()));
        let t0 = world.get::<SkinnedPose>(e).unwrap().player.current_time;
        update_skinned_poses(&mut world, 0.0);
        let t1 = world.get::<SkinnedPose>(e).unwrap().player.current_time;
        assert_eq!(t0, t1);
    }
}
