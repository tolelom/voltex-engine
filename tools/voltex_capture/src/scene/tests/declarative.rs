//! GPU tests for the declarative (`PrimitiveMesh` + `MaterialDesc`) scenes
//! and `materialize_renderables`.

use super::*;

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
    // survivor (v1.0.70) renders its objects via PrimitiveMesh +
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
    let rgba = render_survivor_seed_scene(&gpu, 320, 200);
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
