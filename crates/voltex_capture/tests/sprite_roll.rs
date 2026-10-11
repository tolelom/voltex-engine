//! GPU check for `Sprite::roll_from_transform`: a wide sprite whose transform
//! is rotated 90° about Z renders tall with the flag on and stays a wide
//! upright billboard with it off (the default).

use voltex_capture::HeadlessGpu;
use voltex_ecs::{Transform, World};
use voltex_math::{Quat, Vec3};
use voltex_renderer_ecs::Sprite;

const W: u32 = 200;
const H: u32 = 200;

fn render(gpu: &HeadlessGpu, roll: bool) -> Vec<u8> {
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_axis_angle(Vec3::Z, std::f32::consts::FRAC_PI_2);
    let mut sprite = Sprite::solid([3.0, 1.0], [1.0, 0.0, 0.0, 1.0]);
    sprite.roll_from_transform = roll;
    let mut world = World::new();
    world.spawn_bundle((t, sprite));
    // 4x4 world units across 200 px: 50 px per unit, origin at pixel (100, 100).
    voltex_capture::render_world_ortho(gpu, W, H, &mut world, Vec3::new(0.0, 0.0, 10.0), 0.0, 0.0, 4.0)
}

fn is_red(rgba: &[u8], x: u32, y: u32) -> bool {
    let i = ((y * W + x) * 4) as usize;
    rgba[i] > 150 && rgba[i + 1] < 60 && rgba[i + 2] < 60
}

#[test]
#[ignore] // GPU: cargo test -p voltex_capture --test sprite_roll -- --ignored
fn roll_from_transform_rotates_the_billboard_in_screen_plane() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // 1.2 units from the centre: inside the long (3.0) axis, outside the short (1.0) one.
    let (right_of_centre, above_centre) = ((160, 100), (100, 40));

    let upright = render(&gpu, false);
    assert!(is_red(&upright, right_of_centre.0, right_of_centre.1), "default stays wide");
    assert!(!is_red(&upright, above_centre.0, above_centre.1), "default ignores rotation");

    let rolled = render(&gpu, true);
    assert!(!is_red(&rolled, right_of_centre.0, right_of_centre.1), "rolled sprite is no longer wide");
    assert!(is_red(&rolled, above_centre.0, above_centre.1), "rolled sprite stands tall");

    let png = voltex_capture::encode_rgba8(W, H, &rolled);
    if let Err(e) = std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/visual/sprite_roll.png"),
        png,
    ) {
        eprintln!("screenshot write failed: {e}");
    }
}
