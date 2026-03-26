use voltex_math::Vec3;

/// ECS component for AI pathfinding navigation.
#[derive(Debug, Clone)]
pub struct NavAgent {
    pub target: Option<Vec3>,
    pub speed: f32,
    pub path: Vec<Vec3>,
    pub current_waypoint: usize,
    pub reached: bool,
}

impl NavAgent {
    pub fn new(speed: f32) -> Self {
        NavAgent {
            target: None,
            speed,
            path: Vec::new(),
            current_waypoint: 0,
            reached: false,
        }
    }

    pub fn set_target(&mut self, target: Vec3) {
        self.target = Some(target);
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
    }

    pub fn clear_target(&mut self) {
        self.target = None;
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
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
