//! Sprite scenes: billboards, sprite-sheet frames and a particle burst.

use voltex_ecs::{Transform, World};
use voltex_math::Vec3;
use voltex_renderer::{Camera, LightUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use crate::scene::draw_and_read;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Spawn an orange particle burst at the origin, advance it `ticks` times by
/// `dt`, then render the surviving particles (as billboard sprites) from a
/// top-down orthographic camera. Exercises particle update → sprite render.
/// Returns RGBA8.
pub fn render_particle_burst(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    ticks: u32,
    dt: f32,
) -> Vec<u8> {
    use voltex_renderer_ecs::{spawn_particle, update_particles, Particle};
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();

    let mut world = World::new();
    let n = 16;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        // Spread in the XY (screen) plane so a front camera sees the whole fan.
        let vel = Vec3::new(a.cos() * 3.0, a.sin() * 3.0, 0.0);
        let part = Particle::new(vel, 2.0, 0.4);
        spawn_particle(&mut world, Vec3::ZERO, [1.0, 0.55, 0.1, 1.0], part);
    }
    for _ in 0..ticks {
        update_particles(&mut world, dt);
    }
    voltex_ecs::propagate_transforms(&mut world);

    // Front-facing ortho (looks -Z, up +Y) so the radial spread fills the view.
    let camera =
        Camera::orthographic(Vec3::new(0.0, 0.0, 12.0), width as f32 / height as f32, 16.0);
    let light = LightUniform::new();

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture parts",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render a textured billboard sampling `frame` of a 2×2 sprite sheet. The
/// atlas is a 64×64 four-color grid (TL red, TR green, BL blue, BR yellow), so
/// each frame fills the sprite with one solid color — verifying `uv_rect`
/// sub-rect sampling. Returns RGBA8.
pub fn render_sprite_sheet_frame(gpu: &HeadlessGpu, width: u32, height: u32, frame: u32) -> Vec<u8> {
    use voltex_renderer::GpuTexture;
    use voltex_renderer_ecs::Sprite;

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();

    // 64×64 atlas: 2×2 blocks of solid color (row-major, v=0 at top).
    let dim = 64usize;
    let mut pixels = vec![0u8; dim * dim * 4];
    let color_at = |cx: usize, cy: usize| -> [u8; 4] {
        match (cx, cy) {
            (0, 0) => [255, 0, 0, 255],   // TL red
            (1, 0) => [0, 255, 0, 255],   // TR green
            (0, 1) => [0, 0, 255, 255],   // BL blue
            _ => [255, 255, 0, 255],      // BR yellow
        }
    };
    for y in 0..dim {
        for x in 0..dim {
            let c = color_at(x / (dim / 2), y / (dim / 2));
            let o = (y * dim + x) * 4;
            pixels[o..o + 4].copy_from_slice(&c);
        }
    }
    let layout = GpuTexture::bind_group_layout(&gpu.device);
    let tex = GpuTexture::from_rgba(&gpu.device, &gpu.queue, dim as u32, dim as u32, &pixels, &layout);
    let tex_h = assets.add_texture(tex);

    let mut sprite = Sprite::textured(tex_h, [3.0, 3.0]);
    sprite.set_sheet_frame(2, 2, frame);

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, sprite);
    voltex_ecs::propagate_transforms(&mut world);

    let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), width as f32 / height as f32, 6.0);
    let light = LightUniform::new();

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture sheet",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

/// Render a single magenta billboard [`Sprite`] at the world origin, viewed
/// from `cam_pos` with the given `yaw` (pitch 0). A camera-facing quad should
/// present the same footprint from any orbit angle. Returns RGBA8.
pub fn render_billboard_sprite(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    cam_pos: Vec3,
    yaw: f32,
) -> Vec<u8> {
    use voltex_renderer_ecs::Sprite;
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::ZERO));
    world.add(e, Sprite::solid([1.5, 1.5], [1.0, 0.0, 1.0, 1.0]));
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(cam_pos, width as f32 / height as f32);
    camera.yaw = yaw;
    camera.pitch = 0.0;
    let light = LightUniform::new();

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture sprite",
        wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn particles_spread_outward_over_time() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Mean distance of orange pixels from the image center — grows as the
        // radial burst expands.
        // Orange/amber test by channel ordering (r > g > b), robust to whether
        // the target is sRGB-encoded (which lifts the mid/low channels).
        let is_orange = |p: &[u8]| -> bool {
            p[0] > 150 && p[0] as i32 > p[1] as i32 + 20 && p[1] as i32 > p[2] as i32 + 20
        };
        let mean_radius = |rgba: &[u8]| -> f64 {
            let (mut sum, mut n) = (0.0f64, 0u64);
            for (i, p) in rgba.chunks_exact(4).enumerate() {
                if is_orange(p) {
                    let x = (i % 256) as f64 - 128.0;
                    let y = (i / 256) as f64 - 128.0;
                    sum += (x * x + y * y).sqrt();
                    n += 1;
                }
            }
            if n == 0 { 0.0 } else { sum / n as f64 }
        };

        let early = render_particle_burst(&gpu, 256, 256, 2, 0.1); // t≈0.2s
        let late = render_particle_burst(&gpu, 256, 256, 8, 0.1); // t≈0.8s
        crate::write_visual_record("particle_burst.png", 256, 256, &late);

        let (re, rl) = (mean_radius(&early), mean_radius(&late));
        assert!(re > 1.0, "early burst should have visible particles, r={re:.1}");
        assert!(
            rl > re + 10.0,
            "particles should spread outward over time: early r={re:.1} late r={rl:.1}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn overlapping_sprites_blend_independent_of_spawn_order() {
        use voltex_renderer_ecs::Sprite;
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // A far blue + a near red sprite, both semi-transparent, overlapping at
        // screen center. Back-to-front sorting must make the composited result
        // the same no matter which order they were spawned in.
        let render = |near_first: bool| -> [u8; 3] {
            let target = OffscreenTarget::new(&gpu.device, 256, 256);
            let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
            let assets = RenderAssets::new();
            let mut world = World::new();
            let far = (Vec3::new(0.0, 0.0, -2.0), [0.1, 0.1, 1.0, 0.5]); // blue
            let near = (Vec3::new(0.0, 0.0, 2.0), [1.0, 0.1, 0.1, 0.5]); // red
            let order = if near_first { [near, far] } else { [far, near] };
            for (pos, color) in order {
                let e = world.spawn();
                world.add(e, Transform::from_position(pos));
                world.add(e, Sprite::solid([3.0, 3.0], color));
            }
            voltex_ecs::propagate_transforms(&mut world);
            let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), 1.0, 6.0);
            let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("capture sprite order"),
            });
            renderer.render(
                &gpu.queue,
                &mut encoder,
                &target.color_view,
                &target.depth_view,
                wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                &mut world,
                &assets,
                camera.view_projection(),
                camera.position,
                LightUniform::new(),
            );
            gpu.queue.submit(Some(encoder.finish()));
            let rgba = target.read_rgba8(&gpu.device, &gpu.queue);
            let i = (128 * 256 + 128) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };

        let a = render(true);
        let b = render(false);
        // Spawn-order independence: the two composites match closely.
        let diff = (a[0] as i32 - b[0] as i32).abs()
            + (a[1] as i32 - b[1] as i32).abs()
            + (a[2] as i32 - b[2] as i32).abs();
        assert!(diff < 12, "overlap must be spawn-order-independent: {a:?} vs {b:?}");
        // And the near (red) sprite composites on top → red beats blue.
        assert!(a[0] > a[2], "near red sprite should dominate the overlap: {a:?}");
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn render_assets_png_loader_roundtrips_into_a_sprite() {
        use voltex_renderer_ecs::Sprite;
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Encode a 2×2 solid-red PNG, then load it via the convenience method.
        let red = [255u8, 0, 0, 255].repeat(4); // 2×2 RGBA red
        let png = crate::encode_rgba8(2, 2, &red);

        let target = OffscreenTarget::new(&gpu.device, 256, 256);
        let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
        let mut assets = RenderAssets::new();
        let tex = assets
            .add_texture_png(&gpu.device, &gpu.queue, &png)
            .expect("decode+upload PNG");

        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, Sprite::textured(tex, [3.0, 3.0]));
        voltex_ecs::propagate_transforms(&mut world);

        let camera = Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), 1.0, 6.0);
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture png loader"),
        });
        renderer.render(
            &gpu.queue,
            &mut encoder,
            &target.color_view,
            &target.depth_view,
            wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            &mut world,
            &assets,
            camera.view_projection(),
            camera.position,
            LightUniform::new(),
        );
        gpu.queue.submit(Some(encoder.finish()));
        let rgba = target.read_rgba8(&gpu.device, &gpu.queue);

        let i = (128 * 256 + 128) * 4;
        let c = [rgba[i], rgba[i + 1], rgba[i + 2]];
        assert!(
            c[0] > 150 && c[1] < 90 && c[2] < 90,
            "PNG-loaded red texture should render red, got {c:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn sprite_sheet_frame_selects_atlas_cell() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let center = |rgba: &[u8]| -> [u8; 3] {
            let i = (128 * 256 + 128) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        // Frame 0 = TL red, 1 = TR green, 2 = BL blue, 3 = BR yellow.
        let f0 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 0));
        let f1 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 1));
        let f2 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 2));
        let f3 = center(&render_sprite_sheet_frame(&gpu, 256, 256, 3));
        assert!(f0[0] > 150 && f0[1] < 90 && f0[2] < 90, "frame 0 should be red, got {f0:?}");
        assert!(f1[1] > 150 && f1[0] < 90 && f1[2] < 90, "frame 1 should be green, got {f1:?}");
        assert!(f2[2] > 150 && f2[0] < 90 && f2[1] < 90, "frame 2 should be blue, got {f2:?}");
        assert!(
            f3[0] > 150 && f3[1] > 150 && f3[2] < 90,
            "frame 3 should be yellow, got {f3:?}"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn billboard_sprite_faces_camera_from_any_angle() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Count magenta pixels (the sprite color).
        let magenta = |rgba: &[u8]| -> u32 {
            rgba.chunks_exact(4)
                .filter(|p| p[0] > 120 && p[2] > 120 && p[1] < 80)
                .count() as u32
        };
        use std::f32::consts::FRAC_PI_2;
        // Front: camera on +Z looking -Z. Side: camera on +X looking -X. Same
        // distance (4) → a true billboard keeps the same on-screen footprint.
        let front = render_billboard_sprite(&gpu, 256, 256, Vec3::new(0.0, 0.0, 4.0), 0.0);
        let side = render_billboard_sprite(&gpu, 256, 256, Vec3::new(4.0, 0.0, 0.0), -FRAC_PI_2);
        crate::write_visual_record("sprite_billboard.png", 256, 256, &front);

        let (cf, cs) = (magenta(&front), magenta(&side));
        assert!(cf > 1500, "front view should show the sprite, got {cf} px");
        assert!(cs > 1500, "side view should show the sprite, got {cs} px");
        // Footprint should be nearly identical (it always faces the camera).
        let ratio = cf.min(cs) as f64 / cf.max(cs) as f64;
        assert!(
            ratio > 0.8,
            "billboard footprint must match across angles: front={cf} side={cs} ratio={ratio:.2}"
        );
    }
}
