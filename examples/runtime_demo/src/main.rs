//! runtime_demo — minimal "hello world" of `voltex_runtime`.
//!
//! Spawns a small solar system (sun + 3 planets + 1 moon) and animates each
//! body's local Y rotation via a user system. The point is the `main` body —
//! four lines drive the whole window/render loop because `RuntimeApp` owns
//! everything else.

use voltex::ecs::{add_child, Bounds, MaterialHandle, MeshHandle, Tag};
use voltex::math::{Vec3, AABB};
use voltex::prelude::*;
use voltex_renderer::{material::MaterialUniform, mesh::Mesh, vertex::MeshVertex};
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

const SUN_TAG: &str = "sun";
const PLANET_TAG: &str = "planet";
const MOON_TAG: &str = "moon";

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Runtime Demo - Solar System")
            .with_window_size(1280, 720)
            .with_scene_builder(setup_solar_system)
            .with_gpu_init(upload_meshes_and_handles)
            .with_system_setup(register_orbit_system),
    )?
    .run()
}

fn setup_solar_system(world: &mut World, _registry: &mut ComponentRegistry) {
    // Sun at the origin.
    let sun = world.spawn();
    world.add(sun, Transform::from_position(Vec3::ZERO));
    world.add(sun, Tag(SUN_TAG.to_string()));

    // Three planets at increasing radius, each with their own scale.
    let planet_specs = [
        (Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.5)),
        (Vec3::new(9.0, 0.0, 0.0), Vec3::new(0.7, 0.7, 0.7)),
        (Vec3::new(13.0, 0.0, 0.0), Vec3::new(0.4, 0.4, 0.4)),
    ];
    let mut planets = Vec::new();
    for (pos, scale) in planet_specs {
        let p = world.spawn();
        let mut t = Transform::from_position(pos);
        t.scale = scale;
        world.add(p, t);
        world.add(p, Tag(PLANET_TAG.to_string()));
        add_child(world, sun, p);
        planets.push(p);
    }

    // One moon orbiting the first planet.
    let moon = world.spawn();
    let mut moon_t = Transform::from_position(Vec3::new(1.5, 0.0, 0.0));
    moon_t.scale = Vec3::new(0.3, 0.3, 0.3);
    world.add(moon, moon_t);
    world.add(moon, Tag(MOON_TAG.to_string()));
    add_child(world, planets[0], moon);
}

fn upload_meshes_and_handles(
    app: &mut RuntimeApp,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let _ = queue; // we only need the device for mesh upload
    let mesh = make_unit_cube(device);
    let mesh_h = app.assets.add_mesh(mesh);
    let mat_h = app.assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));

    // Attach render handles to every entity that already has a Transform.
    let entities: Vec<_> = app
        .world
        .query::<Transform>()
        .map(|(e, _)| e)
        .collect();
    for e in entities {
        app.world.add(e, MeshHandle(mesh_h.0));
        app.world.add(e, MaterialHandle(mat_h.0));
        app.world.add(e, bounds);
    }
}

fn register_orbit_system(app: &mut RuntimeApp) {
    // HUD demo: set a one-shot user segment shown in the window title.
    // The runtime auto-appends the FPS counter, so the title bar reads
    // "voltex_runtime: solar system | 3 planets, 6 moons | XX fps".
    let bodies = app
        .world
        .query::<Tag>()
        .filter(|(_, t)| t.0 == PLANET_TAG || t.0 == MOON_TAG)
        .count();
    let planets = app
        .world
        .query::<Tag>()
        .filter(|(_, t)| t.0 == PLANET_TAG)
        .count();
    app.hud.set_user_segment(Some(format!(
        "{} planets, {} moons",
        planets,
        bodies - planets
    )));

    // Sun spins slowly; planets faster; moons fastest. Two-pass to avoid
    // overlapping borrows: collect (entity, delta) then mutate Transforms.
    app.add_system(|world, dt| {
        let mut updates: Vec<(voltex::ecs::Entity, f32)> = Vec::new();
        for (e, tag) in world.query::<Tag>() {
            let speed = match tag.0.as_str() {
                SUN_TAG => 0.2,
                PLANET_TAG => 0.8,
                MOON_TAG => 2.0,
                _ => 0.0,
            };
            if speed != 0.0 {
                updates.push((e, speed * dt));
            }
        }
        for (e, delta) in updates {
            if let Some(t) = world.get_mut::<Transform>(e) {
                t.rotation.y += delta;
            }
        }
    });
}

fn make_unit_cube(device: &wgpu::Device) -> Mesh {
    // Single-face cube approximation — six faces, 24 vertices, 36 indices.
    // Normals point outward; UVs are unit-square per face.
    let s = 0.5;
    let verts: Vec<MeshVertex> = {
        let mut v = Vec::with_capacity(24);
        // +X
        v.extend([
            MeshVertex { position: [ s, -s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
            MeshVertex { position: [ s, -s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
            MeshVertex { position: [ s,  s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
            MeshVertex { position: [ s,  s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        ]);
        // -X
        v.extend([
            MeshVertex { position: [-s, -s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
            MeshVertex { position: [-s, -s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
            MeshVertex { position: [-s,  s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
            MeshVertex { position: [-s,  s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        ]);
        // +Y
        v.extend([
            MeshVertex { position: [-s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [-s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        ]);
        // -Y
        v.extend([
            MeshVertex { position: [-s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [-s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        ]);
        // +Z
        v.extend([
            MeshVertex { position: [ s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [-s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [-s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        ]);
        // -Z
        v.extend([
            MeshVertex { position: [-s, -s, -s], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s, -s, -s], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [ s,  s, -s], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
            MeshVertex { position: [-s,  s, -s], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        ]);
        v
    };
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(device, &verts, &indices)
}
