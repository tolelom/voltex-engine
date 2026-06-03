//! voltex_survivor — v1.0 flagship game.
//!
//! Game logic lives in `scripts/*.lua` + `scene.vscn`. The Rust side does
//! three things:
//!
//! 1. Bootstrap `RuntimeApp` with title/size/window.
//! 2. Seed the scene with a ground plane + player + game manager entity.
//! 3. Translate keyboard input into player Transform deltas (WASD/arrows).
//!
//! The flagship test for v1.0: `cargo run -p voltex_survivor` boots a game
//! whose behaviour is fully described by Lua + scene.

use voltex::ecs::{Bounds, MaterialHandle, MeshHandle, ScriptComponent, Tag};
use voltex::math::{Vec3, AABB};
use voltex::prelude::*;
use voltex_renderer::{material::MaterialUniform, mesh::Mesh, vertex::MeshVertex};
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};
use winit::keyboard::KeyCode;

const PLAYER_SPEED: f32 = 4.0;

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Survivor")
            .with_window_size(1280, 720)
            .with_scene_builder(setup_scene)
            .with_gpu_init(upload_meshes)
            .with_system_setup(register_systems),
    )?
    .run()
}

/// Seed: ground plane (visual reference) + player (Tag="player") + game
/// manager (ScriptComponent → game.lua). Every other entity (enemies,
/// projectiles, markers) is created at runtime by Lua.
fn setup_scene(world: &mut World, _registry: &mut ComponentRegistry) {
    // Ground — flat large cube at y = -0.5.
    let ground = world.spawn();
    let mut t_ground = Transform::from_position(Vec3::new(0.0, -0.5, 0.0));
    t_ground.scale = Vec3::new(20.0, 0.1, 20.0);
    world.add(ground, t_ground);
    world.add(ground, Tag("ground".to_string()));

    // Player.
    let player = world.spawn();
    world.add(player, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
    world.add(player, Tag("player".to_string()));
    world.add(player, ScriptComponent::new("scripts/player.lua"));

    // Game manager — invisible entity that just runs game.lua.
    let manager = world.spawn();
    world.add(manager, Transform::new());
    world.add(manager, Tag("game_manager".to_string()));
    world.add(manager, ScriptComponent::new("scripts/game.lua"));
}

/// Upload shared cube mesh + material, then attach handles to every
/// entity that has a Transform (covers ground + player + game_manager).
fn upload_meshes(app: &mut RuntimeApp, device: &wgpu::Device, queue: &wgpu::Queue) {
    let _ = queue;
    let mesh = make_unit_cube(device);
    let mesh_h = app.assets.add_mesh(mesh);
    let mat_h = app.assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));

    // Attach to the three seed entities.
    let entities: Vec<_> = app.world.query::<Transform>().map(|(e, _)| e).collect();
    for e in entities {
        // Skip the game_manager so it stays invisible.
        let is_manager = app
            .world
            .get::<Tag>(e)
            .map(|t| t.0 == "game_manager")
            .unwrap_or(false);
        if is_manager {
            continue;
        }
        app.world.add(e, MeshHandle(mesh_h.0));
        app.world.add(e, MaterialHandle(mat_h.0));
        app.world.add(e, bounds);
    }
}

fn register_systems(app: &mut RuntimeApp) {
    // 1. Player input → Transform.position via the semantic InputMap. WASD and
    //    the arrow keys are bound as two alternatives on the same axes, so the
    //    movement system reads "move_x"/"move_y" without knowing any keycodes —
    //    rebinding lives here, in one place.
    app.input_map
        .bind_axis("move_x", KeyCode::KeyD, KeyCode::KeyA)
        .bind_axis("move_x", KeyCode::ArrowRight, KeyCode::ArrowLeft)
        .bind_axis("move_y", KeyCode::KeyW, KeyCode::KeyS)
        .bind_axis("move_y", KeyCode::ArrowUp, KeyCode::ArrowDown);

    app.add_action_system(|world, input, map, dt| {
        // x = strafe (right +), y = forward (W/Up +); world forward is -Z.
        let mv = map.vector("move_x", "move_y", input);
        let dir = Vec3::new(mv.x, 0.0, -mv.y).normalize_or_zero();
        if dir.x == 0.0 && dir.z == 0.0 {
            return;
        }
        let step = PLAYER_SPEED * dt;

        let player: Option<Entity> = world
            .query::<Tag>()
            .find(|(_, t)| t.0 == "player")
            .map(|(e, _)| e);
        if let Some(p) = player {
            if let Some(t) = world.get_mut::<Transform>(p) {
                t.position.x += dir.x * step;
                t.position.z += dir.z * step;
            }
        }
    });

    // 2. Lua-spawned entities (enemy / projectile) come into the world
    //    without mesh / material handles AND without a ScriptComponent —
    //    `spawn_entity()` only creates a bare entity with Transform. This
    //    system attaches the right ScriptComponent + render handles based
    //    on the tag the Lua script set. Markers (kill_marker /
    //    damage_marker) are intentionally skipped — they stay invisible
    //    and have no script to run.
    app.add_system(|world, _dt| {
        // Find a shared mesh/material handle by sampling the player's.
        let sample = world
            .query::<Tag>()
            .find(|(_, t)| t.0 == "player")
            .map(|(e, _)| e);
        let Some(sample_player) = sample else { return };
        let mesh = world.get::<MeshHandle>(sample_player).copied();
        let mat = world.get::<MaterialHandle>(sample_player).copied();
        let (Some(mesh), Some(mat)) = (mesh, mat) else { return };
        let bounds = Bounds(AABB::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));

        // Collect (entity, tag_name) pairs needing attachment so we can
        // borrow the world mutably afterwards.
        let needy: Vec<(Entity, String)> = world
            .query::<Tag>()
            .filter(|(e, t)| {
                (t.0 == "enemy" || t.0 == "projectile")
                    && world.get::<MeshHandle>(*e).is_none()
            })
            .map(|(e, t)| (e, t.0.clone()))
            .collect();
        for (e, tag) in needy {
            world.add(e, mesh);
            world.add(e, mat);
            world.add(e, bounds);
            // Tag-driven script attachment (the engine_api has no
            // set_script(...) helper — v1.1 work). Marker entities don't
            // get scripts; game.lua handles them via iter_by_tag.
            let path = match tag.as_str() {
                "enemy" => "scripts/enemy.lua",
                "projectile" => "scripts/projectile.lua",
                _ => continue,
            };
            world.add(e, ScriptComponent::new(path));
        }
    });
}

/// Standard 24-vertex unit cube. Same routine as runtime_demo /
/// tutorial_minigame — kept inline so the flagship game has no extra
/// shared helper crate.
fn make_unit_cube(device: &wgpu::Device) -> Mesh {
    let s = 0.5;
    let mut verts: Vec<MeshVertex> = Vec::with_capacity(24);
    verts.extend([
        MeshVertex { position: [ s, -s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s, -s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s,  s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
    ]);
    verts.extend([
        MeshVertex { position: [-s, -s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s, -s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s,  s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
    ]);
    verts.extend([
        MeshVertex { position: [-s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ]);
    verts.extend([
        MeshVertex { position: [-s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ]);
    verts.extend([
        MeshVertex { position: [ s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
    ]);
    verts.extend([
        MeshVertex { position: [-s, -s, -s], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, -s, -s], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s, -s], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s,  s, -s], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ]);
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(device, &verts, &indices)
}
