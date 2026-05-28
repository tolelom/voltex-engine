use voltex_math::Vec3;

/// A dynamic obstacle represented as a position and radius.
#[derive(Debug, Clone)]
pub(crate) struct DynamicObstacle {
    pub(crate) position: Vec3,
    pub(crate) radius: f32,
}

/// Compute avoidance steering force using velocity obstacle approach.
///
/// Projects the agent's velocity forward by `look_ahead` distance and checks
/// for circle intersections with obstacles. Returns a steering force perpendicular
/// to the approach direction to avoid the nearest threatening obstacle.
pub(crate) fn avoid_obstacles(
    agent_pos: Vec3,
    agent_vel: Vec3,
    agent_radius: f32,
    obstacles: &[DynamicObstacle],
    look_ahead: f32,
) -> Vec3 {
    let speed = agent_vel.length();
    if speed < f32::EPSILON {
        return Vec3::ZERO;
    }

    let forward = agent_vel * (1.0 / speed);
    let mut nearest_t = f32::INFINITY;
    let mut avoidance = Vec3::ZERO;

    for obs in obstacles {
        let to_obs = obs.position - agent_pos;
        let combined_radius = agent_radius + obs.radius;

        // Project obstacle center onto the velocity ray
        let proj = to_obs.dot(forward);

        // Obstacle is behind or too far ahead
        if proj < 0.0 || proj > look_ahead {
            continue;
        }

        // Lateral distance from the velocity ray to obstacle center (XZ only for ground agents)
        let closest_on_ray = agent_pos + forward * proj;
        let diff = obs.position - closest_on_ray;
        let lateral_dist_sq = diff.x * diff.x + diff.z * diff.z;
        let combined_sq = combined_radius * combined_radius;

        if lateral_dist_sq >= combined_sq {
            continue; // No collision
        }

        // This obstacle threatens the agent — check if it's the nearest
        if proj < nearest_t {
            nearest_t = proj;

            // Avoidance direction: perpendicular to approach, away from obstacle
            // Use XZ plane lateral vector
            let lateral = Vec3::new(diff.x, 0.0, diff.z);
            let lat_len = lateral.length();
            if lat_len > f32::EPSILON {
                // Steer away from obstacle (opposite direction of lateral offset)
                let steer_dir = lateral * (-1.0 / lat_len);
                // Strength inversely proportional to distance (closer = stronger)
                let strength = 1.0 - (proj / look_ahead);
                avoidance = steer_dir * strength * speed;
            } else {
                // Agent heading straight at obstacle center — pick perpendicular
                // Use cross product with Y to get a lateral direction
                let perp = Vec3::new(-forward.z, 0.0, forward.x);
                avoidance = perp * speed;
            }
        }
    }

    avoidance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_obstacle_zero_force() {
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.5,
            &[],
            5.0,
        );
        assert!(force.length() < 1e-6, "no obstacles should give zero force");
    }

    #[test]
    fn test_obstacle_behind_zero_force() {
        let obs = DynamicObstacle {
            position: Vec3::new(-3.0, 0.0, 0.0),
            radius: 1.0,
        };
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.5,
            &[obs],
            5.0,
        );
        assert!(force.length() < 1e-6, "obstacle behind should give zero force");
    }

    #[test]
    fn test_obstacle_ahead_lateral_force() {
        let obs = DynamicObstacle {
            position: Vec3::new(3.0, 0.0, 0.5), // slightly to the right
            radius: 1.0,
        };
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0), // moving in +X
            0.5,
            &[obs],
            5.0,
        );
        assert!(force.length() > 0.1, "obstacle ahead should give non-zero force");
        // Force should push away from obstacle (obstacle is at +Z, force should be -Z)
        assert!(force.z < 0.0, "force should push away from obstacle (negative Z)");
    }

    #[test]
    fn test_obstacle_far_away_zero_force() {
        let obs = DynamicObstacle {
            position: Vec3::new(3.0, 0.0, 10.0), // far to the side
            radius: 1.0,
        };
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.5,
            &[obs],
            5.0,
        );
        assert!(force.length() < 1e-6, "distant obstacle should give zero force");
    }

    #[test]
    fn test_obstacle_beyond_lookahead_zero_force() {
        let obs = DynamicObstacle {
            position: Vec3::new(10.0, 0.0, 0.0),
            radius: 1.0,
        };
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.5,
            &[obs],
            5.0, // look_ahead is only 5
        );
        assert!(force.length() < 1e-6, "obstacle beyond lookahead should give zero force");
    }

    #[test]
    fn test_zero_velocity_zero_force() {
        let obs = DynamicObstacle {
            position: Vec3::new(3.0, 0.0, 0.0),
            radius: 1.0,
        };
        let force = avoid_obstacles(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
            0.5,
            &[obs],
            5.0,
        );
        assert!(force.length() < 1e-6, "zero velocity should give zero force");
    }
}
