//! Continuous collision detection — swept-sphere-vs-AABB plus the
//! `physics_step` pass that stops fast bodies from tunnelling.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Vec3, AABB, Ray};
use crate::collider::Collider;
use crate::ray_shapes::ray_vs_aabb;
use crate::rigid_body::RigidBody;

/// Shorter sweeps than this are treated as "not moving".
const MIN_SWEEP_LENGTH: f32 = 1e-10;

/// Swept sphere vs AABB continuous collision detection.
/// Expands the AABB by the sphere radius, then tests a ray from start to end.
/// Returns the sweep fraction in `[0, 1]` (0 = `start`, 1 = `end`) of first
/// contact, or `None` if no contact.
pub fn swept_sphere_vs_aabb(start: Vec3, end: Vec3, radius: f32, aabb: &AABB) -> Option<f32> {
    // Expand AABB by sphere radius
    let radius_vec = Vec3::new(radius, radius, radius);
    let expanded = AABB::new(aabb.min - radius_vec, aabb.max + radius_vec);

    let direction = end - start;
    let sweep_len = direction.length();

    if sweep_len < MIN_SWEEP_LENGTH {
        // No movement — check if already inside
        if expanded.contains_point(start) {
            return Some(0.0);
        }
        return None;
    }

    let ray = Ray::new(start, direction * (1.0 / sweep_len));

    match ray_vs_aabb(&ray, &expanded) {
        Some(distance) => {
            let fraction = distance / sweep_len;
            if fraction <= 1.0 {
                Some(fraction)
            } else {
                None
            }
        }
        None => None,
    }
}

/// Positions of every awake dynamic body with a collider, taken before
/// integration so [`clamp_tunnelling`] can sweep the step's motion.
pub(crate) fn snapshot_positions(world: &World) -> Vec<(Entity, Vec3)> {
    world
        .query3::<Transform, RigidBody, Collider>()
        .filter(|(_, _, rb, _)| !rb.is_static() && !rb.is_sleeping)
        .map(|(entity, transform, _, _)| (entity, transform.position))
        .collect()
}

/// How far a body may move in one step before it can skip past a collider.
fn tunnelling_radius(collider: &Collider) -> f32 {
    match collider {
        Collider::Sphere { radius } | Collider::Capsule { radius, .. } => *radius,
        Collider::Box { half_extents } => half_extents.x.min(half_extents.y).min(half_extents.z),
        Collider::ConvexHull(hull) => hull.vertices.iter().map(|v| v.length()).fold(f32::MAX, f32::min),
    }
}

/// Distance the body is placed past first contact so the discrete
/// narrowphase that follows sees the contact and resolves the velocity.
const CONTACT_SKIN: f32 = 1e-3;

/// For each body that moved further than its own radius this step, sweeps a
/// sphere from its pre-integration position and pulls it back to the first
/// *new* contact. Colliders it already overlapped at the start (fraction 0,
/// resting contact) are left to the discrete solver.
pub(crate) fn clamp_tunnelling(world: &mut World, positions_before_integration: &[(Entity, Vec3)]) {
    let colliders: Vec<(Entity, AABB)> = world
        .query2::<Transform, Collider>()
        .map(|(entity, transform, collider)| (entity, collider.aabb(transform.position)))
        .collect();
    let mut corrections = Vec::new();
    for &(entity, start) in positions_before_integration {
        let (Some(transform), Some(collider)) = (world.get::<Transform>(entity), world.get::<Collider>(entity)) else {
            continue;
        };
        let end = transform.position;
        let radius = tunnelling_radius(collider);
        let travel = (end - start).length();
        if travel <= radius {
            continue;
        }
        let first_hit = colliders
            .iter()
            .filter(|(other, _)| *other != entity)
            .filter_map(|(_, aabb)| swept_sphere_vs_aabb(start, end, radius, aabb))
            .filter(|&fraction| fraction > 0.0)
            .fold(f32::INFINITY, f32::min);
        if first_hit <= 1.0 {
            let fraction = (first_hit + CONTACT_SKIN / travel).min(1.0);
            corrections.push((entity, start + (end - start) * fraction));
        }
    }
    for (entity, position) in corrections {
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.position = position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn test_swept_sphere_hits_aabb() {
        let start = Vec3::new(-10.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        // Expanded AABB min.x = 3.5, start.x = -10, direction = 20
        // t = (3.5 - (-10)) / 20 = 13.5 / 20 = 0.675
        assert!(t > 0.0 && t < 1.0);
        assert!(approx(t, 0.675));
    }

    #[test]
    fn test_swept_sphere_misses_aabb() {
        let start = Vec3::new(-10.0, 10.0, 0.0);
        let end = Vec3::new(10.0, 10.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        assert!(swept_sphere_vs_aabb(start, end, radius, &aabb).is_none());
    }

    #[test]
    fn test_swept_sphere_starts_inside() {
        let start = Vec3::new(5.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    #[test]
    fn test_swept_sphere_tunneling_detection() {
        // Fast sphere that would tunnel through a thin wall
        let start = Vec3::new(-100.0, 0.0, 0.0);
        let end = Vec3::new(100.0, 0.0, 0.0);
        let radius = 0.1;
        // Thin wall at x=0
        let aabb = AABB::new(Vec3::new(-0.05, -10.0, -10.0), Vec3::new(0.05, 10.0, 10.0));

        let t = swept_sphere_vs_aabb(start, end, radius, &aabb);
        assert!(t.is_some(), "should detect tunneling through thin wall");
        let t = t.unwrap();
        // Expanded AABB min.x = -0.05 - radius(0.1) = -0.15.
        // start.x = -100, sweep direction = +200.
        // First contact at parametric t = (-0.15 - (-100)) / 200 = 99.85 / 200 = 0.499250.
        assert!(approx(t, 0.499_25), "expected TOI ~0.49925, got {t}");
    }

    #[test]
    fn test_swept_sphere_no_movement() {
        let start = Vec3::new(5.0, 0.0, 0.0);
        let end = Vec3::new(5.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        // Inside expanded AABB, so should return 0
        let t = swept_sphere_vs_aabb(start, end, radius, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    #[test]
    fn test_swept_sphere_beyond_range() {
        let start = Vec3::new(-10.0, 0.0, 0.0);
        let end = Vec3::new(-5.0, 0.0, 0.0);
        let radius = 0.5;
        let aabb = AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0));

        // AABB is at x=4..6, moving from -10 to -5 won't reach it
        assert!(swept_sphere_vs_aabb(start, end, radius, &aabb).is_none());
    }
}
