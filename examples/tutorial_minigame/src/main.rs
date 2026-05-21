//! tutorial_minigame — `docs/tutorial.md` 의 최종 산출물.
//!
//! 1 player + 3 enemy 를 같은 cube mesh + material 로 렌더링하고,
//! enemy 마다 `scripts/enemy.lua` 가 player 를 추적, player 에는
//! `scripts/spinner.lua` 가 부착되어 자전한다. main 본문은 4 줄.

use voltex::ecs::{Bounds, MaterialHandle, MeshHandle, ScriptComponent, Tag};
use voltex::math::{Vec3, AABB};
use voltex::prelude::*;
use voltex_renderer::{material::MaterialUniform, mesh::Mesh, vertex::MeshVertex};
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Tutorial Minigame")
            .with_window_size(1280, 720)
            .with_scene_builder(setup_scene)
            .with_gpu_init(upload_meshes)
            .with_system_setup(register_score_system),
    )?
    .run()
}

/// Chapter 4 — player + 3 enemy + 각자의 Lua 스크립트.
fn setup_scene(world: &mut World, _registry: &mut ComponentRegistry) {
    // Player at origin.
    let player = world.spawn();
    world.add(player, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
    world.add(player, Tag("player".to_string()));
    world.add(
        player,
        ScriptComponent::new("scripts/spinner.lua"),
    );

    // Three enemies around the player.
    let enemy_positions = [
        Vec3::new(3.0, 0.5, 0.0),
        Vec3::new(-2.5, 0.5, 2.0),
        Vec3::new(0.5, 0.5, -3.5),
    ];
    for pos in enemy_positions {
        let e = world.spawn();
        world.add(e, Transform::from_position(pos));
        world.add(e, Tag("enemy".to_string()));
        world.add(e, ScriptComponent::new("scripts/enemy.lua"));
    }
}

/// GPU mesh + material 한 세트만 만들어 모든 entity 에 공유.
fn upload_meshes(app: &mut RuntimeApp, device: &wgpu::Device, queue: &wgpu::Queue) {
    let _ = queue;
    let mesh = make_unit_cube(device);
    let mesh_h = app.assets.add_mesh(mesh);
    let mat_h = app.assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)));

    let entities: Vec<_> = app.world.query::<Transform>().map(|(e, _)| e).collect();
    for e in entities {
        app.world.add(e, MeshHandle(mesh_h.0));
        app.world.add(e, MaterialHandle(mat_h.0));
        app.world.add(e, bounds);
    }
}

/// Chapter 5 — 살아있는 enemy 수를 stdout 에 (변경 시에만) 출력.
fn register_score_system(app: &mut RuntimeApp) {
    let mut last_count: usize = usize::MAX;
    app.add_system(move |world, _dt| {
        let enemies = world
            .query::<Tag>()
            .filter(|(_, t)| t.0 == "enemy")
            .count();
        if enemies != last_count {
            println!("[score] enemies alive: {}", enemies);
            last_count = enemies;
        }
    });
}

/// 24 vertex / 36 index 큐브. 한 면당 4 vertex, 정면 노멀 / 단위 UV.
fn make_unit_cube(device: &wgpu::Device) -> Mesh {
    let s = 0.5;
    let mut verts: Vec<MeshVertex> = Vec::with_capacity(24);
    // +X
    verts.extend([
        MeshVertex { position: [ s, -s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s, -s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
        MeshVertex { position: [ s,  s, -s], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, 1.0, 1.0] },
    ]);
    // -X
    verts.extend([
        MeshVertex { position: [-s, -s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s, -s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s,  s, -s], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0], tangent: [0.0, 0.0, -1.0, 1.0] },
    ]);
    // +Y
    verts.extend([
        MeshVertex { position: [-s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ]);
    // -Y
    verts.extend([
        MeshVertex { position: [-s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, -s,  s], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, -s, -s], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0], tangent: [1.0, 0.0, 0.0, 1.0] },
    ]);
    // +Z
    verts.extend([
        MeshVertex { position: [ s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s, -s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [-s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
        MeshVertex { position: [ s,  s,  s], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0], tangent: [-1.0, 0.0, 0.0, 1.0] },
    ]);
    // -Z
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
