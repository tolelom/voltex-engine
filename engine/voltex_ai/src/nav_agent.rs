//! `NavAgent` ECS component: target + computed waypoint path + progress state.

use voltex_math::Vec3;

/// ECS component for AI pathfinding navigation.
#[derive(Debug, Clone)]
pub struct NavAgent {
    /// Goal position; `None` while the agent is idle.
    pub target: Option<Vec3>,
    /// Movement speed in world units per second.
    pub speed: f32,
    /// Sequence of waypoints from the agent's current position to `target`.
    pub path: Vec<Vec3>,
    /// Index into `path` of the waypoint the agent is currently heading to.
    pub current_waypoint: usize,
    /// `true` once the agent has reached the final waypoint.
    pub reached: bool,
    /// The target `path` was planned for — owned by `nav_agent_system`, which
    /// repaths whenever `target` no longer matches (so direct `target`
    /// assignment works just like [`NavAgent::set_target`]).
    pub(crate) planned_target: Option<Vec3>,
}

impl NavAgent {
    /// Creates an idle agent with the given movement `speed`.
    pub fn new(speed: f32) -> Self {
        NavAgent {
            target: None,
            speed,
            path: Vec::new(),
            current_waypoint: 0,
            reached: false,
            planned_target: None,
        }
    }

    /// Sets a new target and resets the cached path / waypoint / reached flag.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = Some(target);
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
        self.planned_target = None;
    }

    /// Clears the target, cached path, and progress state — agent is idle again.
    pub fn clear_target(&mut self) {
        self.target = None;
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
        self.planned_target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let agent = NavAgent::new(5.0);
        assert!((agent.speed - 5.0).abs() < 1e-6);
        assert!(agent.target.is_none());
        assert!(agent.path.is_empty());
        assert_eq!(agent.current_waypoint, 0);
        assert!(!agent.reached);
    }

    #[test]
    fn test_set_target() {
        let mut agent = NavAgent::new(3.0);
        agent.set_target(Vec3::new(10.0, 0.0, 5.0));
        assert!(agent.target.is_some());
        let t = agent.target.unwrap();
        assert!((t.x - 10.0).abs() < 1e-6);
        assert!(!agent.reached);
    }

    #[test]
    fn test_set_target_clears_path() {
        let mut agent = NavAgent::new(3.0);
        agent.path = vec![Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
        agent.current_waypoint = 1;
        agent.set_target(Vec3::new(5.0, 0.0, 5.0));
        assert!(agent.path.is_empty());
        assert_eq!(agent.current_waypoint, 0);
    }

    #[test]
    fn test_clear_target() {
        let mut agent = NavAgent::new(3.0);
        agent.set_target(Vec3::new(10.0, 0.0, 5.0));
        agent.path = vec![Vec3::new(1.0, 0.0, 0.0)];
        agent.reached = true;
        agent.clear_target();
        assert!(agent.target.is_none());
        assert!(agent.path.is_empty());
        assert_eq!(agent.current_waypoint, 0);
        assert!(!agent.reached);
    }
}
