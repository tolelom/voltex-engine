//! boids_demo — capstone for the v1.0.62–80 authoring/runtime stack.
//!
//! A swarm of cubes is:
//! - **seeded with [`Rng`]** (random start positions + headings, deterministic),
//! - **declared with `PrimitiveMesh` + `MaterialDesc`** (no manual GPU upload),
//! - **spawned with `spawn_bundle`**, and
//! - **steered every frame by `flocking_system`** (separation/cohesion/alignment
//!   over a `SpatialGrid`), with a toroidal wrap to keep the flock on screen.
//!
//! Run: `cargo run -p boids_demo`. (No controls — sit back and watch the swarm.)

use voltex::prelude::*;
use voltex_ai::{flocking_system, Boid, BoidParams};
use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

const NUM_BOIDS: usize = 120;
const ARENA: f32 = 14.0; // half-extent of the square the flock wraps within
const SPEED: f32 = 6.0;

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Boids — flocking swarm")
            .with_window_size(1280, 720)
            // Look straight down at the arena (top-down).
            .with_camera(Vec3::new(0.0, 36.0, 0.01), 0.0, -std::f32::consts::FRAC_PI_2)
            .with_light(Vec3::new(-0.3, -1.0, -0.2), Vec3::new(1.0, 0.97, 0.9), 0.25)
            .disable_physics()
            .disable_scripts()
            .with_scene_builder(seed_swarm)
            .with_system_setup(register_flocking),
    )?
    .run()
}

/// Spawn the flock: each boid is a small cube with a per-boid color, a random
/// position in the arena, and a random initial heading in the XZ plane.
fn seed_swarm(world: &mut World, _registry: &mut ComponentRegistry) {
    let mut rng = Rng::seeded(0xB0_1D5);

    // Ground for visual reference.
    let mut ground = Transform::from_position(Vec3::new(0.0, -0.2, 0.0));
    ground.scale = Vec3::new(ARENA * 2.2, 0.2, ARENA * 2.2);
    world.spawn_bundle((
        ground,
        PrimitiveMesh::cube(1.0),
        MaterialDesc::color([0.12, 0.13, 0.16, 1.0]),
    ));

    for _ in 0..NUM_BOIDS {
        let pos = Vec3::new(
            rng.range_f32(-ARENA, ARENA),
            0.5,
            rng.range_f32(-ARENA, ARENA),
        );
        let heading = rng.unit_vec2(); // XZ plane direction
        let velocity = Vec3::new(heading.x, 0.0, heading.y) * SPEED;
        // Warm random color so individuals are distinguishable in the swarm.
        let color = [
            rng.range_f32(0.5, 1.0),
            rng.range_f32(0.4, 0.8),
            rng.range_f32(0.2, 0.5),
            1.0,
        ];
        world.spawn_bundle((
            Transform::from_position(pos),
            PrimitiveMesh::cube(0.5),
            MaterialDesc::color(color),
            Boid::new(velocity),
        ));
    }
}

/// Run flocking each frame, then wrap boids toroidally so the swarm never
/// leaves the arena (and the camera framing).
fn register_flocking(app: &mut RuntimeApp) {
    let params = BoidParams {
        neighbour_radius: 3.0,
        separation: 2.2,
        cohesion: 0.8,
        alignment: 1.1,
        max_speed: SPEED,
    };
    app.add_system(move |world, dt| {
        flocking_system(world, &params, dt);

        // Toroidal wrap: collect strays first, then mutate (avoids aliasing).
        let wrapped: Vec<(Entity, Vec3)> = world
            .query2::<Transform, Boid>()
            .filter_map(|(e, t, _)| {
                let mut p = t.position;
                let mut changed = false;
                if p.x > ARENA {
                    p.x -= ARENA * 2.0;
                    changed = true;
                } else if p.x < -ARENA {
                    p.x += ARENA * 2.0;
                    changed = true;
                }
                if p.z > ARENA {
                    p.z -= ARENA * 2.0;
                    changed = true;
                } else if p.z < -ARENA {
                    p.z += ARENA * 2.0;
                    changed = true;
                }
                changed.then_some((e, p))
            })
            .collect();
        for (e, p) in wrapped {
            if let Some(t) = world.get_mut::<Transform>(e) {
                t.position = p;
            }
        }
    });
}
