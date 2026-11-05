//! Gizmo overlay capture — renders a cube via BasicRenderer, then overlays the
//! editor's translate/rotate/scale manipulation handles
//! (`voltex_editor::GizmoRenderer`). The editor's gizmos are central to the
//! editor workflow and have never been visually verified.

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_editor::{GizmoDraw, GizmoMode, GizmoRenderer};
use voltex_math::Vec3;
use voltex_renderer::{Camera, LightUniform, MaterialUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use super::unit_cube;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT, DEPTH_FORMAT};

/// Look-at camera: position the camera and orient it toward `target`.
fn look_at_camera(pos: Vec3, target: Vec3, aspect: f32) -> Camera {
    let mut cam = Camera::new(pos, aspect);
    let f = (target - pos).normalize();
    cam.pitch = f.y.asin();
    cam.yaw = f.x.atan2(-f.z);
    cam
}

fn render_with_mode(gpu: &HeadlessGpu, width: u32, height: u32, mode: GizmoMode) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 8);
    let gizmo = GizmoRenderer::new(device, CAPTURE_FORMAT, DEPTH_FORMAT);

    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(device));
    let mat_h = assets.add_material(MaterialUniform::new());

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));

    let camera = look_at_camera(
        Vec3::new(2.5, 2.0, 3.5),
        Vec3::ZERO,
        width as f32 / height as f32,
    );
    let view_proj = camera.view_projection();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gizmo capture"),
    });
    // Base pass: cube (clears color + depth).
    renderer.render_frame(
        queue,
        &mut encoder,
        voltex_renderer_ecs::FrameTarget {
            color_view: &target.color_view,
            depth_view: &target.depth_view,
            clear_color: wgpu::Color {
                r: 0.08,
                g: 0.09,
                b: 0.12,
                a: 1.0,
            },
        },
        &mut world,
        &assets,
        &voltex_renderer_ecs::FrameView {
            view_proj,
            camera_pos: camera.position,
            light: LightUniform::new(),
        },
    );
    // Overlay: gizmo handles at the cube origin (loads color + depth).
    let draw = GizmoDraw { view_proj, origin: Vec3::ZERO, mode, active_axis: None };
    gizmo.render_to(queue, &mut encoder, &target.color_view, &target.depth_view, &draw);
    queue.submit(Some(encoder.finish()));

    target.read_rgba8(device, queue)
}

/// Translate gizmo (3 axis arrows) over a cube.
pub fn render_gizmo_translate(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    render_with_mode(gpu, width, height, GizmoMode::Translate)
}

/// Rotate gizmo (3 axis rings) over a cube.
pub fn render_gizmo_rotate(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    render_with_mode(gpu, width, height, GizmoMode::Rotate)
}

/// Scale gizmo (3 axis boxes) over a cube.
pub fn render_gizmo_scale(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    render_with_mode(gpu, width, height, GizmoMode::Scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    #[test]
    #[ignore]
    fn gizmo_overlay_adds_colored_handles() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        // The gizmo axes are pure R/G/B lines, so the overlay must introduce
        // colors the plain cube scene lacks.
        let plain = ImageStats::analyze(&render_with_mode(&gpu, 400, 400, GizmoMode::Translate));
        assert!(!plain.looks_blank(), "gizmo scene blank: {plain:?}");
        assert!(
            plain.distinct_colors > 3,
            "no gizmo handles drawn: {plain:?}"
        );
    }
}
