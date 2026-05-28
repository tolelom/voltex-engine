//! Reynolds-style steering behaviours (seek/flee/arrive/wander/follow_path).
//! Each free function returns a desired steering force; integration is left to
//! the caller.

use voltex_math::Vec3;

/// An agent that can be steered through the world.
#[derive(Debug, Clone)]
pub struct SteeringAgent {
    /// World position.
    pub position: Vec3,
    /// Current velocity (world units / second).
    pub velocity: Vec3,
    /// Velocity magnitude cap.
    pub max_speed: f32,
    /// Steering-force magnitude cap (applied each behaviour call).
    pub max_force: f32,
}

impl SteeringAgent {
    /// Creates an agent at `position` with zero velocity and the given caps.
    pub fn new(position: Vec3, max_speed: f32, max_force: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            max_speed,
            max_force,
        }
    }
}

/// Truncate vector `v` to at most `max_len` magnitude.
pub(crate) fn truncate(v: Vec3, max_len: f32) -> Vec3 {
    let len_sq = v.length_squared();
    if len_sq > max_len * max_len {
        v.normalize() * max_len
    } else {
        v
    }
}

/// Seek steering behavior: move toward `target` at max speed.
/// Returns the steering force to apply.
pub fn seek(agent: &SteeringAgent, target: Vec3) -> Vec3 {
    let to_target = target - agent.position;
    let len = to_target.length();
    if len < f32::EPSILON {
        return Vec3::ZERO;
    }
    let desired = to_target.normalize() * agent.max_speed;
    let steering = desired - agent.velocity;
    truncate(steering, agent.max_force)
}

/// Flee steering behavior: move away from `threat` at max speed.
/// Returns the steering force to apply.
pub fn flee(agent: &SteeringAgent, threat: Vec3) -> Vec3 {
    let away = agent.position - threat;
    let len = away.length();
    if len < f32::EPSILON {
        return Vec3::ZERO;
    }
    let desired = away.normalize() * agent.max_speed;
    let steering = desired - agent.velocity;
    truncate(steering, agent.max_force)
}

/// Arrive steering behavior: decelerate when within `slow_radius` of `target`.
/// Returns the steering force to apply.
pub fn arrive(agent: &SteeringAgent, target: Vec3, slow_radius: f32) -> Vec3 {
    let to_target = target - agent.position;
    let dist = to_target.length();
    if dist < f32::EPSILON {
        return Vec3::ZERO;
    }

    let desired_speed = if dist < slow_radius {
        agent.max_speed * (dist / slow_radius)
    } else {
        agent.max_speed
    };

    let desired = to_target.normalize() * desired_speed;
    let steering = desired - agent.velocity;
    truncate(steering, agent.max_force)
}

/// Wander steering behavior: steer toward a point on a circle projected ahead of the agent.
/// `wander_radius` is the radius of the wander circle,
/// `wander_distance` is how far ahead the circle is projected,
/// `angle` is the current angle on the wander circle (in radians).
/// Returns the steering force to apply.
pub fn wander(agent: &SteeringAgent, wander_radius: f32, wander_distance: f32, angle: f32) -> Vec3 {
    // Compute forward direction; use +Z if velocity is near zero
    let forward = {
        let len = agent.velocity.length();
        if len > f32::EPSILON {
            agent.velocity.normalize()
        } else {
            Vec3::Z
        }
    };

    // Circle center is ahead of the agent
    let circle_center = agent.position + forward * wander_distance;

    // Point on the circle at the given angle (in XZ plane)
    let wander_target = Vec3::new(
        circle_center.x + angle.cos() * wander_radius,
        circle_center.y,
        circle_center.z + angle.sin() * wander_radius,
    );

    seek(agent, wander_target)
}

/// Follow path steering behavior.
/// Seeks toward the current waypoint; advances to the next when within `waypoint_radius`.
/// Uses `arrive` for the last waypoint.
///
/// Returns (steering_force, updated_current_waypoint_index).
pub fn follow_path(
    agent: &SteeringAgent,
    path: &[Vec3],
    current_waypoint: usize,
    waypoint_radius: f32,
) -> (Vec3, usize) {
    if path.is_empty() {
        return (Vec3::ZERO, 0);
    }

    let clamped = current_waypoint.min(path.len() - 1);
    let waypoint = path[clamped];
    let dist = (waypoint - agent.position).length();

    // If we are close enough and there is a next waypoint, advance
    if dist < waypoint_radius && clamped + 1 < path.len() {
        let next = clamped + 1;
        let force = if next == path.len() - 1 {
            arrive(agent, path[next], waypoint_radius)
        } else {
            seek(agent, path[next])
        };
        return (force, next);
    }

    // Use arrive for the last waypoint, seek for all others
    let force = if clamped == path.len() - 1 {
        arrive(agent, waypoint, waypoint_radius)
    } else {
        seek(agent, waypoint)
    };

    (force, clamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_at(x: f32, z: f32) -> SteeringAgent {
        SteeringAgent::new(Vec3::new(x, 0.0, z), 5.0, 10.0)
    }

    #[test]
    fn test_seek_direction() {
        let agent = agent_at(0.0, 0.0);
        let target = Vec3::new(1.0, 0.0, 0.0);
        let force = seek(&agent, target);
        // Force should be in the +X direction
        assert!(force.x > 0.0, "seek force x should be positive");
        assert!(force.z.abs() < 1e-4, "seek force z should be ~0");
    }

    #[test]
    fn test_flee_direction() {
        let agent = agent_at(0.0, 0.0);
        let threat = Vec3::new(1.0, 0.0, 0.0);
        let force = flee(&agent, threat);
        // Force should be in the -X direction (away from threat)
        assert!(force.x < 0.0, "flee force x should be negative");
        assert!(force.z.abs() < 1e-4, "flee force z should be ~0");
    }

    #[test]
    fn test_arrive_deceleration() {
        let mut agent = agent_at(0.0, 0.0);
        // Give agent some velocity toward target
        agent.velocity = Vec3::new(5.0, 0.0, 0.0);
        let target = Vec3::new(2.0, 0.0, 0.0); // within slow_radius=5
        let slow_radius = 5.0;
        let force = arrive(&agent, target, slow_radius);
        // The desired speed is reduced (dist/slow_radius * max_speed = 2/5 * 5 = 2)
        // desired velocity = (1,0,0)*2, current velocity = (5,0,0)
        // steering = (2-5, 0, 0) = (-3, 0, 0) — a braking force
        assert!(force.x < 0.0, "arrive should produce braking force when inside slow_radius");
    }

    #[test]
    fn test_arrive_at_target() {
        let agent = agent_at(0.0, 0.0);
        let target = Vec3::new(0.0, 0.0, 0.0); // same position
        let force = arrive(&agent, target, 1.0);
        // At target, force should be zero
        assert!(force.length() < 1e-4, "force at target should be zero");
    }

    #[test]
    fn test_follow_path_advance() {
        // Path: (0,0,0) -> (5,0,0) -> (10,0,0)
        let path = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        // Agent is very close to waypoint 1
        let agent = SteeringAgent::new(Vec3::new(4.9, 0.0, 0.0), 5.0, 10.0);
        let waypoint_radius = 0.5;
        let (_, next_wp) = follow_path(&agent, &path, 1, waypoint_radius);
        // Should advance to waypoint 2
        assert_eq!(next_wp, 2, "should advance to waypoint 2 when close to waypoint 1");
    }

    #[test]
    fn test_follow_path_last_arrives() {
        // Path: single-segment, agent near last waypoint
        let path = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        let agent = SteeringAgent::new(Vec3::new(9.0, 0.0, 0.0), 5.0, 10.0);
        let waypoint_radius = 2.0;
        let (force, wp_idx) = follow_path(&agent, &path, 1, waypoint_radius);
        // Still at waypoint 1 (the last one); force should be arrive (decelerating)
        assert_eq!(wp_idx, 1);
        // arrive should produce a deceleration toward (10,0,0)
        // agent has zero velocity, dist=1, slow_radius=2 → desired_speed=1/2*5=2.5 in +X
        // steering = desired - velocity = (2.5,0,0) - (0,0,0) = (2.5,0,0)
        assert!(force.x > 0.0, "arrive force should be toward last waypoint");
    }
}
