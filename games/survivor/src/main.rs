//! survivor — v1.0 flagship game.
//!
//! Game logic lives in `scripts/*.lua` + `scene.vscn`. The Rust side does
//! three things:
//!
//! 1. Bootstrap `RuntimeApp` with title/size/window.
//! 2. Seed the scene with a ground plane + player + game manager entity.
//! 3. Translate keyboard input into player Transform deltas (WASD/arrows).
//!
//! The flagship test for v1.0: `cargo run -p survivor` boots a game
//! whose behaviour is fully described by Lua + scene.

use voltex::ecs::Tag;
use voltex::math::Vec3;
use voltex::prelude::*;
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

const PLAYER_SPEED: f32 = 4.0;

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Survivor")
            .with_window_size(1280, 720)
            .with_scene_builder(setup_scene)
            .with_system_setup(register_systems),
    )?
    .run()
}

/// Seed: ground plane (visual reference) + player (Tag="player") + game
/// manager (ScriptComponent → game.lua). Every other entity (enemies,
/// projectiles, markers) is created at runtime by Lua.
///
/// Shapes + colors are declared by value (`PrimitiveMesh` + `MaterialDesc`);
/// the runtime materializes the GPU meshes/materials each frame, so there is no
/// `gpu_init` upload phase and no hand-written cube mesh.
fn setup_scene(world: &mut World, _registry: &mut ComponentRegistry) {
    // Ground — flat large gray cube at y = -0.5.
    let mut t_ground = Transform::from_position(Vec3::new(0.0, -0.5, 0.0));
    t_ground.scale = Vec3::new(20.0, 0.1, 20.0);
    world.spawn_bundle((
        t_ground,
        Tag("ground".to_string()),
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.30, 0.32, 0.35, 1.0]),
    ));

    // Player — green cube, driven by player.lua + the action system.
    world.spawn_bundle((
        Transform::from_position(Vec3::new(0.0, 0.5, 0.0)),
        Tag("player".to_string()),
        ScriptComponent::new("scripts/player.lua"),
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.20, 0.80, 0.30, 1.0]),
    ));

    // Game manager — invisible entity that just runs game.lua (no renderable).
    world.spawn_bundle((
        Transform::new(),
        Tag("game_manager".to_string()),
        ScriptComponent::new("scripts/game.lua"),
    ));
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

        if let Some(p) = world.find_by_tag("player") {
            if let Some(t) = world.get_mut::<Transform>(p) {
                t.position.x += dir.x * step;
                t.position.z += dir.z * step;
            }
        }
    });

    // 2. Lua-spawned entities (enemy / projectile) arrive as bare Transform
    //    entities — `spawn_entity()` only creates that. Attach a declarative
    //    renderable (colored by role; per-frame materialize turns it into a GPU
    //    mesh) plus the matching script, keyed by the tag the Lua script set.
    //    Markers (kill_marker / damage_marker) stay invisible — no renderable,
    //    no script; game.lua handles them via iter_by_tag.
    app.add_system(|world, _dt| {
        // Entities still needing their renderable/script attached.
        let needy: Vec<(Entity, String)> = world
            .query::<Tag>()
            .filter(|(e, t)| {
                (t.0 == "enemy" || t.0 == "projectile")
                    && world.get::<PrimitiveMesh>(*e).is_none()
            })
            .map(|(e, t)| (e, t.0.clone()))
            .collect();
        for (e, tag) in needy {
            let (color, script) = match tag.as_str() {
                "enemy" => ([0.90, 0.25, 0.20, 1.0], "scripts/enemy.lua"),
                "projectile" => ([1.00, 0.90, 0.30, 1.0], "scripts/projectile.lua"),
                _ => continue,
            };
            world.insert_bundle(
                e,
                (
                    PrimitiveMesh::cube(1.0),
                    MaterialDesc::color(color),
                    ScriptComponent::new(script),
                ),
            );
        }
    });
}
