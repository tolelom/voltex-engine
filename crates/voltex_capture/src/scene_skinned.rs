//! Skinned-mesh capture scene — the real GPU vertex-skinning path
//! (`create_skinned_mesh_pipeline` + `mesh_skinned.wgsl`) driven by a CPU
//! [`Skeleton`] pose. Renders a 2-joint vertical quad ("bending strip"): the
//! bottom edge is bound to the root joint, the top edge to a child joint. At
//! the bind pose the strip is straight; rotating the child joint bends the top
//! half. The GPU-gated test asserts (a) the rest pose draws something and
//! (b) the bent pose differs from it — i.e. the palette actually deforms
//! geometry on the GPU.

use voltex_math::{Quat, Vec3};
use voltex_renderer::gltf::{
    AnimationPath, GltfAnimation, GltfChannel, GltfNode, GltfSkin, Interpolation,
};
use voltex_renderer::{
    bone_palette_bind_group_layout, create_skinned_mesh_pipeline, skinned_camera_bind_group_layout,
    BonePalette, Camera, Skeleton, SkinnedMeshVertex,
};
use wgpu::util::DeviceExt;

use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Camera UBO matching `mesh_skinned.wgsl` group(0): `view_proj` + `model`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinCam {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
}

const IDENTITY4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// A 4-vertex vertical strip from y=0 (bound to joint 0) to y=2 (bound to
/// joint 1). Front face points +Z.
pub(crate) fn strip_vertices() -> ([SkinnedMeshVertex; 4], [u32; 6]) {
    let mk = |x: f32, y: f32, joint: u32| SkinnedMeshVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
        joints: [joint, 0, 0, 0],
        weights: [1.0, 0.0, 0.0, 0.0],
    };
    let verts = [
        mk(-0.5, 0.0, 0), // bottom-left  → root joint
        mk(0.5, 0.0, 0),  // bottom-right → root joint
        mk(0.5, 2.0, 1),  // top-right    → child joint
        mk(-0.5, 2.0, 1), // top-left     → child joint
    ];
    let indices = [0u32, 1, 2, 0, 2, 3];
    (verts, indices)
}

/// Two-joint skeleton: root at the origin, child joint pivoting at y=1. The
/// child's inverse-bind matrix is the inverse of its bind world (T(0,1,0)), so
/// the rest pose produces identity palettes (no deformation).
pub(crate) fn demo_skeleton() -> Skeleton {
    fn node(t: [f32; 3], children: Vec<usize>) -> GltfNode {
        GltfNode {
            name: None,
            children,
            translation: t,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            mesh: None,
            skin: None,
        }
    }
    let nodes = vec![node([0.0, 0.0, 0.0], vec![1]), node([0.0, 1.0, 0.0], vec![])];
    let skin = GltfSkin {
        name: None,
        joints: vec![0, 1],
        inverse_bind_matrices: vec![
            IDENTITY4,
            // inverse of T(0,1,0) = T(0,-1,0)
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

/// Bend animation: rotate the child joint (node 1) by `angle` radians about Z.
fn bend_animation(angle: f32) -> GltfAnimation {
    let q = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), angle);
    GltfAnimation {
        name: None,
        channels: vec![GltfChannel {
            target_node: 1,
            target_path: AnimationPath::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: vec![q.x, q.y, q.z, q.w],
        }],
    }
}

/// Render the skinned strip with the given bone palette. Returns RGBA8.
fn render_with_palette(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    palette: &BonePalette,
) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let cam_layout = skinned_camera_bind_group_layout(device);
    let palette_layout = bone_palette_bind_group_layout(device);
    let pipeline = create_skinned_mesh_pipeline(device, CAPTURE_FORMAT, &cam_layout, &palette_layout);

    // Camera at (0,1,5) looking down -Z at the strip's middle.
    let camera = Camera::new(Vec3::new(0.0, 1.0, 5.0), width as f32 / height as f32);
    let cam = SkinCam {
        view_proj: camera.view_projection().cols,
        model: IDENTITY4,
    };
    let cam_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("skin cam"),
        contents: bytemuck::bytes_of(&cam),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let palette_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("skin palette"),
        contents: bytemuck::bytes_of(palette),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("skin cam bg"),
        layout: &cam_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: cam_buf.as_entire_binding(),
        }],
    });
    let palette_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("skin palette bg"),
        layout: &palette_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: palette_buf.as_entire_binding(),
        }],
    });

    let (verts, indices) = strip_vertices();
    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("skin vbuf"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("skin ibuf"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("capture skinned"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("skinned pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &cam_bg, &[]);
        pass.set_bind_group(1, &palette_bg, &[]);
        pass.set_vertex_buffer(0, vbuf.slice(..));
        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

/// Render the strip at its rest (bind) pose — straight, undeformed.
pub fn render_skinned_rest(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let skel = demo_skeleton();
    render_with_palette(gpu, width, height, &skel.compute_palette())
}

/// Render the strip with the child joint bent `angle` radians about Z.
pub fn render_skinned_bent(gpu: &HeadlessGpu, width: u32, height: u32, angle: f32) -> Vec<u8> {
    let mut skel = demo_skeleton();
    skel.apply_animation(&bend_animation(angle), 0.0);
    render_with_palette(gpu, width, height, &skel.compute_palette())
}

/// Fixed 60° bend — signature matches the capture bin's shot table.
pub fn render_skinned_bent_demo(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    render_skinned_bent(gpu, width, height, std::f32::consts::FRAC_PI_3)
}

/// Two-keyframe bend (rest at t=0 → 60° about Z at t=1) so advancing the
/// animation clock visibly deforms the strip.
fn bend_animation_2kf() -> GltfAnimation {
    let s = (std::f32::consts::FRAC_PI_3 * 0.5).sin();
    let c = (std::f32::consts::FRAC_PI_3 * 0.5).cos();
    GltfAnimation {
        name: None,
        channels: vec![GltfChannel {
            target_node: 1,
            target_path: AnimationPath::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, s, c],
        }],
    }
}

/// Looping wave for the demo totem: child joint rotates -35° → +35° → -35°
/// about Z over 2s.
pub(crate) fn wave_animation() -> GltfAnimation {
    let q = |deg: f32| {
        let a = deg.to_radians() * 0.5;
        [0.0f32, 0.0, a.sin(), a.cos()]
    };
    let a = q(-35.0);
    let b = q(35.0);
    GltfAnimation {
        name: None,
        channels: vec![GltfChannel {
            target_node: 1,
            target_path: AnimationPath::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0, 2.0],
            values: vec![
                a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], a[0], a[1], a[2], a[3],
            ],
        }],
    }
}

/// Render the 2-joint strip as a **runtime** skinned entity through
/// `BasicRenderer` (the full ECS path: SkinnedMesh asset + SkinnedMeshHandle +
/// SkinnedPose advanced `anim_time` seconds). Proves the runtime renderer skins
/// on the GPU. Returns RGBA8.
pub fn render_skinned_runtime(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    anim_time: f32,
) -> Vec<u8> {
    render_skinned_runtime_mode(gpu, width, height, anim_time, false)
}

/// Like [`render_skinned_runtime`] but selects the colored (`pbr=false`) or
/// full PBR (`pbr=true`) skinned path.
pub fn render_skinned_runtime_mode(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    anim_time: f32,
    pbr: bool,
) -> Vec<u8> {
    use voltex_ecs::{propagate_transforms, Transform, World};
    use voltex_renderer::{LightUniform, SkinnedMesh};
    use voltex_renderer_ecs::{update_skinned_poses, BasicRenderer, RenderAssets, SkinnedPose};

    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);
    let mut renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 16);
    renderer.set_pbr_enabled(pbr);

    let mut assets = RenderAssets::new();
    let (verts, indices) = strip_vertices();
    let mesh_h = assets.add_skinned_mesh(SkinnedMesh::new(device, &verts, &indices));

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::new());
    world.add(e, mesh_h);
    world.add(e, SkinnedPose::new(demo_skeleton(), bend_animation_2kf()));

    // Advance the pose (dt=0 leaves it at the rest/bind pose).
    update_skinned_poses(&mut world, anim_time);
    propagate_transforms(&mut world);

    let camera = Camera::new(Vec3::new(0.0, 1.0, 5.0), width as f32 / height as f32);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("capture skinned_runtime"),
    });
    renderer.render(
        queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        LightUniform::new(),
    );
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

/// Render a skinned strip hovering over a ground plane with directional shadow
/// mapping, to verify skinned meshes cast into the shadow map. The ground is
/// the only static mesh (the shadow *receiver*); the bent skinned strip is the
/// only *caster*. With `shadows` on, the strip darkens a band of ground; with
/// it off, the ground is uniformly lit. `hide_caster` adds a `Hidden` component
/// so the strip neither draws nor casts — the control that proves any ground
/// darkening comes from the skinned caster specifically.
pub fn render_skinned_shadow_scene(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    shadows: bool,
    hide_caster: bool,
) -> Vec<u8> {
    use voltex_ecs::{propagate_transforms, MaterialHandle, MeshHandle, Transform, World};
    use voltex_renderer::{LightUniform, MaterialUniform, MeshVertex, SkinnedMesh};
    use voltex_renderer_ecs::{update_skinned_poses, BasicRenderer, Hidden, RenderAssets, SkinnedPose};

    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);
    let mut renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 8);
    renderer.set_shadows(shadows);

    let mut assets = RenderAssets::new();

    // Up-facing ground quad (receiver).
    let mk = |x: f32, z: f32| MeshVertex {
        position: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let gverts = [mk(-12.0, -12.0), mk(12.0, -12.0), mk(12.0, 12.0), mk(-12.0, 12.0)];
    let gidx = [0u32, 2, 1, 0, 3, 2];
    let ground_mesh = assets.add_mesh(voltex_renderer::Mesh::new(device, &gverts, &gidx));
    let gray = assets.add_material(MaterialUniform::with_params([0.7, 0.7, 0.7, 1.0], 0.0, 1.0));

    // Skinned strip (caster), scaled up and lifted above the ground, bent so it
    // is clearly a deformed shape.
    let (verts, indices) = strip_vertices();
    let mesh_h = assets.add_skinned_mesh(SkinnedMesh::new(device, &verts, &indices));

    let mut world = World::new();
    let ground = world.spawn();
    world.add(ground, Transform::from_position(Vec3::ZERO));
    world.add(ground, MeshHandle(ground_mesh.0));
    world.add(ground, MaterialHandle(gray.0));

    let strip = world.spawn();
    let mut st = Transform::from_position(Vec3::new(0.0, 2.0, 0.0));
    st.scale = Vec3::new(3.0, 3.0, 3.0);
    world.add(strip, st);
    world.add(strip, mesh_h);
    world.add(strip, SkinnedPose::new(demo_skeleton(), bend_animation_2kf()));
    if hide_caster {
        world.add(strip, Hidden);
    }

    update_skinned_poses(&mut world, 1.0);
    propagate_transforms(&mut world);

    // Camera looks down at the ground from a corner; light angled so the strip's
    // shadow falls beside it onto open ground.
    let mut camera = Camera::new(Vec3::new(7.0, 7.0, 7.0), width as f32 / height as f32);
    camera.yaw = -0.785;
    camera.pitch = -0.615;
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.2];
    light.ambient_strength = 0.12;

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("skinned shadow") });
    renderer.render(
        queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    );
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    /// Count pixels whose RGB differs by more than `tol` between two frames.
    fn diff_pixels(a: &[u8], b: &[u8], tol: i32) -> usize {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(pa, pb)| {
                (pa[0] as i32 - pb[0] as i32).abs() > tol
                    || (pa[1] as i32 - pb[1] as i32).abs() > tol
                    || (pa[2] as i32 - pb[2] as i32).abs() > tol
            })
            .count()
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn load_skinned_gltf_builds_mesh_and_pose() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        use voltex_renderer::gltf::{GltfData, GltfMesh, GltfNode, GltfSkin};
        use voltex_renderer::MeshVertex;
        use voltex_renderer_ecs::{load_skinned_gltf, RenderAssets};

        let node = |t: [f32; 3], children: Vec<usize>| GltfNode {
            name: None,
            children,
            translation: t,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            mesh: None,
            skin: None,
        };
        let data = GltfData {
            meshes: vec![GltfMesh {
                vertices: vec![MeshVertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                }],
                indices: vec![0],
                name: None,
                material: None,
                joints: Some(vec![[1u16, 0, 0, 0]]),
                weights: Some(vec![[1.0f32, 0.0, 0.0, 0.0]]),
            }],
            nodes: vec![node([0.0, 0.0, 0.0], vec![1]), node([0.0, 1.0, 0.0], vec![])],
            skins: vec![GltfSkin {
                name: None,
                joints: vec![0, 1],
                inverse_bind_matrices: vec![IDENTITY4, IDENTITY4],
                skeleton: None,
            }],
            animations: vec![],
        };

        let mut assets = RenderAssets::new();
        let (handle, pose) =
            load_skinned_gltf(&gpu.device, &mut assets, &data).expect("skinned glTF loads");
        assert_eq!(pose.skeleton.joint_count(), 2, "two joints");
        assert!(assets.skinned_mesh(handle).is_some(), "skinned mesh uploaded");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn runtime_basic_renderer_skins_and_animates() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: no GPU adapter ({e})");
                return;
            }
        };
        let (w, h) = (256, 256);
        // Drive the skinned strip through BasicRenderer (the runtime path) at
        // rest vs animated — the GPU-skinned geometry must differ.
        let rest = render_skinned_runtime(&gpu, w, h, 0.0);
        let posed = render_skinned_runtime(&gpu, w, h, 1.0);
        assert!(
            !ImageStats::analyze(&rest).looks_blank(),
            "runtime skinned strip should render"
        );
        let changed = diff_pixels(&rest, &posed, 16);
        assert!(
            changed > 150,
            "animating the runtime skinned entity should change pixels, got {changed}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn runtime_skinned_pbr_path_renders_and_animates() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: no GPU adapter ({e})");
                return;
            }
        };
        let (w, h) = (256, 256);
        // The PBR skinned pipeline must compile + render, deform with the pose,
        // and shade differently from the colored skinned path.
        let pbr_rest = render_skinned_runtime_mode(&gpu, w, h, 0.0, true);
        let pbr_posed = render_skinned_runtime_mode(&gpu, w, h, 1.0, true);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}skinned_pbr.png"), w, h, &pbr_posed).unwrap();

        assert!(
            !ImageStats::analyze(&pbr_rest).looks_blank(),
            "PBR skinned strip should render"
        );
        assert!(
            diff_pixels(&pbr_rest, &pbr_posed, 16) > 150,
            "PBR skinned entity should deform with the pose"
        );
        // PBR shading should differ from the colored skinned path on the same pose.
        let colored_posed = render_skinned_runtime_mode(&gpu, w, h, 1.0, false);
        assert!(
            diff_pixels(&colored_posed, &pbr_posed, 16) > 100,
            "PBR skinned shading should differ from the colored skinned path"
        );
    }

    /// Count pixels that got darker (in luminance) in `a` vs `b` by more than
    /// `tol` — i.e. where a shadow fell.
    fn darkened_pixels(a: &[u8], b: &[u8], tol: i32) -> usize {
        let lum = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(pa, pb)| lum(pb) - lum(pa) > tol)
            .count()
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn skinned_mesh_casts_shadow() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: no GPU adapter ({e})");
                return;
            }
        };
        let (w, h) = (512, 512);

        // Strip present: shadows on vs off. The strip's own pixels don't receive
        // shadow (the skinned shader ignores the shadow map), so the darkening is
        // the ground shadow cast BY the skinned strip.
        let on = render_skinned_shadow_scene(&gpu, w, h, true, false);
        let off = render_skinned_shadow_scene(&gpu, w, h, false, false);
        let cast = darkened_pixels(&on, &off, 40);

        // Control: strip hidden → no caster. Any residual darkening (flat-ground
        // self-shadow) must be far smaller than the strip's cast shadow.
        let on_ctrl = render_skinned_shadow_scene(&gpu, w, h, true, true);
        let off_ctrl = render_skinned_shadow_scene(&gpu, w, h, false, true);
        let baseline = darkened_pixels(&on_ctrl, &off_ctrl, 40);

        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}skinned_shadow.png"), w, h, &on).unwrap();

        assert!(
            cast > 500,
            "skinned strip should cast a visible ground shadow, got {cast} darkened px"
        );
        assert!(
            cast > baseline * 4,
            "skinned cast shadow ({cast}) should dominate flat-ground baseline ({baseline})"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn skinned_bend_deforms_geometry() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: no GPU adapter ({e})");
                return;
            }
        };
        let (w, h) = (256, 256);

        // Rest pose must draw the strip (not a blank frame).
        let rest = render_skinned_rest(&gpu, w, h);
        let stats = ImageStats::analyze(&rest);
        assert!(!stats.looks_blank(), "rest skinned strip should not be blank");

        // Bent pose must differ from the rest pose — the GPU palette deformed
        // the top half of the strip.
        let bent = render_skinned_bent(&gpu, w, h, std::f32::consts::FRAC_PI_3);
        let changed = diff_pixels(&rest, &bent, 16);
        assert!(
            changed > 200,
            "bending the joint should change many pixels, got {changed}"
        );
    }
}
