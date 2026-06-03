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

use voltex_math::Vec3;

/// Steer away from nearby `neighbours`, weighted by inverse distance so closer
/// crowders push harder. Returns a unit direction away from the crowd, or zero
/// if there are no (non-coincident) neighbours.
pub fn separation(position: Vec3, neighbours: &[Vec3]) -> Vec3 {
    let mut acc = Vec3::ZERO;
    for &n in neighbours {
        let away = position - n;
        let len = away.length();
        if len > 1e-6 {
            // away/len is the unit direction; the extra 1/len weights by 1/dist.
            acc = acc + away * (1.0 / (len * len));
        }
    }
    acc.normalize_or_zero()
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
