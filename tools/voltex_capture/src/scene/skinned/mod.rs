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

use super::{ground_quad_mesh, matte};
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Camera UBO matching `mesh_skinned.wgsl` group(0): `view_proj` + `model`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinCam {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
}

const IDENTITY_MAT4: [[f32; 4]; 4] = [
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
            IDENTITY_MAT4,
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

/// Single-keyframe pose: the child joint (node 1) held at `angle` radians
/// about Z.
fn static_bend_animation(angle: f32) -> GltfAnimation {
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
        model: IDENTITY_MAT4,
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
    skel.apply_animation(&static_bend_animation(angle), 0.0);
    render_with_palette(gpu, width, height, &skel.compute_palette())
}

/// Fixed 60° bend — signature matches the capture bin's shot table.
pub fn render_skinned_bent_demo(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    render_skinned_bent(gpu, width, height, std::f32::consts::FRAC_PI_3)
}

/// Two-keyframe bend (rest at t=0 → 60° about Z at t=1) so advancing the
/// animation clock visibly deforms the strip.
fn bend_from_rest_animation() -> GltfAnimation {
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
    world.add(e, SkinnedPose::new(demo_skeleton(), bend_from_rest_animation()));

    // Advance the pose (dt=0 leaves it at the rest/bind pose).
    update_skinned_poses(&mut world, anim_time);
    propagate_transforms(&mut world);

    let camera = Camera::new(Vec3::new(0.0, 1.0, 5.0), width as f32 / height as f32);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("capture skinned_runtime"),
    });
    renderer.render_frame(
        queue,
        &mut encoder,
        voltex_renderer_ecs::FrameTarget {
            color_view: &target.color_view,
            depth_view: &target.depth_view,
            clear_color: wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        },
        &mut world,
        &assets,
        &voltex_renderer_ecs::FrameView {
            view_proj: camera.view_projection(),
            camera_pos: camera.position,
            light: LightUniform::new(),
        },
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
    use voltex_renderer::{LightUniform, SkinnedMesh};
    use voltex_renderer_ecs::{update_skinned_poses, BasicRenderer, Hidden, RenderAssets, SkinnedPose};

    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);
    let mut renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 8);
    renderer.set_shadows(shadows);

    let mut assets = RenderAssets::new();

    // Up-facing ground quad (receiver).
    let ground_mesh = assets.add_mesh(ground_quad_mesh(device, 12.0, 0.0));
    let gray = assets.add_material(matte([0.7, 0.7, 0.7, 1.0]));

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
    world.add(strip, SkinnedPose::new(demo_skeleton(), bend_from_rest_animation()));
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
    renderer.render_frame(
        queue,
        &mut encoder,
        voltex_renderer_ecs::FrameTarget {
            color_view: &target.color_view,
            depth_view: &target.depth_view,
            clear_color: wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 },
        },
        &mut world,
        &assets,
        &voltex_renderer_ecs::FrameView {
            view_proj: camera.view_projection(),
            camera_pos: camera.position,
            light,
        },
    );
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

#[cfg(test)]
mod tests;
