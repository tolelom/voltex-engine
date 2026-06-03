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
fn strip_vertices() -> ([SkinnedMeshVertex; 4], [u32; 6]) {
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
fn demo_skeleton() -> Skeleton {
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
