//! Boid flocking steering — the three classic neighbour-based rules
//! (separation, cohesion, alignment). Each takes the *nearby* agents the caller
//! has already gathered, so the expensive part — finding neighbours — is done
//! once per agent with [`voltex_ecs::SpatialGrid`] instead of an O(n²) scan.
//!
//! ```ignore
//! let grid = SpatialGrid::from_transforms(&world, neighbour_radius);
//! let neighbours: Vec<Vec3> = grid
//!     .query_radius(my_pos, neighbour_radius)
//!     .into_iter()
//!     .filter(|&e| e != me)
//!     .map(|e| world.get::<Transform>(e).unwrap().position)
//!     .collect();
//! let steer = separation(my_pos, &neighbours) * sep_weight
//!           + cohesion(my_pos, &neighbours) * coh_weight;
//! ```
//!
//! All three return a **direction** (unit vector, or zero when there are no
//! neighbours / the result is degenerate); scale by a weight and feed into your
//! velocity integration.

use std::collections::HashMap;

use voltex_ecs::{Entity, SpatialGrid, Transform, World};
use voltex_math::Vec3;

/// Neighbours closer than this are treated as coincident and ignored by [`separation`].
const COINCIDENT_DISTANCE: f32 = 1e-6;

/// Velocity carried by a flocking agent. Attach alongside a [`Transform`]; the
/// [`flocking_system`] integrates it each tick.
#[derive(Debug, Clone, Copy)]
pub struct Boid {
    /// Current velocity (world units / second).
    pub velocity: Vec3,
}

impl Default for Boid {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
        }
    }
}

impl Boid {
    /// A boid with the given initial velocity.
    pub fn new(velocity: Vec3) -> Self {
        Self { velocity }
    }
}

/// Tuning for [`flocking_system`]: neighbour search radius, the three rule
/// weights, and a speed cap.
#[derive(Debug, Clone, Copy)]
pub struct BoidParams {
    /// Agents within this distance count as neighbours.
    pub neighbour_radius: f32,
    /// Weight (multiplier) of the [`separation`] rule.
    pub separation: f32,
    /// Weight (multiplier) of the [`cohesion`] rule.
    pub cohesion: f32,
    /// Weight (multiplier) of the [`alignment`] rule.
    pub alignment: f32,
    /// Maximum speed; velocity is clamped to this after applying forces.
    pub max_speed: f32,
}

impl Default for BoidParams {
    fn default() -> Self {
        Self {
            neighbour_radius: 3.0,
            separation: 1.5,
            cohesion: 1.0,
            alignment: 1.0,
            max_speed: 5.0,
        }
    }
}

/// Advance every `(Transform, Boid)` entity by one flocking step: gather each
/// boid's neighbours via a [`SpatialGrid`] (so this is ~O(n) for evenly spread
/// crowds, not O(n²)), combine the three boid rules into an acceleration,
/// integrate velocity (clamped to `params.max_speed`) and position.
///
/// All boids are read into a snapshot first, so every boid reacts to the *same*
/// frame's state regardless of iteration order.
pub fn flocking_system(world: &mut World, params: &BoidParams, dt: f32) {
    let boids: Vec<(Entity, Vec3, Vec3)> = world
        .query2::<Transform, Boid>()
        .map(|(entity, transform, boid)| (entity, transform.position, boid.velocity))
        .collect();
    if boids.is_empty() {
        return;
    }

    let mut grid = SpatialGrid::new(params.neighbour_radius);
    let mut position_velocity: HashMap<Entity, (Vec3, Vec3)> = HashMap::with_capacity(boids.len());
    for &(entity, position, velocity) in &boids {
        grid.insert(entity, position);
        position_velocity.insert(entity, (position, velocity));
    }

    let mut neighbour_buf = Vec::new();
    let mut updates: Vec<(Entity, Vec3, Vec3)> = Vec::with_capacity(boids.len());
    for &(entity, position, velocity) in &boids {
        grid.query_radius_into(position, params.neighbour_radius, &mut neighbour_buf);
        let mut neighbour_positions = Vec::new();
        let mut neighbour_velocities = Vec::new();
        for &neighbour in &neighbour_buf {
            if neighbour == entity {
                continue;
            }
            let (neighbour_position, neighbour_velocity) = position_velocity[&neighbour];
            neighbour_positions.push(neighbour_position);
            neighbour_velocities.push(neighbour_velocity);
        }
        let acceleration = separation(position, &neighbour_positions) * params.separation
            + cohesion(position, &neighbour_positions) * params.cohesion
            + alignment(&neighbour_velocities) * params.alignment;
        let mut new_velocity = velocity + acceleration * dt;
        let speed = new_velocity.length();
        if speed > params.max_speed {
            new_velocity = new_velocity.normalize_or_zero() * params.max_speed;
        }
        let new_position = position + new_velocity * dt;
        updates.push((entity, new_position, new_velocity));
    }

    for (entity, new_position, new_velocity) in updates {
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.position = new_position;
        }
        if let Some(boid) = world.get_mut::<Boid>(entity) {
            boid.velocity = new_velocity;
        }
    }
}

/// Steer away from nearby `neighbours`, weighted by inverse distance so closer
/// crowders push harder. Returns a unit direction away from the crowd, or zero
/// if there are no (non-coincident) neighbours.
pub fn separation(position: Vec3, neighbours: &[Vec3]) -> Vec3 {
    let mut push = Vec3::ZERO;
    for &neighbour in neighbours {
        let away = position - neighbour;
        let len = away.length();
        if len > COINCIDENT_DISTANCE {
            // away/len is the unit direction; the extra 1/len weights by 1/dist.
            push = push + away * (1.0 / (len * len));
        }
    }
    push.normalize_or_zero()
}

/// Steer toward the average position of `neighbours` (group centre). Unit
/// direction, or zero if there are no neighbours.
pub fn cohesion(position: Vec3, neighbours: &[Vec3]) -> Vec3 {
    if neighbours.is_empty() {
        return Vec3::ZERO;
    }
    let sum = neighbours.iter().copied().fold(Vec3::ZERO, |a, b| a + b);
    let centre = sum * (1.0 / neighbours.len() as f32);
    (centre - position).normalize_or_zero()
}

/// Steer to match the average heading of `neighbour_velocities`. Unit
/// direction, or zero if there are no neighbours / the average is ~still.
pub fn alignment(neighbour_velocities: &[Vec3]) -> Vec3 {
    if neighbour_velocities.is_empty() {
        return Vec3::ZERO;
    }
    let sum = neighbour_velocities
        .iter()
        .copied()
        .fold(Vec3::ZERO, |a, b| a + b);
    let avg = sum * (1.0 / neighbour_velocities.len() as f32);
    avg.normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn separation_points_away_from_a_single_neighbour() {
        // Neighbour to the +X of us → we steer -X.
        let steer = separation(Vec3::ZERO, &[Vec3::new(2.0, 0.0, 0.0)]);
        assert!(approx(steer, Vec3::new(-1.0, 0.0, 0.0)), "got {steer:?}");
    }

    #[test]
    fn separation_cancels_for_symmetric_neighbours() {
        let steer = separation(
            Vec3::ZERO,
            &[Vec3::new(2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 0.0)],
        );
        assert!(steer.length() < 1e-4, "symmetric crowd should cancel: {steer:?}");
    }

    #[test]
    fn separation_with_no_neighbours_is_zero() {
        assert_eq!(separation(Vec3::ZERO, &[]), Vec3::ZERO);
    }

    #[test]
    fn separation_closer_neighbour_dominates() {
        // A close neighbour on +X and a far one on -X: the close one wins, so we
        // steer -X (away from the close neighbour).
        let steer = separation(
            Vec3::ZERO,
            &[Vec3::new(1.0, 0.0, 0.0), Vec3::new(-10.0, 0.0, 0.0)],
        );
        assert!(steer.x < 0.0, "closer neighbour on +X should push us -X: {steer:?}");
    }

    #[test]
    fn cohesion_points_toward_group_centre() {
        let steer = cohesion(
            Vec3::ZERO,
            &[Vec3::new(4.0, 0.0, 0.0), Vec3::new(6.0, 0.0, 0.0)],
        );
        assert!(approx(steer, Vec3::new(1.0, 0.0, 0.0)), "got {steer:?}");
    }

    #[test]
    fn cohesion_with_no_neighbours_is_zero() {
        assert_eq!(cohesion(Vec3::ZERO, &[]), Vec3::ZERO);
    }

    #[test]
    fn alignment_matches_average_heading() {
        let steer = alignment(&[Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, 5.0)]);
        assert!(approx(steer, Vec3::new(0.0, 0.0, 1.0)), "got {steer:?}");
    }

    #[test]
    fn alignment_with_no_neighbours_is_zero() {
        assert_eq!(alignment(&[]), Vec3::ZERO);
    }

    // ── flocking_system (ECS integration) ──────────────────────────────

    use voltex_ecs::{Transform, World};

    fn spawn_boid(world: &mut World, pos: Vec3, vel: Vec3) -> voltex_ecs::Entity {
        let e = world.spawn();
        world.add(e, Transform::from_position(pos));
        world.add(e, Boid::new(vel));
        e
    }

    fn dist(world: &World, a: voltex_ecs::Entity, b: voltex_ecs::Entity) -> f32 {
        let pa = world.get::<Transform>(a).unwrap().position;
        let pb = world.get::<Transform>(b).unwrap().position;
        (pa - pb).length()
    }

    #[test]
    fn separation_pushes_crowded_boids_apart() {
        let mut world = World::new();
        let a = spawn_boid(&mut world, Vec3::new(-0.25, 0.0, 0.0), Vec3::ZERO);
        let b = spawn_boid(&mut world, Vec3::new(0.25, 0.0, 0.0), Vec3::ZERO);
        let params = BoidParams {
            separation: 5.0,
            cohesion: 0.0,
            alignment: 0.0,
            ..Default::default()
        };
        let before = dist(&world, a, b);
        flocking_system(&mut world, &params, 0.1);
        assert!(dist(&world, a, b) > before, "separation should increase spacing");
    }

    #[test]
    fn cohesion_pulls_spread_boids_together() {
        let mut world = World::new();
        // Within neighbour radius (3.0) but spread apart.
        let a = spawn_boid(&mut world, Vec3::new(-1.0, 0.0, 0.0), Vec3::ZERO);
        let b = spawn_boid(&mut world, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO);
        let params = BoidParams {
            separation: 0.0,
            cohesion: 5.0,
            alignment: 0.0,
            ..Default::default()
        };
        let before = dist(&world, a, b);
        flocking_system(&mut world, &params, 0.1);
        assert!(dist(&world, a, b) < before, "cohesion should reduce spacing");
    }

    #[test]
    fn lone_boid_coasts_at_constant_velocity() {
        let mut world = World::new();
        let e = spawn_boid(&mut world, Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0));
        flocking_system(&mut world, &BoidParams::default(), 0.5);
        // No neighbours → no steering force → velocity unchanged, position moved.
        assert_eq!(world.get::<Boid>(e).unwrap().velocity, Vec3::new(2.0, 0.0, 0.0));
        assert!((world.get::<Transform>(e).unwrap().position.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn velocity_is_clamped_to_max_speed() {
        let mut world = World::new();
        let a = spawn_boid(&mut world, Vec3::new(-0.1, 0.0, 0.0), Vec3::ZERO);
        let _b = spawn_boid(&mut world, Vec3::new(0.1, 0.0, 0.0), Vec3::ZERO);
        let params = BoidParams {
            separation: 1000.0, // huge force
            max_speed: 2.0,
            ..Default::default()
        };
        flocking_system(&mut world, &params, 0.5);
        let speed = world.get::<Boid>(a).unwrap().velocity.length();
        assert!(speed <= 2.0 + 1e-4, "speed {speed} exceeds max_speed");
    }

    #[test]
    fn empty_world_is_a_noop() {
        let mut world = World::new();
        flocking_system(&mut world, &BoidParams::default(), 0.1); // must not panic
    }

    /// The headline integration claim: gathering neighbours through a
    /// `SpatialGrid` yields exactly the same flocking force as an O(n²)
    /// brute-force scan — the grid only changes *how fast* you find neighbours,
    /// not *which* ones, so behaviour is identical.
    #[test]
    fn grid_gathered_neighbours_match_brute_force_separation() {
        use voltex_ecs::{SpatialGrid, Transform, World};

        const RADIUS: f32 = 3.0;
        let mut world = World::new();
        // A spread-out crowd, some inside RADIUS of the focal point, some not.
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),   // focal agent
            Vec3::new(1.0, 0.0, 0.0),   // in
            Vec3::new(-2.0, 0.0, 1.0),  // in
            Vec3::new(2.5, 0.0, -1.0),  // in (dist ~2.7)
            Vec3::new(10.0, 0.0, 0.0),  // out
            Vec3::new(0.0, 0.0, -20.0), // out
        ];
        let mut entities = Vec::new();
        for &p in &positions {
            let e = world.spawn();
            world.add(e, Transform::from_position(p));
            entities.push(e);
        }
        let focal = entities[0];
        let focal_pos = positions[0];

        // Brute force: every other agent within RADIUS.
        let brute: Vec<Vec3> = entities
            .iter()
            .copied()
            .filter(|&e| e != focal)
            .map(|e| world.get::<Transform>(e).unwrap().position)
            .filter(|p| (*p - focal_pos).length() <= RADIUS)
            .collect();

        // Grid-accelerated: same set, found via cell scan.
        let grid = SpatialGrid::from_transforms(&world, RADIUS);
        let mut grid_neighbours: Vec<Vec3> = grid
            .query_radius(focal_pos, RADIUS)
            .into_iter()
            .filter(|&e| e != focal)
            .map(|e| world.get::<Transform>(e).unwrap().position)
            .collect();

        // Same neighbour count, and identical separation force.
        assert_eq!(grid_neighbours.len(), brute.len(), "grid found a different neighbour count");
        let force_brute = separation(focal_pos, &brute);
        // Order can differ between the two gathering methods; separation is
        // order-independent (sum), so the forces must still match.
        let force_grid = separation(focal_pos, &grid_neighbours);
        assert!(approx(force_brute, force_grid), "{force_brute:?} vs {force_grid:?}");

        // Sanity: there really were both in- and out-of-range agents.
        assert_eq!(brute.len(), 3);
        grid_neighbours.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        assert!(grid_neighbours.iter().all(|p| (*p - focal_pos).length() <= RADIUS));
    }
}
