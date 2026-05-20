//! GizmoRenderer — wgpu LINE pipeline + procedural meshes. Task 5.

use super::{GizmoAxis, GizmoMode};
use voltex_math::{Mat4, Vec3};

pub struct GizmoRenderer;

impl GizmoRenderer {
    pub fn new(
        _device: &wgpu::Device,
        _target_format: wgpu::TextureFormat,
        _depth_format: wgpu::TextureFormat,
    ) -> Self {
        Self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _target_view: &wgpu::TextureView,
        _depth_view: &wgpu::TextureView,
        _view_proj: Mat4,
        _origin: Vec3,
        _mode: GizmoMode,
        _active_axis: Option<GizmoAxis>,
    ) {
        // implemented in Task 5
    }
}
