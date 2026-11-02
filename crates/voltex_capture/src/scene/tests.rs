use super::*;
use crate::ImageStats;

/// Minimal GLB (one triangle, POSITION + indices) — mirrors the
/// voltex_image parser's own test fixture.
fn minimal_glb_triangle() -> Vec<u8> {
    let mut bin = Vec::new();
    for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bin.extend_from_slice(&v.to_le_bytes());
    }
    for &i in &[0u16, 1, 2] {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    bin.extend_from_slice(&[0, 0]); // 4-byte align
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":{}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":6}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","max":[1.0,1.0,0.0],"min":[0.0,0.0,0.0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],"meshes":[{{"name":"Tri","primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}]}}"#,
        bin.len()
    );
    let jb = json.into_bytes();
    let jpad = (jb.len() + 3) & !3;
    let mut json_padded = jb;
    while json_padded.len() < jpad {
        json_padded.push(b' ');
    }
    let total = 12 + 8 + json_padded.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // glTF magic
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // JSON
    glb.extend_from_slice(&json_padded);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // BIN\0
    glb.extend_from_slice(&bin);
    glb
}

#[test]
#[ignore = "requires a GPU adapter"]
fn shadows_darken_ground_under_occluder() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let off = render_shadow_demo(&gpu, 512, 512, false);
    let on = render_shadow_demo(&gpu, 512, 512, true);
    crate::write_visual_record("shadow_off.png", 512, 512, &off);
    crate::write_visual_record("shadow_on.png", 512, 512, &on);
    // Shadows on → overall darker (occluded ground region) than shadows off.
    let lum = |r: &[u8]| -> u64 {
        r.as_chunks::<4>().0.iter().map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum()
    };
    assert!(lum(&on) < lum(&off), "shadowed frame should be darker overall");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn hidden_component_skips_rendering() {
    use voltex_renderer_ecs::Hidden;
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let render = |hide: bool| -> Vec<u8> {
        let target = OffscreenTarget::new(&gpu.device, 128, 128);
        let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
        let mut assets = RenderAssets::new();
        let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
        let mat_h = assets.add_material(MaterialUniform::new());
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, MeshHandle(mesh_h.0));
        world.add(e, MaterialHandle(mat_h.0));
        if hide {
            world.add(e, Hidden);
        }
        voltex_ecs::propagate_transforms(&mut world);
        let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), 1.0);
        camera.yaw = -0.7;
        camera.pitch = -0.4;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
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
        target.read_rgba8(&gpu.device, &gpu.queue)
    };
    assert!(!ImageStats::analyze(&render(false)).looks_blank(), "visible cube should render");
    assert!(ImageStats::analyze(&render(true)).looks_blank(), "Hidden cube should not render");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn sphere_front_hemisphere_is_lit() {
    // Regression for generate_sphere winding: with a front directional
    // light the camera-facing hemisphere must be lit (bright center). When
    // the winding was reversed the near hemisphere was culled and the far
    // (unlit) inside showed through → a uniformly dark sphere.
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    // render_cube_point_lit draws a sphere with a directional light coming
    // from +Z (toward the camera); no point lights.
    let rgba = render_cube_point_lit(&gpu, 256, 256, &[]);
    // Center pixel = sphere's front pole, facing the light → should be bright.
    let idx = ((128 * 256 + 128) * 4) as usize;
    let center_sum = rgba[idx] as u32 + rgba[idx + 1] as u32 + rgba[idx + 2] as u32;
    assert!(
        center_sum > 180,
        "front-lit sphere center should be bright, got sum={center_sum}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn point_lights_add_colored_illumination() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    // mean luminance over lit (non-near-black) pixels.
    fn mean_lum(rgba: &[u8]) -> f64 {
        let (mut s, mut n) = (0u64, 0u64);
        for p in rgba.as_chunks::<4>().0.iter() {
            let sum = p[0] as u32 + p[1] as u32 + p[2] as u32;
            if sum > 24 {
                s += sum as u64;
                n += 1;
            }
        }
        s as f64 / n.max(1) as f64
    }

    let baseline = render_cube_point_lit(&gpu, 256, 256, &[]);
    // Surround the sphere with white point lights on all six axes, so the
    // visible hemisphere is lit regardless of its normal orientation.
    let pl = |p: [f32; 3]| voltex_renderer::PointLight::new(p, [1.0, 1.0, 1.0], 10.0, 2.0);
    let lights = [
        pl([3.0, 0.0, 0.0]),
        pl([-3.0, 0.0, 0.0]),
        pl([0.0, 3.0, 0.0]),
        pl([0.0, -3.0, 0.0]),
        pl([0.0, 0.0, 3.0]),
        pl([0.0, 0.0, -3.0]),
    ];
    let lit = render_cube_point_lit(&gpu, 256, 256, &lights);

    let (lb, ll) = (mean_lum(&baseline), mean_lum(&lit));
    assert!(
        ll > lb + 15.0,
        "surrounding point lights should brighten the sphere: baseline={lb} lit={ll}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn spotlight_lights_cone_footprint_not_corners() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let lum = |rgba: &[u8], x: usize, y: usize| -> u32 {
        let i = (y * 256 + x) * 4;
        rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32
    };

    // Spotlight above the plane aimed straight down — a tight cone hits the
    // center of the top-down view but not the corners.
    let down = voltex_renderer::PointLight::spot(
        [0.0, 5.0, 0.0],
        [0.0, -1.0, 0.0],
        [1.0, 1.0, 1.0],
        12.0,
        6.0,
        22.0,
    );
    let on = render_spot_lit(&gpu, 256, 256, &[down]);
    crate::write_visual_record("spot_cone.png", 256, 256, &on);
    let center = lum(&on, 128, 128);
    let corner = lum(&on, 20, 20);
    assert!(
        center > 150,
        "spot cone center should be brightly lit, got {center}"
    );
    assert!(
        center > corner + 60,
        "inside the cone (center={center}) must be much brighter than the corner outside it (corner={corner})"
    );

    // Same light aimed horizontally — its cone no longer points at the ground
    // below, so the center stays dark (the cone gate culls it).
    let sideways = voltex_renderer::PointLight::spot(
        [0.0, 5.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        12.0,
        6.0,
        22.0,
    );
    let off = render_spot_lit(&gpu, 256, 256, &[sideways]);
    let center_off = lum(&off, 128, 128);
    assert!(
        center_off + 60 < center,
        "aiming the spot away from the ground should darken the center: aimed-down={center} aimed-away={center_off}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn pbr_normal_map_perturbs_shading() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let flat = render_pbr_sphere_normalmap(&gpu, 256, 256, false);
    let mapped = render_pbr_sphere_normalmap(&gpu, 256, 256, true);
    let mut diff = 0u64;
    for (a, b) in flat.as_chunks::<4>().0.iter().zip(mapped.as_chunks::<4>().0.iter()) {
        diff += (a[0] as i64 - b[0] as i64).unsigned_abs();
        diff += (a[1] as i64 - b[1] as i64).unsigned_abs();
        diff += (a[2] as i64 - b[2] as i64).unsigned_abs();
    }
    let mean = diff as f64 / (256.0 * 256.0 * 3.0);
    assert!(mean > 2.0, "a normal map should perturb PBR shading, mean diff={mean:.2}");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn basic_renderer_pbr_path_shades_differently_than_colored() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let colored = render_pbr_basic_renderer(&gpu, 256, 256, false);
    let pbr = render_pbr_basic_renderer(&gpu, 256, 256, true);
    crate::write_visual_record("pbr_runtime.png", 256, 256, &pbr);

    assert!(!crate::ImageStats::analyze(&pbr).looks_blank(), "PBR frame blank");
    // Enabling PBR must visibly change shading vs the Blinn-Phong path.
    let mut diff = 0u64;
    for (a, b) in colored.as_chunks::<4>().0.iter().zip(pbr.as_chunks::<4>().0.iter()) {
        diff += (a[0] as i64 - b[0] as i64).unsigned_abs();
        diff += (a[1] as i64 - b[1] as i64).unsigned_abs();
        diff += (a[2] as i64 - b[2] as i64).unsigned_abs();
    }
    let mean = diff as f64 / (256.0 * 256.0 * 3.0);
    assert!(mean > 4.0, "PBR shading should differ from colored, mean channel diff={mean:.2}");
}

#[test]
#[ignore = "requires a GPU adapter"]
fn orthographic_footprint_is_depth_independent() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    // Count lit (non-near-black) pixels — the cube's on-screen footprint.
    let footprint = |rgba: &[u8]| -> u32 {
        rgba.as_chunks::<4>().0.iter()
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
            .count() as u32
    };

    // Orthographic: a cube at z=0 vs z=-5 should cover ~the same pixels.
    let o_near = footprint(&render_cube_at_depth(&gpu, 256, 256, true, 0.0));
    let o_far = footprint(&render_cube_at_depth(&gpu, 256, 256, true, -5.0));
    let o_ratio = o_near.min(o_far) as f64 / o_near.max(o_far) as f64;
    assert!(
        o_near > 400 && o_ratio > 0.9,
        "orthographic footprint must be depth-independent: near={o_near} far={o_far} ratio={o_ratio:.2}"
    );

    // Perspective: the farther cube must be clearly smaller.
    let p_near = footprint(&render_cube_at_depth(&gpu, 256, 256, false, 0.0));
    let p_far = footprint(&render_cube_at_depth(&gpu, 256, 256, false, -5.0));
    assert!(
        (p_far as f64) < (p_near as f64) * 0.7,
        "perspective should foreshorten the farther cube: near={p_near} far={p_far}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn debug_lines_draw_colored_segments() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let px = |rgba: &[u8], x: usize, y: usize| -> [u8; 3] {
        let i = (y * 256 + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    };

    // No lines → black frame (control).
    let empty = render_debug_lines(&gpu, 256, 256, &[]);
    let c0 = px(&empty, 128, 128);
    assert!(
        c0[0] < 12 && c0[1] < 12 && c0[2] < 12,
        "no lines should leave the center black, got {c0:?}"
    );

    // A red horizontal segment through the world origin crosses screen
    // center; a green vertical one shares the center pixel.
    use voltex_renderer::LineVertex;
    let red = [1.0, 0.0, 0.0];
    let green = [0.0, 1.0, 0.0];
    let verts = [
        LineVertex::new([-1.5, 0.0, 0.0], red),
        LineVertex::new([1.5, 0.0, 0.0], red),
        LineVertex::new([0.0, -1.5, 0.0], green),
        LineVertex::new([0.0, 1.5, 0.0], green),
    ];
    let lit = render_debug_lines(&gpu, 256, 256, &verts);
    crate::write_visual_record("debug_lines.png", 256, 256, &lit);

    // The horizontal line sits at world y=0 → near the center row; a 1px
    // line straddles the row boundary, so scan a small band for it.
    let brightest_in = |xs: std::ops::Range<usize>, ys: std::ops::Range<usize>| -> [u8; 3] {
        let mut best = [0u8; 3];
        for y in ys {
            for x in xs.clone() {
                let p = px(&lit, x, y);
                if p.iter().map(|&c| c as u32).sum::<u32>()
                    > best.iter().map(|&c| c as u32).sum::<u32>()
                {
                    best = p;
                }
            }
        }
        best
    };
    // A point on the red horizontal line, well left of the vertical one.
    let on_red = brightest_in(55..66, 120..137);
    assert!(
        on_red[0] > 150 && on_red[1] < 90 && on_red[2] < 90,
        "the horizontal segment should paint a red row, got {on_red:?}"
    );
    // A point on the green vertical line, well above the horizontal one.
    let on_green = brightest_in(120..137, 55..66);
    assert!(
        on_green[1] > 150 && on_green[0] < 90 && on_green[2] < 90,
        "the vertical segment should paint a green column, got {on_green:?}"
    );
    // A corner away from both lines stays black.
    let corner = px(&lit, 20, 20);
    assert!(
        corner.iter().all(|&c| c < 12),
        "corners away from the lines should stay black, got {corner:?}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn loads_gltf_mesh() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let glb = minimal_glb_triangle();
    let mesh = voltex_renderer::Mesh::from_gltf(&gpu.device, &glb).expect("parse+upload glTF");
    assert_eq!(mesh.num_indices, 3, "triangle should have 3 indices");
}

/// Count pixels that differ from the top-left (background) pixel — i.e. the
/// rendered object's footprint.
fn object_pixels(rgba: &[u8]) -> usize {
    let bg = [rgba[0], rgba[1], rgba[2]];
    rgba
        .as_chunks::<4>().0.iter()
        .filter(|p| {
            (p[0] as i32 - bg[0] as i32).abs() > 20
                || (p[1] as i32 - bg[1] as i32).abs() > 20
                || (p[2] as i32 - bg[2] as i32).abs() > 20
        })
        .count()
}

/// Regression for the "I can't see my boxes" report: a 1-unit object at the
/// origin is a tiny speck under the runtime's default far camera
/// `(0,10,20)`, but fills a large area once a framing camera is used. This
/// is why `RuntimeConfig::with_camera` / `RuntimeApp::set_camera_fn` exist.
/// Regression for "objects camouflage against the ground": BasicRenderer
/// must honour `MaterialUniform.base_color`. With the old white-default
/// path a framed scene of same-shaded cubes + ground showed only 2 colors
/// (ground + background); distinct per-role colors add at least the blue
/// player on the gray ground → 3+.
#[test]
#[ignore = "requires a GPU adapter"]
fn basic_renderer_applies_material_base_color() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    let rgba = render_demo_like_cam(&gpu, 320, 180, Vec3::new(0.0, 9.0, 13.0), 0.0, -0.55, true);
    let stats = ImageStats::analyze(&rgba);
    assert!(
        stats.distinct_colors >= 3,
        "per-object base_color should be visible against the ground: {stats:?}"
    );
}

#[test]
#[ignore = "requires a GPU adapter"]
fn camera_framing_controls_visible_object_size() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (320, 180);
    // Cubes only (no ground), so the footprint isolates the gameplay objects.
    let far = render_demo_like_cam(&gpu, w, h, Vec3::new(0.0, 10.0, 20.0), 0.0, -0.4, false);
    let close = render_demo_like_cam(&gpu, w, h, Vec3::new(0.0, 2.5, 6.0), 0.0, -0.3, false);
    let (far_px, close_px) = (object_pixels(&far), object_pixels(&close));
    assert!(far_px > 0, "the cube must render even when far (got {far_px})");
    assert!(
        close_px > far_px * 5,
        "framing camera should show the object far larger: far={far_px} close={close_px}"
    );
}

// GPU-gated: ignored by default (no GPU in CI). Run with:
//   cargo test -p voltex_capture -- --ignored
#[test]
#[ignore]
fn single_cube_is_not_blank() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_single_cube(&gpu, 256, 256);
    let stats = ImageStats::analyze(&rgba);
    assert!(
        !stats.looks_blank(),
        "single cube frame is blank: {stats:?}"
    );
    assert!(stats.mean_luminance > 0.01, "frame too dark: {stats:?}");
}

#[test]
#[ignore]
fn cube_grid_is_not_blank() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_cube_grid(&gpu, 320, 240);
    let stats = ImageStats::analyze(&rgba);
    assert!(!stats.looks_blank(), "cube grid frame is blank: {stats:?}");
}

#[test]
#[ignore]
fn materialized_primitives_render_nonblank() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_materialized_primitives(&gpu, 320, 240);
    let stats = ImageStats::analyze(&rgba);
    assert!(
        !stats.looks_blank(),
        "declarative primitive frame is blank — materialize_renderables \
             did not produce drawable meshes: {stats:?}"
    );
    assert!(stats.mean_luminance > 0.01, "frame too dark: {stats:?}");
}

#[test]
#[ignore]
fn data_authored_scene_renders_nonblank() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_data_authored_scene(&gpu, 320, 240);
    let stats = ImageStats::analyze(&rgba);
    assert!(
        !stats.looks_blank(),
        "scene loaded from JSON data rendered blank — the data→materialize→\
             render path is broken: {stats:?}"
    );
    assert!(stats.mean_luminance > 0.01, "frame too dark: {stats:?}");
}

#[test]
#[ignore]
fn declarative_scene_renders_distinct_per_object_colors() {
    // voltex_survivor (v1.0.70) renders its objects via PrimitiveMesh +
    // MaterialDesc. This proves those per-object colors actually reach the
    // screen distinctly — a green player pixel and a red enemy pixel both
    // exist (everything was white before the material-color path).
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_declarative_colored_scene(&gpu, 320, 200);
    let stats = ImageStats::analyze(&rgba);
    assert!(
        stats.distinct_colors >= 3,
        "ground/player/enemy must shade distinctly, not all one color: {stats:?}"
    );

    let mut has_green = false;
    let mut has_red = false;
    for p in rgba.as_chunks::<4>().0.iter() {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        if g > r + 30 && g > b + 30 {
            has_green = true;
        }
        if r > g + 30 && r > b + 30 {
            has_red = true;
        }
    }
    assert!(has_green, "the green player should produce green-dominant pixels");
    assert!(has_red, "the red enemy should produce red-dominant pixels");
}

#[test]
#[ignore]
fn boids_swarm_renders_many_objects() {
    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let rgba = render_boids_swarm(&gpu, 400, 400);
    let stats = ImageStats::analyze(&rgba);
    assert!(!stats.looks_blank(), "swarm frame is blank: {stats:?}");
    // A 120-cube swarm should paint a lot of distinct colors over the ground.
    assert!(
        stats.distinct_colors > 50,
        "expected a colorful swarm, got {} colors",
        stats.distinct_colors
    );
}

#[test]
#[ignore]
fn materialize_picks_up_entities_spawned_after_a_prior_pass() {
    // The runtime calls materialize_renderables every frame (not just at
    // gpu_init) so entities spawned at runtime — by Lua, projectile
    // factories, etc. — become drawable. This proves a later pass resolves
    // an entity that did not exist during the first pass.
    use voltex_renderer_ecs::{materialize_renderables, MaterialDesc, PrimitiveMesh};

    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut assets = RenderAssets::new();
    let mut world = World::new();

    // Frame 1: one cube, materialized.
    let a = world.spawn();
    world.add(a, Transform::default());
    world.add(a, PrimitiveMesh::cube(1.0));
    world.add(a, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));
    materialize_renderables(&mut world, &mut assets, &gpu.device);
    assert!(world.get::<MeshHandle>(a).is_some(), "frame-1 entity resolved");

    // Frame 2: a new entity appears (as if spawned by a script) and the
    // next materialize pass must resolve it too.
    let b = world.spawn();
    world.add(b, Transform::from_position(Vec3::new(2.0, 0.0, 0.0)));
    world.add(b, PrimitiveMesh::sphere_default(0.5));
    world.add(b, MaterialDesc::color([1.0, 0.0, 0.0, 1.0]));
    assert!(world.get::<MeshHandle>(b).is_none(), "not yet materialized");

    materialize_renderables(&mut world, &mut assets, &gpu.device);
    assert!(
        world.get::<MeshHandle>(b).is_some(),
        "a later materialize pass must resolve a runtime-spawned entity"
    );
    assert!(world.get::<MaterialHandle>(b).is_some());
}

#[test]
#[ignore]
fn materialize_dedupes_identical_meshes_and_attaches_bounds() {
    use voltex_ecs::Bounds;
    use voltex_renderer_ecs::{materialize_renderables, MaterialDesc, PrimitiveMesh};

    let gpu = match HeadlessGpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut assets = RenderAssets::new();
    let mut world = World::new();

    // Three identical cubes + one sphere.
    let cubes: Vec<_> = (0..3)
        .map(|i| {
            let e = world.spawn();
            world.add(e, Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)));
            world.add(e, PrimitiveMesh::cube(1.0));
            world.add(e, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));
            e
        })
        .collect();
    let sphere = world.spawn();
    world.add(sphere, Transform::from_position(Vec3::ZERO));
    world.add(sphere, PrimitiveMesh::sphere_default(0.5));
    world.add(sphere, MaterialDesc::color([1.0, 1.0, 1.0, 1.0]));

    materialize_renderables(&mut world, &mut assets, &gpu.device);

    // Every entity got a MeshHandle, a MaterialHandle, and Bounds.
    for &e in &cubes {
        assert!(world.get::<MeshHandle>(e).is_some(), "cube missing MeshHandle");
        assert!(world.get::<MaterialHandle>(e).is_some(), "cube missing MaterialHandle");
        assert!(world.get::<Bounds>(e).is_some(), "cube missing Bounds");
    }

    // The three identical cubes share ONE mesh; the sphere differs.
    let c0 = *world.get::<MeshHandle>(cubes[0]).unwrap();
    let c1 = *world.get::<MeshHandle>(cubes[1]).unwrap();
    let c2 = *world.get::<MeshHandle>(cubes[2]).unwrap();
    let sp = *world.get::<MeshHandle>(sphere).unwrap();
    assert_eq!(c0, c1, "identical cubes must dedupe to one mesh");
    assert_eq!(c1, c2, "identical cubes must dedupe to one mesh");
    assert_ne!(c0, sp, "a sphere must not share the cube's mesh");

    // Identical materials also dedupe to one handle.
    let m0 = *world.get::<MaterialHandle>(cubes[0]).unwrap();
    let m_sp = *world.get::<MaterialHandle>(sphere).unwrap();
    assert_eq!(m0, m_sp, "identical materials must dedupe to one handle");

    // Idempotent: a second call adds nothing (handles unchanged).
    materialize_renderables(&mut world, &mut assets, &gpu.device);
    assert_eq!(c0, *world.get::<MeshHandle>(cubes[0]).unwrap());
}
