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
use voltex::physics::Collider;
use voltex_ai::{nav_agent_system, NavAgent, NavMesh, NavTriangle};
use voltex_audio::{AudioClip, AudioSource};
use voltex_renderer::gltf::{
    AnimationPath, GltfAnimation, GltfChannel, GltfNode, GltfSkin, Interpolation,
};
use voltex_renderer::{
    material::MaterialUniform, mesh::Mesh, vertex::MeshVertex, PointLight, Skeleton, SkinnedMesh,
    SkinnedMeshVertex,
};
use voltex_renderer_ecs::SkinnedPose;
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

const PLAYER_TAG: &str = "player";
const CHASER_TAG: &str = "chaser";
const TOTEM_TAG: &str = "totem";

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
            // Warm, angled sun so faces shade distinctly (vs the default
            // straight-down-ish white light).
            .with_light(
                Vec3::new(-0.5, -1.0, -0.4),
                Vec3::new(1.0, 0.95, 0.85),
                0.15,
            )
            .with_audio(vec![beep_clip()])
            // No rigid-body dynamics here — collisions are used only as
            // trigger events (the set_collision_fn catch detector).
            .disable_physics()
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
    world.add(player, Collider::Sphere { radius: 0.6 });

    // Chaser: nav-mesh agent that hunts the player, beeps on catch.
    let chaser = world.spawn();
    let mut ct = Transform::from_position(Vec3::new(10.0, 0.5, 10.0));
    ct.scale = Vec3::new(0.8, 0.8, 0.8);
    world.add(chaser, ct);
    world.add(chaser, Tag(CHASER_TAG.to_string()));
    world.add(chaser, NavAgent::new(3.5));
    world.add(chaser, AudioSource::new(0u32).spatial());
    world.add(chaser, Collider::Sphere { radius: 0.5 });

    // Totem: a stationary GPU-skinned strip that waves — shows skeletal
    // animation in the live runtime (mesh + pose added in gpu_init).
    let totem = world.spawn();
    let mut tt = Transform::from_position(Vec3::new(-5.0, 0.0, -3.0));
    tt.scale = Vec3::new(1.5, 1.5, 1.5);
    world.add(totem, tt);
    world.add(totem, Tag(TOTEM_TAG.to_string()));
}

fn upload_meshes(app: &mut RuntimeApp, device: &wgpu::Device, queue: &wgpu::Queue) {
    let _ = queue;
    let mesh_h = app.assets.add_mesh(make_unit_cube(device));
    // Distinct per-role colors so the player/chaser stand out from the ground
    // (BasicRenderer now honours MaterialUniform.base_color).
    let ground_mat = app
        .assets
        .add_material(MaterialUniform::with_params([0.35, 0.37, 0.40, 1.0], 0.0, 1.0));
    let player_mat = app
        .assets
        .add_material(MaterialUniform::with_params([0.20, 0.50, 1.00, 1.0], 0.0, 1.0));
    let chaser_mat = app
        .assets
        .add_material(MaterialUniform::with_params([1.00, 0.30, 0.20, 1.0], 0.0, 1.0));
    let bounds = Bounds(AABB::new(
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
    ));
    // Cube mesh for ground/player/chaser; the totem gets a skinned mesh below.
    let assignments: Vec<(Entity, MaterialHandle)> = app
        .world
        .query::<Tag>()
        .filter_map(|(e, tag)| match tag.0.as_str() {
            PLAYER_TAG => Some((e, player_mat)),
            CHASER_TAG => Some((e, chaser_mat)),
            TOTEM_TAG => None, // skinned, handled separately
            _ => Some((e, ground_mat)),
        })
        .collect();
    for (e, mat) in assignments {
        app.world.add(e, MeshHandle(mesh_h.0));
        app.world.add(e, MaterialHandle(mat.0));
        app.world.add(e, bounds);
    }

    // Skinned totem: GPU vertex-skinned strip + a looping wave animation.
    let strip_h = app.assets.add_skinned_mesh(skinned_strip(device));
    let totem = app
        .world
        .query::<Tag>()
        .find(|(_, t)| t.0 == TOTEM_TAG)
        .map(|(e, _)| e);
    if let Some(e) = totem {
        app.world.add(e, strip_h);
        app.world
            .add(e, SkinnedPose::new(two_bone_skeleton(), wave_animation()));
    }
}

/// A 4-vertex vertical strip (y 0→2): bottom edge bound to joint 0, top edge to
/// joint 1. Front face +Z.
fn skinned_strip(device: &wgpu::Device) -> SkinnedMesh {
    let mk = |x: f32, y: f32, joint: u32| SkinnedMeshVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
        joints: [joint, 0, 0, 0],
        weights: [1.0, 0.0, 0.0, 0.0],
    };
    let verts = [
        mk(-0.5, 0.0, 0),
        mk(0.5, 0.0, 0),
        mk(0.5, 2.0, 1),
        mk(-0.5, 2.0, 1),
    ];
    let indices = [0u32, 1, 2, 0, 2, 3];
    SkinnedMesh::new(device, &verts, &indices)
}

/// Two-joint skeleton: root at origin, child pivot at y=1 (inverse-bind = the
/// inverse of its bind world, so the rest pose is undeformed).
fn two_bone_skeleton() -> Skeleton {
    let node = |t: [f32; 3], children: Vec<usize>| GltfNode {
        name: None,
        children,
        translation: t,
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
        mesh: None,
        skin: None,
    };
    let nodes = vec![node([0.0, 0.0, 0.0], vec![1]), node([0.0, 1.0, 0.0], vec![])];
    let skin = GltfSkin {
        name: None,
        joints: vec![0, 1],
        inverse_bind_matrices: vec![
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0, 1.0],
            ],
        ],
        skeleton: None,
    };
    Skeleton::from_skin(&skin, &nodes)
}

/// Looping wave: child joint rotates -35° → +35° → -35° about Z over 2s.
fn wave_animation() -> GltfAnimation {
    let q = |deg: f32| {
        let a = deg.to_radians() * 0.5;
        [0.0, 0.0, a.sin(), a.cos()]
    };
    let a = q(-35.0);
    let b = q(35.0);
    GltfAnimation {
        name: None,
        channels: vec![GltfChannel {
            target_node: 1,
            target_path: AnimationPath::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0, 2.0],
            values: vec![
                a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], a[0], a[1], a[2], a[3],
            ],
        }],
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

    // Two colored arena point lights (warm + cool) for some dynamic-looking
    // shading on top of the rotating sun.
    app.set_point_lights(vec![
        PointLight::new([6.0, 4.0, 6.0], [1.0, 0.6, 0.3], 18.0, 2.5),
        PointLight::new([-6.0, 4.0, -6.0], [0.3, 0.5, 1.0], 18.0, 2.5),
    ]);

    let stats = Rc::new(RefCell::new(DemoState::default()));

    // Chase system: target the player, run nav, track distance + elapsed time.
    let navmesh = flat_navmesh();
    let chase_stats = stats.clone();
    app.add_system(move |world, dt| {
        let Some(player_pos) = tagged_position(world, PLAYER_TAG) else {
            return;
        };
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

        if let Some(chaser_pos) = tagged_position(world, CHASER_TAG) {
            let dx = chaser_pos.x - player_pos.x;
            let dz = chaser_pos.z - player_pos.z;
            let mut s = chase_stats.borrow_mut();
            s.dist = (dx * dx + dz * dz).sqrt();
            s.time += dt;
        }
    });

    // Catch detection via the engine's collision events (player + chaser carry
    // sphere Colliders). Rising edge → beep + bump the counter.
    let coll_stats = stats.clone();
    app.set_collision_fn(move |contacts, world, _dt| {
        let is_player_chaser = |a: Option<&str>, b: Option<&str>| {
            (a == Some(PLAYER_TAG) && b == Some(CHASER_TAG))
                || (a == Some(CHASER_TAG) && b == Some(PLAYER_TAG))
        };
        let mut caught = false;
        for c in contacts {
            let ta = world.get::<Tag>(c.entity_a).map(|t| t.0.clone());
            let tb = world.get::<Tag>(c.entity_b).map(|t| t.0.clone());
            if is_player_chaser(ta.as_deref(), tb.as_deref()) {
                caught = true;
            }
        }
        // Beep the chaser on the catch (rising edge handled by audio_sync).
        let chaser = world
            .query::<Tag>()
            .find(|(_, t)| t.0 == CHASER_TAG)
            .map(|(e, _)| e);
        if let Some(e) = chaser {
            if let Some(src) = world.get_mut::<AudioSource>(e) {
                src.playing = caught;
            }
        }
        let mut s = coll_stats.borrow_mut();
        if caught && !s.caught {
            s.caught_count += 1;
        }
        s.caught = caught;
    });

    // Follow camera: frame the player from behind/above so the gameplay is
    // actually visible (the default runtime camera sits far back at (0,10,20),
    // which leaves a 1-unit cube a tiny speck). Uses the new set_camera_fn API.
    app.set_camera_fn(|cam, world, _dt| {
        if let Some(p) = tagged_position(world, PLAYER_TAG) {
            cam.position = p + Vec3::new(0.0, 9.0, 13.0);
            cam.yaw = 0.0;
            cam.pitch = -0.55;
        }
    });

    // Slowly rotate the sun around Y (day/night feel) — shows the per-frame
    // light hook driving shading live.
    app.set_light_fn(|light, _world, dt| {
        let a = 0.3 * dt;
        let (s, c) = a.sin_cos();
        let d = light.direction;
        light.direction = [c * d[0] + s * d[2], d[1], -s * d[0] + c * d[2]];
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
        // Danger meter — fills red as the chaser closes in (progress_bar HUD helper).
        let danger = (1.0 - (s.dist / 15.0).min(1.0)).clamp(0.0, 1.0);
        ui.progress_bar(12.0, 130.0, 200.0, 14.0, danger, [220, 60, 40, 255], [40, 40, 40, 255]);
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
