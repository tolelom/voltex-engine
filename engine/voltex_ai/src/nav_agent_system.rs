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

/// Smallest waypoint acceptance radius, so a stationary or very slow agent
/// can still reach a waypoint.
const MIN_WAYPOINT_RADIUS: f32 = 0.05;
/// Steering force limit as a multiple of the agent's speed.
const MAX_FORCE_PER_SPEED: f32 = 8.0;
/// Steering forces with a squared length below this count as zero.
const MIN_FORCE_LENGTH_SQ: f32 = 1e-12;

/// Advance all [`NavAgent`] entities by `dt` seconds over `navmesh`.
///
/// No-op when `dt <= 0.0`.
pub fn nav_agent_system(world: &mut World, navmesh: &NavMesh, dt: f32) {
    if dt <= 0.0 {
        return;
    }
    replan_paths(world, navmesh);
    advance_agents(world, dt);
}

/// Pass 1: (re)compute a path for agents whose target is new or has changed.
/// `planned_target` records what `path` was computed for, so a direct
/// `agent.target = Some(..)` assignment is picked up like `set_target`.
fn replan_paths(world: &mut World, navmesh: &NavMesh) {
    let need_path: Vec<(Entity, Vec3, Vec3, bool)> = world
        .query2::<NavAgent, Transform>()
        .filter(|(_, agent, _)| {
            agent.target.is_some()
                && (agent.target != agent.planned_target || (agent.path.is_empty() && !agent.reached))
        })
        .map(|(entity, agent, transform)| {
            let goal = agent.target.expect("target is_some checked above");
            (entity, transform.position, goal, !agent.path.is_empty())
        })
        .collect();
    for (entity, start, goal, has_path) in need_path {
        // Retarget fast path: the goal moved but stayed in the same navmesh
        // triangle as the current path's endpoint — reuse the path and just
        // move its endpoint instead of running A* again.
        if has_path {
            let endpoint = world
                .get::<NavAgent>(entity)
                .and_then(|agent| agent.path.last().copied())
                .expect("has_path implies non-empty path");
            if let (Some(goal_tri), Some(end_tri)) =
                (navmesh.find_triangle(goal), navmesh.find_triangle(endpoint))
            {
                if goal_tri == end_tri {
                    if let Some(agent) = world.get_mut::<NavAgent>(entity) {
                        *agent.path.last_mut().expect("non-empty path") = goal;
                        agent.reached = false;
                        agent.planned_target = agent.target;
                    }
                    continue;
                }
            }
        }
        match find_path(navmesh, start, goal) {
            Some(path) => {
                if let Some(agent) = world.get_mut::<NavAgent>(entity) {
                    agent.path = path;
                    agent.current_waypoint = 0;
                    agent.reached = false;
                    agent.planned_target = agent.target;
                }
            }
            None => {
                // Goal is unreachable / off-mesh: drop the target so the agent
                // goes idle instead of recomputing a failing path every frame.
                if let Some(agent) = world.get_mut::<NavAgent>(entity) {
                    agent.clear_target();
                }
            }
        }
    }
}

/// Pass 2: steer active agents toward their current waypoint and integrate.
fn advance_agents(world: &mut World, dt: f32) {
    struct Step {
        entity: Entity,
        position: Vec3,
        waypoint: usize,
        reached: bool,
    }
    let steps: Vec<Step> = world
        .query2::<NavAgent, Transform>()
        .filter(|(_, agent, _)| !agent.path.is_empty() && !agent.reached)
        .map(|(entity, agent, transform)| {
            // A waypoint is "reached" within one frame of travel.
            let radius = (agent.speed * dt).max(MIN_WAYPOINT_RADIUS);
            let steering = SteeringAgent::new(transform.position, agent.speed, agent.speed * MAX_FORCE_PER_SPEED);
            let (force, waypoint) = follow_path(&steering, &agent.path, agent.current_waypoint, radius);
            let dir = if force.length_squared() > MIN_FORCE_LENGTH_SQ {
                force.normalize()
            } else {
                Vec3::ZERO
            };
            let candidate = transform.position + dir * agent.speed * dt;
            let goal = *agent.path.last().expect("path non-empty checked above");
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
    for step in steps {
        if let Some(transform) = world.get_mut::<Transform>(step.entity) {
            transform.position = step.position;
        }
        if let Some(agent) = world.get_mut::<NavAgent>(step.entity) {
            agent.current_waypoint = step.waypoint;
            if step.reached {
                agent.reached = true;
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
    fn direct_target_assignment_repaths_after_reached() {
        // Assigning `agent.target = Some(..)` directly (without set_target)
        // must be picked up by the system — the historical footgun left the
        // agent permanently parked on its old goal.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(10.0);
        agent.set_target(Vec3::new(0.0, 0.0, 20.0));
        world.add(e, agent);

        let mesh = flat_navmesh();
        for _ in 0..1000 {
            nav_agent_system(&mut world, &mesh, 0.1);
            if world.get::<NavAgent>(e).unwrap().reached {
                break;
            }
        }
        assert!(world.get::<NavAgent>(e).unwrap().reached, "precondition: first goal reached");

        // Direct field assignment — no set_target.
        world.get_mut::<NavAgent>(e).unwrap().target = Some(Vec3::new(20.0, 0.0, -20.0));

        let mut reached_new = false;
        for _ in 0..1000 {
            nav_agent_system(&mut world, &mesh, 0.1);
            let a = world.get::<NavAgent>(e).unwrap();
            if a.reached {
                reached_new = true;
                break;
            }
        }
        assert!(reached_new, "agent must chase a directly-assigned target");
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!(
            (pos - Vec3::new(20.0, 0.0, -20.0)).length() < 1.0,
            "agent should end near the new target, got {pos:?}"
        );
    }

    #[test]
    fn direct_target_assignment_redirects_mid_path() {
        // Changing the target while the agent is still walking must redirect it.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(5.0);
        agent.set_target(Vec3::new(0.0, 0.0, 30.0));
        world.add(e, agent);

        let mesh = flat_navmesh();
        for _ in 0..5 {
            nav_agent_system(&mut world, &mesh, 0.1);
        }
        world.get_mut::<NavAgent>(e).unwrap().target = Some(Vec3::new(0.0, 0.0, -10.0));

        let mut reached = false;
        for _ in 0..1000 {
            nav_agent_system(&mut world, &mesh, 0.1);
            if world.get::<NavAgent>(e).unwrap().reached {
                reached = true;
                break;
            }
        }
        assert!(reached, "agent must reach the redirected target");
        let pos = world.get::<Transform>(e).unwrap().position;
        assert!(
            (pos - Vec3::new(0.0, 0.0, -10.0)).length() < 1.0,
            "agent should end near the redirected target, got {pos:?}"
        );
    }

    #[test]
    fn small_target_move_reuses_path() {
        // When the new target lies in the same navmesh triangle as the current
        // path's endpoint, the system retargets by moving the endpoint instead
        // of recomputing the whole path — path[0] keeps the original start.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        let mut agent = NavAgent::new(5.0);
        agent.set_target(Vec3::new(0.0, 0.0, 30.0));
        world.add(e, agent);

        let mesh = flat_navmesh();
        for _ in 0..5 {
            nav_agent_system(&mut world, &mesh, 0.1);
        }
        let original_first = world.get::<NavAgent>(e).unwrap().path[0];

        // Small move within the same (single) triangle.
        world.get_mut::<NavAgent>(e).unwrap().target = Some(Vec3::new(1.0, 0.0, 29.0));
        nav_agent_system(&mut world, &mesh, 0.1);

        let a = world.get::<NavAgent>(e).unwrap();
        assert!(
            (*a.path.first().unwrap() - original_first).length() < 1e-6,
            "retarget must reuse the existing path (full repath rewrites path[0])"
        );
        assert!(
            (*a.path.last().unwrap() - Vec3::new(1.0, 0.0, 29.0)).length() < 1e-6,
            "retarget must move the path endpoint to the new target"
        );
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
