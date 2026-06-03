//! Headless "playthrough" of `examples/game_ready_demo` — composes the same
//! scene (ground + blue player + red chaser + skinned totem + warm sun + two
//! colored point lights + follow camera) and renders a single frame through the
//! real `BasicRenderer`, with the skeleton advanced to time `t` and the
//! player/chaser positions scripted. This is what lets the demo's *visual*
//! output be verified offscreen (PNG) without running the interactive window.

use voltex_ecs::{propagate_transforms, MaterialHandle, MeshHandle, Transform, World};
use voltex_math::Vec3;
use voltex_renderer::{Camera, LightUniform, MaterialUniform, PointLight, SkinnedMesh};
use voltex_renderer_ecs::{update_skinned_poses, BasicRenderer, RenderAssets, SkinnedPose};

use crate::headless::HeadlessGpu;
use crate::scene::unit_cube;
use crate::scene_skinned::{demo_skeleton, strip_vertices, wave_animation};
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render one demo frame: skeleton at time `t` seconds, player at
/// `(player_x, 0.5, 0)`, chaser at `chaser_pos`. Follow camera frames the
/// player. Returns RGBA8.
pub fn render_demo_frame(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    t: f32,
    player_x: f32,
    chaser_pos: Vec3,
) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);
    let renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 64);

    let mut assets = RenderAssets::new();
    let cube = assets.add_mesh(unit_cube(device));
    let gray = assets.add_material(MaterialUniform::with_params([0.35, 0.37, 0.40, 1.0], 0.0, 1.0));
    let blue = assets.add_material(MaterialUniform::with_params([0.20, 0.50, 1.00, 1.0], 0.0, 1.0));
    let red = assets.add_material(MaterialUniform::with_params([1.00, 0.30, 0.20, 1.0], 0.0, 1.0));
    let (sv, si) = strip_vertices();
    let strip = assets.add_skinned_mesh(SkinnedMesh::new(device, &sv, &si));

    let mut world = World::new();

    // Ground.
    let ground = world.spawn();
    let mut gt = Transform::from_position(Vec3::new(0.0, -0.1, 0.0));
    gt.scale = Vec3::new(40.0, 0.2, 40.0);
    world.add(ground, gt);
    world.add(ground, MeshHandle(cube.0));
    world.add(ground, MaterialHandle(gray.0));

    // Player (blue).
    let player = world.spawn();
    world.add(player, Transform::from_position(Vec3::new(player_x, 0.5, 0.0)));
    world.add(player, MeshHandle(cube.0));
    world.add(player, MaterialHandle(blue.0));

    // Chaser (red, slightly smaller).
    let chaser = world.spawn();
    let mut ct = Transform::from_position(chaser_pos);
    ct.scale = Vec3::new(0.8, 0.8, 0.8);
    world.add(chaser, ct);
    world.add(chaser, MeshHandle(cube.0));
    world.add(chaser, MaterialHandle(red.0));

    // Skinned totem (waving), off to the left.
    let totem = world.spawn();
    let mut tt = Transform::from_position(Vec3::new(-5.0, 0.0, -3.0));
    tt.scale = Vec3::new(1.5, 1.5, 1.5);
    world.add(totem, tt);
    world.add(totem, strip);
    world.add(totem, SkinnedPose::new(demo_skeleton(), wave_animation()));

    // Advance the skeleton to time t.
    update_skinned_poses(&mut world, t);
    propagate_transforms(&mut world);

    // Follow camera (as in the demo).
    let mut camera = Camera::new(
        Vec3::new(player_x, 9.0, 13.0),
        width as f32 / height as f32,
    );
    camera.yaw = 0.0;
    camera.pitch = -0.55;

    // Warm sun + two colored arena point lights (as in the demo).
    let mut sun = LightUniform::new();
    sun.direction = [-0.5, -1.0, -0.4];
    sun.color = [1.0, 0.95, 0.85];
    sun.ambient_strength = 0.15;
    renderer.set_point_lights(
        queue,
        &[
            PointLight::new([6.0, 4.0, 6.0], [1.0, 0.6, 0.3], 18.0, 2.5),
            PointLight::new([-6.0, 4.0, -6.0], [0.3, 0.5, 1.0], 18.0, 2.5),
        ],
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("demo playthrough frame"),
    });
    renderer.render(
        queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        sun,
    );
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn demo_frames_render_nonblank() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Two scripted frames: start, then player moved + chaser closed in.
        let a = render_demo_frame(&gpu, 640, 360, 0.2, 0.0, Vec3::new(10.0, 0.5, 10.0));
        let b = render_demo_frame(&gpu, 640, 360, 1.0, 5.0, Vec3::new(3.0, 0.5, 3.0));
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/");
        crate::save_png(&format!("{dir}demo_frame_a.png"), 640, 360, &a).unwrap();
        crate::save_png(&format!("{dir}demo_frame_b.png"), 640, 360, &b).unwrap();
        assert!(!ImageStats::analyze(&a).looks_blank(), "frame A blank");
        assert!(!ImageStats::analyze(&b).looks_blank(), "frame B blank");
        // The two frames must differ (player moved, totem waved, chaser closer).
        let diff = a
            .chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(p, q)| (p[0] as i32 - q[0] as i32).abs() > 16)
            .count();
        assert!(diff > 500, "frames should differ across the playthrough: {diff}");
    }
}
