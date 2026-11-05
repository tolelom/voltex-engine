//! The default mesh path of `BasicRenderer`: Blinn-Phong (`mesh_colored.wgsl`)
//! with a per-entity `base_color` tint, one directional light, additive point
//! lights and the directional shadow map.
//!
//! Bind groups: 0 = camera (dynamic) + light, 1 = albedo texture,
//! 2 = point lights, 3 = shadow map (`ShadowResources`).

use voltex_renderer::{
    pipeline::create_colored_mesh_pipeline, ColoredCameraUniform, LightUniform,
    PointLightsUniform,
};

use super::{single_buffer_bind_group, BasicRenderer};
use crate::assets::RenderAssets;
use crate::dynamic_uniform::DynamicUniformBuffer;
use crate::frame::{FrameTarget, FrameView};
use crate::items::RenderItem;
use crate::scene_pass::begin_clearing_pass;

pub(super) struct BlinnPhongResources {
    pipeline: wgpu::RenderPipeline,
    camera_uniforms: DynamicUniformBuffer<ColoredCameraUniform>,
    light_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    pub(super) point_lights_buffer: wgpu::Buffer,
    point_lights_bind_group: wgpu::BindGroup,
}

impl BlinnPhongResources {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        alignment: u32,
        max_entities: usize,
        texture_layout: &wgpu::BindGroupLayout,
        shadow_main_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let camera_light_layout = camera_light_bind_group_layout(device);
        let camera_uniforms = DynamicUniformBuffer::<ColoredCameraUniform>::new(
            device,
            "BasicRenderer Camera Dynamic UBO",
            alignment,
            max_entities,
        );
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Light UBO"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Camera+Light BG"),
            layout: &camera_light_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_uniforms.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // Point lights: single fragment-visible uniform, no dynamic offset.
        let point_lights_layout = point_lights_bind_group_layout(device);
        let point_lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Point Lights UBO"),
            size: std::mem::size_of::<PointLightsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &point_lights_buffer,
            0,
            bytemuck::bytes_of(&PointLightsUniform::new()),
        );
        let point_lights_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer Point Lights BG",
            &point_lights_layout,
            point_lights_buffer.as_entire_binding(),
        );

        let pipeline = create_colored_mesh_pipeline(
            device,
            target_format,
            &camera_light_layout,
            texture_layout,
            &point_lights_layout,
            shadow_main_layout,
        );
        Self {
            pipeline,
            camera_uniforms,
            light_buffer,
            camera_light_bind_group,
            point_lights_buffer,
            point_lights_bind_group,
        }
    }
}

impl BasicRenderer {
    /// The default Blinn-Phong mesh pass (clears the frame). `render_frame` picks it
    /// or [`Self::pbr_mesh_pass`].
    pub(super) fn blinn_phong_mesh_pass(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget<'_>,
        view: &FrameView,
        items: &[RenderItem],
        assets: &RenderAssets,
    ) {
        let FrameView { view_proj, camera_pos, light } = *view;
        let res = &self.blinn_phong;
        let cameras: Vec<ColoredCameraUniform> = items
            .iter()
            .map(|item| ColoredCameraUniform {
                view_proj: view_proj.cols,
                model: item.model.cols,
                camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                _padding: 0.0,
                base_color: item.material.base_color,
            })
            .collect();
        res.camera_uniforms.upload(queue, &cameras);
        queue.write_buffer(&res.light_buffer, 0, bytemuck::cast_slice(&[light]));

        let mut pass = begin_clearing_pass(
            encoder,
            "BasicRenderer Pass",
            target.color_view,
            target.depth_view,
            target.clear_color,
        );
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(2, &res.point_lights_bind_group, &[]);
        pass.set_bind_group(3, &self.shadow.main_bind_group, &[]);
        for (i, item) in items.iter().enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
            let albedo_bind_group = item
                .texture
                .and_then(|h| assets.texture(h))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture.bind_group);
            pass.set_bind_group(1, albedo_bind_group, &[]);
            pass.set_bind_group(0, &res.camera_light_bind_group, &[res.camera_uniforms.offset(i)]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}

/// Group 0: dynamic-offset `ColoredCameraUniform` (binding 0) + `LightUniform`
/// (binding 1).
fn camera_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer Camera+Light BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<ColoredCameraUniform>() as u64,
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// Group 2: a single `PointLightsUniform`, fragment-visible.
fn point_lights_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer Point Lights BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(
                    std::mem::size_of::<PointLightsUniform>() as u64,
                ),
            },
            count: None,
        }],
    })
}
