//! GPU check for the batched tilemap pass: a filled cell renders in its tint,
//! an empty cell leaves the background, and cells land in the right place.

use voltex_capture::HeadlessGpu;
use voltex_ecs::{Transform, World};
use voltex_math::Vec3;
use voltex_renderer_ecs::Tilemap;

const W: u32 = 400;
const H: u32 = 200;

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

#[test]
#[ignore] // GPU: cargo test -p voltex_capture --test tilemap -- --ignored
fn tilemap_fills_set_cells_and_leaves_empty_ones() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // 3x1 map, tile 2.0: cells 0 and 2 filled (red), cell 1 empty.
    let mut map = Tilemap::solid(3, 1, 2.0, [1.0, 0.0, 0.0, 1.0]);
    map.set(0, 0, 1);
    map.set(2, 0, 1);
    let mut world = World::new();
    world.spawn_bundle((Transform::from_position(Vec3::new(0.0, 0.0, 0.0)), map));

    // Ortho view 4 high x 8 wide centred on (3, 1): x in [-1, 7], y in [-1, 3].
    // 50 px per world unit. Cell 0 = x[0,2] -> px 50..150, cell 1 = px 150..250,
    // cell 2 = px 250..350; row y[0,2] -> px rows 50..150.
    let pose = voltex_capture::CameraPose {
        position: Vec3::new(3.0, 1.0, 10.0),
        yaw: 0.0,
        pitch: 0.0,
    };
    let projection = voltex_capture::Projection::Orthographic { height: 4.0 };
    let rgba = voltex_capture::render_world_from(&gpu, W, H, &mut world, pose, projection);

    let red = |p: [u8; 4]| p[0] > 150 && p[1] < 60 && p[2] < 60;
    let c0 = pixel(&rgba, 100, 100);
    let c1 = pixel(&rgba, 200, 100);
    let c2 = pixel(&rgba, 300, 100);
    let above = pixel(&rgba, 100, 25);
    eprintln!("cell0={c0:?} cell1={c1:?} cell2={c2:?} above={above:?}");
    assert!(red(c0), "filled cell 0 should be red: {c0:?}");
    assert!(!red(c1), "empty cell 1 should be background: {c1:?}");
    assert!(red(c2), "filled cell 2 should be red: {c2:?}");
    assert!(!red(above), "above the row (y > 2) should be background: {above:?}");

    voltex_capture::write_visual_record("tilemap_basic.png", W, H, &rgba);
}
