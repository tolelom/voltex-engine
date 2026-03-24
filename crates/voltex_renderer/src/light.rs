use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]],
            model: [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]],
            camera_pos: [0.0; 3],
            _padding: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 3],
    pub _padding1: f32,
    pub color: [f32; 3],
    pub ambient_strength: f32,
}

impl LightUniform {
    pub fn new() -> Self {
        Self {
            direction: [0.0, -1.0, -1.0],
            _padding1: 0.0,
            color: [1.0, 1.0, 1.0],
            ambient_strength: 0.1,
        }
    }
}
