//! Headless "playthrough" of `examples/game_ready_demo` — composes the same
//! scene (ground + blue player + red chaser + skinned totem + warm sun + two
//! colored point lights + a center spotlight + sprite pickups + debug gizmos +
//! follow camera) and renders a single frame through the real `BasicRenderer`,
//! with the skeleton advanced to time `t` and the player/chaser positions
//! scripted. This is what lets the demo's *visual* output be verified offscreen
//! (PNG) without running the interactive window.

use voltex_ecs::{propagate_transforms, MaterialHandle, MeshHandle, Transform, World};
use voltex_math::Vec3;
use voltex_renderer::{Camera, LightUniform, LineVertex, PointLight, SkinnedMesh};
use voltex_renderer_ecs::{update_skinned_poses, BasicRenderer, RenderAssets, Sprite, SkinnedPose};

use super::skinned::{demo_skeleton, strip_vertices, wave_animation};
use super::{matte, unit_cube};
use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// `game_ready_demo` 역할별 색 (base color).
pub(super) const GROUND_COLOR: [f32; 4] = [0.35, 0.37, 0.40, 1.0];
pub(super) const PLAYER_COLOR: [f32; 4] = [0.20, 0.50, 1.00, 1.0];
pub(super) const CHASER_COLOR: [f32; 4] = [1.00, 0.30, 0.20, 1.0];

/// The scripted state of one playthrough frame.
#[derive(Debug, Clone, Copy)]
pub struct DemoMoment {
    /// Skeleton animation time in seconds.
    pub t: f32,
    /// The player stands at `(player_x, 0.5, 0)`.
    pub player_x: f32,
    /// Where the chaser stands.
    pub chaser_pos: Vec3,
}

impl DemoMoment {
    fn player_pos(&self) -> Vec3 {
        Vec3::new(self.player_x, 0.5, 0.0)
    }
}

/// Which runtime shading path draws the demo scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoShading {
    /// Colored path — `RuntimeConfig::with_pbr(false)`, the default.
    Colored,
    /// Full PBR path — `RuntimeConfig::with_pbr(true)`.
    Pbr,
}

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
    let moment = DemoMoment {
        t,
        player_x,
        chaser_pos,
    };
    render_demo_frame_mode(gpu, width, height, moment, DemoShading::Colored)
}

/// Like [`render_demo_frame`] but through the chosen runtime shading path —
/// what a user gets by flipping `RuntimeConfig::with_pbr`.
pub fn render_demo_frame_mode(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    moment: DemoMoment,
    shading: DemoShading,
) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);
    let mut renderer = BasicRenderer::new(device, queue, CAPTURE_FORMAT, 64);
    renderer.set_shadows(true); // demo enables directional shadows
    renderer.set_pbr_enabled(shading == DemoShading::Pbr);

    let mut assets = RenderAssets::new();
    let mut world = spawn_demo_world(device, &mut assets, moment);

    // Follow camera (as in the demo).
    let mut camera = Camera::new(
        Vec3::new(moment.player_x, 9.0, 13.0),
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
            PointLight::spot([0.0, 9.0, 0.0], [0.0, -1.0, 0.0], [1.0, 1.0, 0.95], 22.0, 4.0, 28.0),
        ],
    );
    renderer.set_debug_lines(queue, &demo_debug_lines(moment));

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

/// Spawn the demo's entities (ground, player, chaser, skinned totem, sprite
/// pickups) with the skeleton advanced to `moment.t` and transforms propagated.
fn spawn_demo_world(device: &wgpu::Device, assets: &mut RenderAssets, moment: DemoMoment) -> World {
    let cube = assets.add_mesh(unit_cube(device));
    let gray = assets.add_material(matte(GROUND_COLOR));
    let blue = assets.add_material(matte(PLAYER_COLOR));
    let red = assets.add_material(matte(CHASER_COLOR));
    let (strip_verts, strip_indices) = strip_vertices();
    let strip = assets.add_skinned_mesh(SkinnedMesh::new(device, &strip_verts, &strip_indices));

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
    world.add(player, Transform::from_position(moment.player_pos()));
    world.add(player, MeshHandle(cube.0));
    world.add(player, MaterialHandle(blue.0));

    // Chaser (red, slightly smaller).
    let chaser = world.spawn();
    let mut ct = Transform::from_position(moment.chaser_pos);
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

    // Floating sprite pickups (as in the demo).
    for (pos, color) in [
        (Vec3::new(4.0, 1.2, -4.0), [1.0, 0.85, 0.1, 1.0]),
        (Vec3::new(-4.0, 1.2, 4.0), [0.1, 1.0, 0.4, 1.0]),
        (Vec3::new(6.0, 1.2, 2.0), [0.3, 0.7, 1.0, 1.0]),
    ] {
        let p = world.spawn();
        world.add(p, Transform::from_position(pos));
        world.add(p, Sprite::solid([0.8, 0.8], color));
    }

    // Advance the skeleton to time t.
    update_skinned_poses(&mut world, moment.t);
    propagate_transforms(&mut world);
    world
}

/// Debug gizmos (as in the demo's set_debug_draw_fn): yellow chase vector +
/// crosses on player/chaser. Built as raw LineVertex pairs here (this crate
/// doesn't depend on voltex_runtime's DebugLines helper).
fn demo_debug_lines(moment: DemoMoment) -> Vec<LineVertex> {
    let player_pos = moment.player_pos();
    let chaser_pos = moment.chaser_pos;
    let cross = |c: Vec3, h: f32, col: [f32; 3], out: &mut Vec<LineVertex>| {
        for d in [Vec3::new(h, 0.0, 0.0), Vec3::new(0.0, h, 0.0), Vec3::new(0.0, 0.0, h)] {
            out.push(LineVertex::new([(c - d).x, (c - d).y, (c - d).z], col));
            out.push(LineVertex::new([(c + d).x, (c + d).y, (c + d).z], col));
        }
    };
    let mut lines = Vec::new();
    lines.push(LineVertex::new([chaser_pos.x, chaser_pos.y, chaser_pos.z], [1.0, 1.0, 0.0]));
    lines.push(LineVertex::new([player_pos.x, player_pos.y, player_pos.z], [1.0, 1.0, 0.0]));
    cross(player_pos, 0.6, [0.0, 1.0, 1.0], &mut lines);
    cross(chaser_pos, 0.5, [1.0, 0.0, 1.0], &mut lines);
    lines
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
        crate::write_visual_record("demo_frame_a.png", 640, 360, &a);
        crate::write_visual_record("demo_frame_b.png", 640, 360, &b);
        assert!(!ImageStats::analyze(&a).looks_blank(), "frame A blank");
        assert!(!ImageStats::analyze(&b).looks_blank(), "frame B blank");
        // The two frames must differ (player moved, totem waved, chaser closer).
        let diff = a
            .as_chunks::<4>().0.iter()
            .zip(b.as_chunks::<4>().0.iter())
            .filter(|(p, q)| (p[0] as i32 - q[0] as i32).abs() > 16)
            .count();
        assert!(diff > 500, "frames should differ across the playthrough: {diff}");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn demo_renders_under_pbr_runtime_path() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // The full demo scene (ground + player + chaser + skinned totem +
        // sprites + sun + point/spot lights + shadows) rendered through the PBR
        // runtime path — what a user gets from RuntimeConfig::with_pbr(true).
        let moment = DemoMoment { t: 1.0, player_x: 5.0, chaser_pos: Vec3::new(3.0, 0.5, 3.0) };
        let colored = render_demo_frame_mode(&gpu, 640, 360, moment, DemoShading::Colored);
        let pbr = render_demo_frame_mode(&gpu, 640, 360, moment, DemoShading::Pbr);
        crate::write_visual_record("demo_frame_pbr.png", 640, 360, &pbr);

        assert!(!ImageStats::analyze(&pbr).looks_blank(), "PBR demo frame blank");
        // PBR shading should differ from the colored path on the same scene.
        let diff = colored
            .as_chunks::<4>().0.iter()
            .zip(pbr.as_chunks::<4>().0.iter())
            .filter(|(p, q)| (p[0] as i32 - q[0] as i32).abs() > 16)
            .count();
        assert!(diff > 500, "PBR demo should shade differently than colored: {diff}");
    }
}
