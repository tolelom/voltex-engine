use voltex::prelude::{Vec3, Mat4};

/// Fixed quarter-view (isometric-like) camera for the survivor game.
pub struct QuarterViewCamera {
    /// Offset from the target (camera position = target + offset).
    pub offset: Vec3,
    /// The point the camera is looking at (player position later).
    pub target: Vec3,
}

impl QuarterViewCamera {
    pub fn new() -> Self {
        Self {
            offset: Vec3::new(0.0, 15.0, 10.0),
            target: Vec3::ZERO,
        }
    }

    /// Compute the view matrix for the current camera state.
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.target + self.offset;
        Mat4::look_at(eye, self.target, Vec3::Y)
    }

    /// Compute the perspective projection matrix.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(45.0_f32.to_radians(), aspect, 0.1, 100.0)
    }

    /// Compute combined view-projection matrix.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    /// Get the eye position in world space.
    pub fn eye_position(&self) -> Vec3 {
        self.target + self.offset
    }
}
