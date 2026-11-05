//! Directional shadow mapping for `BasicRenderer`: the depth map, the depth
//! pre-pass resources and the group-3 uniform the Blinn-Phong shader samples.

use voltex_ecs::World;
use voltex_renderer::{
    create_shadow_pipeline, shadow_main_bind_group_layout, shadow_pass_bind_group_layout,
    LightUniform, ShadowMainUniform, ShadowMap, ShadowPassUniform, SHADOW_MAP_SIZE,
};

use super::{
    directional_light_vp, single_buffer_bind_group, BasicRenderer, SHADOW_BIAS,
};
use crate::assets::RenderAssets;
use crate::dynamic_uniform::DynamicUniformBuffer;
use crate::items::RenderItem;
use crate::skinned::SkinnedMeshHandle;

pub(super) struct ShadowResources {
    pub(super) map: ShadowMap,
    /// Depth-only pipeline that renders static meshes into `map`.
    pub(super) pipeline: wgpu::RenderPipeline,
    /// Per-entity `light_vp * model` for the depth pass.
    pub(super) pass_uniforms: DynamicUniformBuffer<ShadowPassUniform>,
    pub(super) pass_bind_group: wgpu::BindGroup,
    /// Group-3 layout the Blinn-Phong pipeline is built with (map + sampler +
    /// `ShadowMainUniform`).
    pub(super) main_layout: wgpu::BindGroupLayout,
    pub(super) main_buffer: wgpu::Buffer,
    pub(super) main_bind_group: wgpu::BindGroup,
}

impl ShadowResources {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        alignment: u32,
        max_entities: usize,
    ) -> Self {
        let map = ShadowMap::new(device);
        let main_layout = shadow_main_bind_group_layout(device);
        let main_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Shadow Main UBO"),
            size: std::mem::size_of::<ShadowMainUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &main_buffer,
            0,
            bytemuck::bytes_of(&ShadowMainUniform::disabled()),
        );
        let main_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Shadow Main BG"),
            layout: &main_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&map.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&map.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: main_buffer.as_entire_binding(),
                },
            ],
        });

        let pass_layout = shadow_pass_bind_group_layout(device);
        let pipeline = create_shadow_pipeline(device, &pass_layout);
        let pass_uniforms = DynamicUniformBuffer::<ShadowPassUniform>::new(
            device,
            "BasicRenderer Shadow Pass UBO",
            alignment,
            max_entities,
        );
        let pass_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer Shadow Pass BG",
            &pass_layout,
            pass_uniforms.binding(),
        );
        Self {
            map,
            pipeline,
            pass_uniforms,
            pass_bind_group,
            main_layout,
            main_buffer,
            main_bind_group,
        }
    }
}

impl BasicRenderer {
    /// Directional shadow depth pre-pass (static meshes, then skinned casters)
    /// and the group-3 uniform upload. With shadows off only the disabled
    /// uniform is written.
    pub(super) fn shadow_prepass(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        world: &World,
        assets: &RenderAssets,
        items: &[RenderItem],
        light: LightUniform,
    ) {
        if !self.shadows_enabled {
            queue.write_buffer(
                &self.shadow.main_buffer,
                0,
                bytemuck::bytes_of(&ShadowMainUniform::disabled()),
            );
            return;
        }
        let light_vp = directional_light_vp(light.direction);
        let pass_uniforms: Vec<ShadowPassUniform> = items
            .iter()
            .map(|item| ShadowPassUniform {
                light_vp_model: (light_vp * item.model).cols,
            })
            .collect();
        self.shadow.pass_uniforms.upload(queue, &pass_uniforms);
        // Skinned casters: upload light-space camera + shared palette, then
        // draw into the same depth map after the static meshes.
        let skinned_casters = self.gather_skinned_shadow(queue, world, light_vp);
        self.draw_shadow_depth(encoder, assets, items, &skinned_casters);

        let main_uniform = ShadowMainUniform {
            light_view_proj: light_vp.cols,
            shadow_map_size: SHADOW_MAP_SIZE as f32,
            shadow_bias: SHADOW_BIAS,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.shadow.main_buffer, 0, bytemuck::bytes_of(&main_uniform));
    }

    /// Clear the shadow map and render static meshes, then skinned casters,
    /// into it (uniforms already uploaded).
    fn draw_shadow_depth(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        assets: &RenderAssets,
        items: &[RenderItem],
        skinned_casters: &[SkinnedMeshHandle],
    ) {
        let mut depth_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Shadow Depth Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow.map.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        depth_pass.set_pipeline(&self.shadow.pipeline);
        for (i, item) in items.iter().enumerate() {
            let mesh = assets.mesh(item.mesh).expect("collect skipped stale");
            depth_pass.set_bind_group(
                0,
                &self.shadow.pass_bind_group,
                &[self.shadow.pass_uniforms.offset(i)],
            );
            depth_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            depth_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            depth_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
        if skinned_casters.is_empty() {
            return;
        }
        let skinned = &self.skinned;
        depth_pass.set_pipeline(&skinned.shadow_pipeline);
        for (i, handle) in skinned_casters.iter().enumerate() {
            let Some(mesh) = assets.skinned_mesh(*handle) else {
                continue;
            };
            depth_pass.set_bind_group(
                0,
                &skinned.shadow_camera_bind_group,
                &[skinned.shadow_cameras.offset(i)],
            );
            depth_pass.set_bind_group(1, &skinned.palette_bind_group, &[skinned.palettes.offset(i)]);
            depth_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            depth_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            depth_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}
