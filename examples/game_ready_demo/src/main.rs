//! game_ready_demo — drive the v1.0.59 "game-ready" wiring by hand, in one
//! window:
//!
//! - **Lua input**: a `player_move.lua` script reads `input.is_key_pressed`
//!   and moves the player cube with WASD.
//! - **nav**: a chaser cube runs `nav_agent_system` to path toward the player
//!   every frame.
//! - **audio**: the chaser carries a spatial `AudioSource`; it beeps on the
//!   frame it catches the player (listener = player).
//! - **HUD**: `set_hud_fn` draws distance / status / counters on screen.
//!
//! Controls: **WASD** move the player (with RMB released). Hold **RMB + drag**
//! to orbit the fly-camera. **Esc** quits.
//!
//! (Skinned animation is verified separately — see `docs/visual/skinned_*.png`
//! — because the runtime render path does not yet do GPU skinning; that is the
//! deferred v1.1 ECS-skinning integration.)

use std::cell::RefCell;
use std::rc::Rc;

use voltex::ecs::{Bounds, Entity, MaterialHandle, MeshHandle, ScriptComponent, Tag};
use voltex::math::{Vec3, AABB};
use voltex::prelude::*;
use voltex_ai::{nav_agent_system, NavAgent, NavMesh, NavTriangle};
use voltex_audio::{AudioClip, AudioSource};
use voltex_renderer::{material::MaterialUniform, mesh::Mesh, vertex::MeshVertex};
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

const PLAYER_TAG: &str = "player";
const CHASER_TAG: &str = "chaser";
const CATCH_RADIUS: f32 = 1.2;

/// The player-movement Lua script (written to a temp file at startup so the
/// path is valid regardless of the working directory).
const PLAYER_LUA: &str = r#"
local SPEED = 6.0
function on_start(id, gen) end
function on_update(id, gen, dt)
    local x, y, z = get_position(id, gen)
    if x == nil then return end
    local dx, dz = 0.0, 0.0
    if input.is_key_pressed("W") then dz = dz - 1.0 end
    if input.is_key_pressed("S") then dz = dz + 1.0 end
    if input.is_key_pressed("A") then dx = dx - 1.0 end
    if input.is_key_pressed("D") then dx = dx + 1.0 end
    set_position(id, gen, x + dx * SPEED * dt, y, z + dz * SPEED * dt)
end
"#;

/// Shared gameplay readout: written by the chase system, read by the HUD.
#[derive(Default)]
struct DemoState {
    dist: f32,
    caught: bool,
    caught_count: u32,
    time: f32,
}

fn main() -> RuntimeResult<()> {
    env_logger::init();
    let script_path = write_player_script();

    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Game-Ready Demo — WASD to move, chase by nav AI")
            .with_window_size(1280, 720)
            .with_hud(true)
            .with_audio(vec![beep_clip()])
            .with_scene_builder(move |world, reg| setup_scene(world, reg, &script_path))
            .with_gpu_init(upload_meshes)
            .with_system_setup(register_systems),
    )?
    .run()
}

fn setup_scene(world: &mut World, _registry: &mut ComponentRegistry, script_path: &str) {
    // Ground: a big flat scaled cube at y ≈ 0.
    let ground = world.spawn();
    let mut gt = Transform::from_position(Vec3::new(0.0, -0.1, 0.0));
    gt.scale = Vec3::new(40.0, 0.2, 40.0);
    world.add(ground, gt);
    world.add(ground, Tag("ground".to_string()));

    // Player: Lua-controlled cube at the origin.
    let player = world.spawn();
    world.add(player, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
    world.add(player, Tag(PLAYER_TAG.to_string()));
    world.add(player, ScriptComponent::new(script_path.to_string()));

    // Chaser: nav-mesh agent that hunts the player, beeps on catch.
    let chaser = world.spawn();
    let mut ct = Transform::from_position(Vec3::new(10.0, 0.5, 10.0));
    ct.scale = Vec3::new(0.8, 0.8, 0.8);
    world.add(chaser, ct);
    world.add(chaser, Tag(CHASER_TAG.to_string()));
    world.add(chaser, NavAgent::new(3.5));
    world.add(chaser, AudioSource::new(0u32).spatial());
}

fn upload_meshes(app: &mut RuntimeApp, device: &wgpu::Device, queue: &wgpu::Queue) {
    let _ = queue;
    let mesh_h = app.assets.add_mesh(make_unit_cube(device));
    let mat_h = app.assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
    ));
    let entities: Vec<_> = app.world.query::<Transform>().map(|(e, _)| e).collect();
    for e in entities {
        app.world.add(e, MeshHandle(mesh_h.0));
        app.world.add(e, MaterialHandle(mat_h.0));
        app.world.add(e, bounds);
    }
}

fn register_systems(app: &mut RuntimeApp) {
    // 3D audio listener = the player, so the chaser's beep pans with its
    // position relative to the player. Bind first so the query borrow ends
    // before the mutable `set_audio_listener` call.
    let player = app
        .world
        .query2::<Tag, Transform>()
        .find(|(_, t, _)| t.0 == PLAYER_TAG)
        .map(|(e, _, _)| e);
    if let Some(player) = player {
        app.set_audio_listener(player);
    }

    let stats = Rc::new(RefCell::new(DemoState::default()));

    // Chase system: target the player, run nav, update audio + readout.
    let navmesh = flat_navmesh();
    let chase_stats = stats.clone();
    app.add_system(move |world, dt| {
        let Some(player_pos) = tagged_position(world, PLAYER_TAG) else {
            return;
        };

        // Point every chaser at the player (re-path each frame as it moves).
        let chasers: Vec<Entity> = world
            .query2::<Tag, NavAgent>()
            .filter(|(_, t, _)| t.0 == CHASER_TAG)
            .map(|(e, _, _)| e)
            .collect();
        for e in &chasers {
            if let Some(agent) = world.get_mut::<NavAgent>(*e) {
                agent.set_target(player_pos);
            }
        }

        nav_agent_system(world, &navmesh, dt);

        // Distance + catch detection (single chaser for the demo).
        let Some(chaser_pos) = tagged_position(world, CHASER_TAG) else {
            return;
        };
        let dx = chaser_pos.x - player_pos.x;
        let dz = chaser_pos.z - player_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let caught = dist < CATCH_RADIUS;

        // Drive the chaser's AudioSource: rising edge (just caught) → beep.
        for e in &chasers {
            if let Some(src) = world.get_mut::<AudioSource>(*e) {
                src.playing = caught;
            }
        }

        let mut s = chase_stats.borrow_mut();
        if caught && !s.caught {
            s.caught_count += 1;
        }
        s.caught = caught;
        s.dist = dist;
        s.time += dt;
    });

    // HUD: read the shared readout each frame.
    let hud_stats = stats;
    app.set_hud_fn(move |ui, _world| {
        let s = hud_stats.borrow();
        ui.text("Voltex Game-Ready Demo");
        ui.text("WASD: move player   RMB+drag: look   Esc: quit");
        ui.text(&format!("Distance to chaser: {:.1}", s.dist));
        ui.text(if s.caught { ">>> CAUGHT! <<<" } else { "RUN!" });
        ui.text(&format!("Times caught: {}", s.caught_count));
        ui.text(&format!("Elapsed: {:.1}s", s.time));
    });
}

/// World-space position of the first entity carrying `tag` (and a Transform).
fn tagged_position(world: &World, tag: &str) -> Option<Vec3> {
    world
        .query2::<Tag, Transform>()
        .find(|(_, t, _)| t.0 == tag)
        .map(|(_, _, tr)| tr.position)
}

/// A single large triangle on the XZ plane — enough for the chaser to path
/// across the whole play area (`find_path` returns a straight line within it).
fn flat_navmesh() -> NavMesh {
    let vertices = vec![
        Vec3::new(-100.0, 0.0, -100.0),
        Vec3::new(100.0, 0.0, -100.0),
        Vec3::new(0.0, 0.0, 100.0),
    ];
    let triangles = vec![NavTriangle {
        indices: [0, 1, 2],
        neighbors: [None, None, None],
    }];
    NavMesh::new(vertices, triangles)
}

/// A short decaying 660 Hz sine — the "caught" beep.
fn beep_clip() -> AudioClip {
    let sample_rate = 44_100u32;
    let duration = 0.15f32;
    let n = (sample_rate as f32 * duration) as usize;
    let freq = 660.0f32;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let env = (1.0 - t / duration).max(0.0);
            (2.0 * std::f32::consts::PI * freq * t).sin() * 0.3 * env
        })
        .collect();
    AudioClip::new(samples, sample_rate, 1)
}

fn write_player_script() -> String {
    let dir = std::env::temp_dir().join("voltex_game_ready_demo");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("player_move.lua");
    std::fs::write(&path, PLAYER_LUA).expect("write player_move.lua");
    path.to_string_lossy().into_owned()
}

/// 24-vertex unit cube (per-face normals), shared by ground / player / chaser.
fn make_unit_cube(device: &wgpu::Device) -> Mesh {
    let s = 0.5;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[s, -s, -s], [s, -s, s], [s, s, s], [s, s, -s]]),
        ([-1.0, 0.0, 0.0], [[-s, -s, s], [-s, -s, -s], [-s, s, -s], [-s, s, s]]),
        ([0.0, 1.0, 0.0], [[-s, s, -s], [s, s, -s], [s, s, s], [-s, s, s]]),
        ([0.0, -1.0, 0.0], [[-s, -s, s], [s, -s, s], [s, -s, -s], [-s, -s, -s]]),
        ([0.0, 0.0, 1.0], [[s, -s, s], [-s, -s, s], [-s, s, s], [s, s, s]]),
        ([0.0, 0.0, -1.0], [[-s, -s, -s], [s, -s, -s], [s, s, -s], [-s, s, -s]]),
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut verts = Vec::with_capacity(24);
    for (normal, corners) in faces {
        for (corner, uv) in corners.iter().zip(uvs.iter()) {
            verts.push(MeshVertex {
                position: *corner,
                normal,
                uv: *uv,
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
    }
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(device, &verts, &indices)
}
