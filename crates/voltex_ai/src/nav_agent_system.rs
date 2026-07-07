//! `nav_agent_system`: ECS system that drives [`NavAgent`] entities along
//! NavMesh paths.
//!
//! For every entity carrying a [`NavAgent`] + [`Transform`], the system:
//! 1. computes a path with [`find_path`] when the agent has a `target` but no
//!    cached path,
//! 2. steers toward the current waypoint via [`follow_path`] and integrates the
//!    agent's [`Transform`] position by `speed * dt`,
//! 3. marks the agent `reached` (and snaps to the goal) once it is within one
//!    step of the final waypoint.
//!
//! The [`NavMesh`] is owned by the caller and passed in each frame, mirroring
//! how `physics_step` takes a `PhysicsConfig`. Agents that move are kinematic
//! (position-integrated); coupling with `RigidBody` velocity is left to the
//! caller / a future cycle.

use crate::{find_path, follow_path, NavAgent, NavMesh, SteeringAgent};
use voltex_ecs::{Entity, Transform, World};
use voltex_math::Vec3;

/// Advance all [`NavAgent`] entities by `dt` seconds over `navmesh`.
///
/// No-op when `dt <= 0.0`.
pub fn nav_agent_system(world: &mut World, navmesh: &NavMesh, dt: f32) {
    if dt <= 0.0 {
        return;
    }

    // Pass 1: (re)compute a path for agents that have a target but no cached path.
    let need_path: Vec<(Entity, Vec3, Vec3)> = world
        .query2::<NavAgent, Transform>()
        .filter(|(_, a, _)| a.target.is_some() && a.path.is_empty() && !a.reached)
        .map(|(e, a, t)| (e, t.position, a.target.expect("target is_some checked above")))
        .collect();
    for (entity, start, goal) in need_path {
        match find_path(navmesh, start, goal) {
            Some(path) => {
                if let Some(a) = world.get_mut::<NavAgent>(entity) {
                    a.path = path;
                    a.current_waypoint = 0;
                }
            }
            None => {
                // Goal is unreachable / off-mesh: drop the target so the agent
                // goes idle instead of recomputing a failing path every frame.
                if let Some(a) = world.get_mut::<NavAgent>(entity) {
                    a.clear_target();
                }
            }
        }
    }

    // Pass 2: steer active agents toward their current waypoint and integrate.
    struct Step {
        entity: Entity,
        position: Vec3,
        waypoint: usize,
        reached: bool,
    }
    let steps: Vec<Step> = world
        .query2::<NavAgent, Transform>()
        .filter(|(_, a, _)| !a.path.is_empty() && !a.reached)
        .map(|(entity, a, t)| {
            // A waypoint is "reached" within one frame of travel (clamped so a
            // stationary / very slow agent still has a non-zero acceptance radius).
            let radius = (a.speed * dt).max(0.05);
            let steer = SteeringAgent::new(t.position, a.speed, a.speed * 8.0);
            let (force, waypoint) = follow_path(&steer, &a.path, a.current_waypoint, radius);
            let dir = if force.length_squared() > 1e-12 {
                force.normalize()
            } else {
                Vec3::ZERO
            };
            let candidate = t.position + dir * a.speed * dt;
            let goal = *a.path.last().expect("path non-empty checked above");
            // Arrival is judged in XZ (navmesh navigation is planar) and the
            // snap keeps the agent's gameplay y — waypoints may sit on the
            // navmesh plane at a different height.
            let goal_on_plane = Vec3::new(goal.x, candidate.y, goal.z);
            if (goal_on_plane - candidate).length() <= radius {
                // Snap onto the goal so repeated runs are idempotent.
                Step { entity, position: goal_on_plane, waypoint, reached: true }
            } else {
                Step { entity, position: candidate, waypoint, reached: false }
            }
        })
        .collect();
    for s in steps {
        if let Some(t) = world.get_mut::<Transform>(s.entity) {
            t.position = s.position;
        }
        if let Some(a) = world.get_mut::<NavAgent>(s.entity) {
            a.current_waypoint = s.waypoint;
            if s.reached {
                a.reached = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navmesh::{NavMesh, NavTriangle};

    /// One large triangle on the XZ plane covering the test area. `find_path`
    /// returns `[start, goal]` for any two interior points (single triangle).
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

    #[test]
    fn agent_moves_toward_target_and_reaches() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(10.0);
        agent.set_target(Vec3::new(0.0, 0.0, 20.0));
        world.add(e, agent);

        let mut reached = false;
        for _ in 0..1000 {
            nav_agent_system(&mut world, &flat_navmesh(), 0.1);
            if world.get::<NavAgent>(e).unwrap().reached {
                reached = true;
                break;
            }
        }
        assert!(reached, "agent should eventually reach its target");
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!(
            (pos - Vec3::new(0.0, 0.0, 20.0)).length() < 1.0,
            "agent should end near target, got {pos:?}"
        );

        // Idempotent after arrival: a reached agent does not drift.
        let before = world.get::<Transform>(e).unwrap().position;
        nav_agent_system(&mut world, &flat_navmesh(), 0.1);
        let after = world.get::<Transform>(e).unwrap().position;
        assert!((before - after).length() < 1e-6, "reached agent must not move");
    }

    #[test]
    fn idle_agent_does_not_move() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 0.0, 2.0)));
        world.add(e, NavAgent::new(5.0)); // no target
        nav_agent_system(&mut world, &flat_navmesh(), 0.1);
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!((pos - Vec3::new(1.0, 0.0, 2.0)).length() < 1e-6);
    }

    #[test]
    fn agent_progresses_in_target_direction() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(5.0);
        agent.set_target(Vec3::new(0.0, 0.0, 30.0));
        world.add(e, agent);
        nav_agent_system(&mut world, &flat_navmesh(), 0.1);
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!(pos.z > 0.0, "agent should move in +Z toward target, got {pos:?}");
    }

    #[test]
    fn unreachable_target_goes_idle() {
        let mut world = World::new();
        let e = world.spawn();
        // Start off-mesh so find_path returns None.
        world.add(e, Transform::from_position(Vec3::new(1000.0, 0.0, 1000.0)));
        let mut agent = NavAgent::new(5.0);
        agent.set_target(Vec3::new(0.0, 0.0, 0.0));
        world.add(e, agent);
        nav_agent_system(&mut world, &flat_navmesh(), 0.1);
        let a = world.get::<NavAgent>(e).unwrap();
        assert!(a.target.is_none(), "unreachable target should be cleared");
        assert!(a.path.is_empty());
    }

    #[test]
    fn agent_keeps_gameplay_y_while_following() {
        // Gameplay places the agent above the navmesh plane (y=0.5); path
        // following must be planar — reach the target in XZ without sinking
        // the agent onto the navmesh plane. speed*dt = 0.2 < y offset 0.5 so
        // a 3D acceptance radius could never trigger.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
        let mut agent = NavAgent::new(2.0);
        agent.set_target(Vec3::new(0.0, 0.0, 20.0));
        world.add(e, agent);

        let mesh = flat_navmesh();
        let mut reached = false;
        for _ in 0..2000 {
            nav_agent_system(&mut world, &mesh, 0.1);
            if world.get::<NavAgent>(e).unwrap().reached {
                reached = true;
                break;
            }
        }
        assert!(reached, "agent must reach an on-mesh target despite its y offset");
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!((pos.y - 0.5).abs() < 1e-3, "gameplay y must be preserved, got {pos:?}");
        let goal_xz = Vec3::new(0.0, pos.y, 20.0);
        assert!((pos - goal_xz).length() < 1.0, "agent should end near target in XZ, got {pos:?}");
    }

    #[test]
    fn zero_dt_is_noop() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(5.0);
        agent.set_target(Vec3::new(0.0, 0.0, 10.0));
        world.add(e, agent);
        nav_agent_system(&mut world, &flat_navmesh(), 0.0);
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!((pos - Vec3::ZERO).length() < 1e-6, "dt=0 must not move the agent");
    }
}
