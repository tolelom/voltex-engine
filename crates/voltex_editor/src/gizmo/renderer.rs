//! GizmoRenderer — LINE-list wgpu pipeline + procedural gizmo meshes.
//! Depth test is forced Always so the gizmo always draws on top.

use bytemuck::{Pod, Zeroable};
use std::mem;
use voltex_math::{Mat4, Vec3};

use super::{GizmoAxis, GizmoMode};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct LineVertex {
    position: [f32; 3],
    _pad: f32,
    color: [f32; 3],
    _pad2: f32,
}

impl LineVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<LineVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 16,
                shader_location: 1,
            },
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

const MAX_VERTS: usize = 2048;

/// wgpu pipeline that draws the translate/rotate/scale gizmo handles.
pub struct GizmoRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
}

impl GizmoRenderer {
    /// Creates the gizmo pipeline targeting the given color + depth formats.
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gizmo Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Gizmo BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(mem::size_of::<CameraUniform>() as u64),
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Gizmo PL"),
            bind_group_layouts: &[&bind_layout],
            immediate_size: 0,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gizmo Camera UBO"),
            size: mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gizmo BG"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gizmo VB"),
            size: (MAX_VERTS * mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gizmo Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::LAYOUT],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline, camera_buffer, bind_group, vertex_buffer }
    }

    /// Renders the gizmo handles into `target_view` for the given state.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view_proj: Mat4,
        origin: Vec3,
        mode: GizmoMode,
        active_axis: Option<GizmoAxis>,
    ) {
        let mut verts: Vec<LineVertex> = Vec::with_capacity(64);
        match mode {
            GizmoMode::Translate => build_translate(&mut verts, origin, active_axis),
            GizmoMode::Rotate    => build_rotate(&mut verts, origin, active_axis),
            GizmoMode::Scale     => build_scale(&mut verts, origin, active_axis),
        }
        if verts.is_empty() { return; }
        let n = verts.len().min(MAX_VERTS);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts[..n]));

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[CameraUniform { view_proj: view_proj.cols }]),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Gizmo Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..n as u32, 0..1);
    }
}

// ── Mesh builders ───────────────────────────────────────────────────────────

const ACTIVE_COLOR: [f32; 3] = [1.0, 1.0, 0.0];
const AXIS_LENGTH: f32 = 1.5;
const AXIS_BASE: f32 = 0.15;
const HEAD_SIZE: f32 = 0.15;
const RING_RADIUS: f32 = 1.2;
const RING_SEGMENTS: usize = 32;
const SCALE_HANDLE: f32 = 0.12;

fn axis_color(axis: GizmoAxis, active: Option<GizmoAxis>) -> [f32; 3] {
    if active == Some(axis) { return ACTIVE_COLOR; }
    match axis {
        GizmoAxis::X => [1.0, 0.2, 0.2],
        GizmoAxis::Y => [0.2, 1.0, 0.2],
        GizmoAxis::Z => [0.2, 0.4, 1.0],
    }
}

fn push_line(verts: &mut Vec<LineVertex>, a: Vec3, b: Vec3, color: [f32; 3]) {
    verts.push(LineVertex { position: [a.x, a.y, a.z], _pad: 0.0, color, _pad2: 0.0 });
    verts.push(LineVertex { position: [b.x, b.y, b.z], _pad: 0.0, color, _pad2: 0.0 });
}

fn build_translate(verts: &mut Vec<LineVertex>, o: Vec3, active: Option<GizmoAxis>) {
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let color = axis_color(axis, active);
        let dir = axis.dir();
        let start = o + dir * AXIS_BASE;
        let end = o + dir * AXIS_LENGTH;
        push_line(verts, start, end, color);
        let (u, v) = ortho_basis(dir);
        let tip = end;
        let back = end - dir * HEAD_SIZE;
        push_line(verts, tip, back + u * HEAD_SIZE * 0.5, color);
        push_line(verts, tip, back - u * HEAD_SIZE * 0.5, color);
        push_line(verts, tip, back + v * HEAD_SIZE * 0.5, color);
        push_line(verts, tip, back - v * HEAD_SIZE * 0.5, color);
    }
}

fn build_rotate(verts: &mut Vec<LineVertex>, o: Vec3, active: Option<GizmoAxis>) {
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let color = axis_color(axis, active);
        let (u, v) = ortho_basis(axis.dir());
        let mut prev = o + u * RING_RADIUS;
        for i in 1..=RING_SEGMENTS {
            let theta = (i as f32) * std::f32::consts::TAU / (RING_SEGMENTS as f32);
            let p = o + u * (RING_RADIUS * theta.cos()) + v * (RING_RADIUS * theta.sin());
            push_line(verts, prev, p, color);
            prev = p;
        }
    }
}

fn build_scale(verts: &mut Vec<LineVertex>, o: Vec3, active: Option<GizmoAxis>) {
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let color = axis_color(axis, active);
        let dir = axis.dir();
        let start = o + dir * AXIS_BASE;
        let end = o + dir * AXIS_LENGTH;
        push_line(verts, start, end, color);
        let (u, v) = ortho_basis(dir);
        let c = end;
        let p000 = c - u * SCALE_HANDLE - v * SCALE_HANDLE - dir * SCALE_HANDLE;
        let p100 = c + u * SCALE_HANDLE - v * SCALE_HANDLE - dir * SCALE_HANDLE;
        let p110 = c + u * SCALE_HANDLE + v * SCALE_HANDLE - dir * SCALE_HANDLE;
        let p010 = c - u * SCALE_HANDLE + v * SCALE_HANDLE - dir * SCALE_HANDLE;
        let p001 = c - u * SCALE_HANDLE - v * SCALE_HANDLE + dir * SCALE_HANDLE;
        let p101 = c + u * SCALE_HANDLE - v * SCALE_HANDLE + dir * SCALE_HANDLE;
        let p111 = c + u * SCALE_HANDLE + v * SCALE_HANDLE + dir * SCALE_HANDLE;
        let p011 = c - u * SCALE_HANDLE + v * SCALE_HANDLE + dir * SCALE_HANDLE;
        push_line(verts, p000, p100, color);
        push_line(verts, p100, p110, color);
        push_line(verts, p110, p010, color);
        push_line(verts, p010, p000, color);
        push_line(verts, p001, p101, color);
        push_line(verts, p101, p111, color);
        push_line(verts, p111, p011, color);
        push_line(verts, p011, p001, color);
        push_line(verts, p000, p001, color);
        push_line(verts, p100, p101, color);
        push_line(verts, p110, p111, color);
        push_line(verts, p010, p011, color);
    }
}

fn ortho_basis(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.y.abs() < 0.9 { Vec3::new(0.0, 1.0, 0.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u).normalize();
    (u, v)
}
