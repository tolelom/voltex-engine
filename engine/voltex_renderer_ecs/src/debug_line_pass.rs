//! `BasicRenderer`'s debug line pass: immediate-mode line segments set once per
//! frame via `set_debug_lines`, drawn last as a depth-tested `LineList`.

use std::cell::Cell;

use voltex_math::Mat4;
use voltex_renderer::{
    create_line_pipeline, line_camera_bind_group_layout, LineCameraUniform, LineVertex,
};

/// Max debug-line vertices uploaded per frame (2 per segment). Fixed-capacity
/// so `set_debug_lines` can stay `&self` (mirrors `set_point_lights`).
pub(crate) const MAX_DEBUG_LINE_VERTICES: usize = 8192;

pub(crate) struct DebugLinePass {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_count: Cell<u32>,
}

impl DebugLinePass {
    pub(crate) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let camera_layout = line_camera_bind_group_layout(device);
        let pipeline = create_line_pipeline(device, target_format, &camera_layout);
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Debug Line Camera UBO"),
            size: std::mem::size_of::<LineCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Debug Line Camera BG"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Debug Line Vertex Buffer"),
            size: (MAX_DEBUG_LINE_VERTICES * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            vertex_buffer,
            vertex_count: Cell::new(0),
        }
    }

    /// Upload the endpoints for the next frame (2 per segment); excess beyond
    /// [`MAX_DEBUG_LINE_VERTICES`] is dropped, an empty slice clears.
    pub(crate) fn set_lines(&self, queue: &wgpu::Queue, verts: &[LineVertex]) {
        let vertex_count = verts.len().min(MAX_DEBUG_LINE_VERTICES) & !1; // even count (pairs)
        if vertex_count > 0 {
            queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&verts[..vertex_count]),
            );
        }
        self.vertex_count.set(vertex_count as u32);
    }

    /// Draw the queued segments (`LineList`, depth-tested against the scene, no
    /// depth write). No-op when no lines are queued.
    pub(crate) fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view_proj: Mat4,
    ) {
        let count = self.vertex_count.get();
        if count == 0 {
            return;
        }
        let cam = LineCameraUniform {
            view_proj: view_proj.cols,
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&cam));

        let mut pass = crate::scene_pass::begin_overlay_pass(
            encoder,
            "BasicRenderer Debug Line Pass",
            target_view,
            depth_view,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..count, 0..1);
    }
}
