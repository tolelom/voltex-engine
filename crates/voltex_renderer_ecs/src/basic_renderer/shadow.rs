//! Directional shadow mapping for `BasicRenderer`: the depth map, the depth
//! pre-pass resources and the group-3 uniform the colored shader samples.

use voltex_ecs::World;
use voltex_renderer::{
    create_shadow_pipeline, shadow_main_bind_group_layout, shadow_pass_bind_group_layout,
    LightUniform, ShadowMainUniform, ShadowMap, ShadowPassUniform, SHADOW_MAP_SIZE,
};

use super::{directional_light_vp, BasicRenderer};
use crate::assets::RenderAssets;
use crate::items::RenderItem;

pub(super) struct ShadowResources {
    pub(super) map: ShadowMap,
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) pass_buffer: wgpu::Buffer,
    pub(super) pass_bind_group: wgpu::BindGroup,
    pub(super) pass_aligned: u32,
    pub(super) main_buffer: wgpu::Buffer,
    pub(super) main_bind_group: wgpu::BindGroup,
}

impl ShadowResources {
    /// Also returns the group-3 layout, which the colored pipeline is built with.
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        alignment: u32,
        max_entities: usize,
    ) -> (Self, wgpu::BindGroupLayout) {
        let shadow_map = ShadowMap::new(device);
        let shadow_main_layout = shadow_main_bind_group_layout(device);
        let shadow_main_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Shadow Main UBO"),
            size: std::mem::size_of::<ShadowMainUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &shadow_main_buffer,
            0,
            bytemuck::bytes_of(&ShadowMainUniform::disabled()),
        );
        let shadow_main_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Shadow Main BG"),
            layout: &shadow_main_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_map.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_map.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: shadow_main_buffer.as_entire_binding(),
                },
            ],
        });

        // Shadow depth pass (per-entity light_vp_model dynamic UBO).
        let shadow_pass_layout = shadow_pass_bind_group_layout(device);
        let shadow_pipeline = create_shadow_pipeline(device, &shadow_pass_layout);
        let shadow_pass_aligned =
            (std::mem::size_of::<ShadowPassUniform>() as u32).div_ceil(alignment) * alignment;
        let shadow_pass_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Shadow Pass UBO"),
            size: (shadow_pass_aligned as usize * max_entities) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_pass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Shadow Pass BG"),
            layout: &shadow_pass_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_pass_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ShadowPassUniform>() as u64),
                }),
            }],
        });
        let resources = Self {
            map: shadow_map,
            pipeline: shadow_pipeline,
            pass_buffer: shadow_pass_buffer,
            pass_bind_group: shadow_pass_bind_group,
            pass_aligned: shadow_pass_aligned,
            main_buffer: shadow_main_buffer,
            main_bind_group: shadow_main_bind_group,
        };
        (resources, shadow_main_layout)
    }
}

impl BasicRenderer {
    /// Directional shadow depth pre-pass (static meshes, then skinned casters)
    /// and the group-3 uniform upload. With shadows off only the disabled
    /// uniform is written.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn shadow_prepass(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        world: &World,
        assets: &RenderAssets,
        items: &[RenderItem],
        n: usize,
        light: LightUniform,
    ) {
        if self.shadows_enabled {
            let light_vp = directional_light_vp(light.direction);
            let sp_aligned = self.shadow.pass_aligned as usize;
            let mut sp_staging = vec![0u8; n * sp_aligned];
            for (i, item) in items.iter().take(n).enumerate() {
                let u = ShadowPassUniform {
                    light_vp_model: (light_vp * item.model).cols,
                };
                let o = i * sp_aligned;
                sp_staging[o..o + std::mem::size_of::<ShadowPassUniform>()]
                    .copy_from_slice(bytemuck::bytes_of(&u));
            }
            if n > 0 {
                queue.write_buffer(&self.shadow.pass_buffer, 0, &sp_staging);
            }
            // Skinned casters: upload light-space camera + shared palette, then
            // draw into the same depth map after the static meshes.
            let skinned_casters = self.gather_skinned_shadow(queue, world, light_vp);
            {
                let mut dpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                dpass.set_pipeline(&self.shadow.pipeline);
                for (i, item) in items.iter().take(n).enumerate() {
                    let mesh = assets.mesh(item.mesh).expect("collect skipped stale");
                    let off = (i as u32) * self.shadow.pass_aligned;
                    dpass.set_bind_group(0, &self.shadow.pass_bind_group, &[off]);
                    dpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    dpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    dpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                }
                if !skinned_casters.is_empty() {
                    dpass.set_pipeline(&self.skinned.shadow_pipeline);
                    for (i, handle) in skinned_casters.iter().enumerate() {
                        let Some(mesh) = assets.skinned_mesh(*handle) else {
                            continue;
                        };
                        let cam_off = (i as u32) * self.skinned.cam_aligned;
                        let pal_off = (i as u32) * self.skinned.palette_aligned;
                        dpass.set_bind_group(0, &self.skinned.shadow_cam_bind_group, &[cam_off]);
                        dpass.set_bind_group(1, &self.skinned.palette_bind_group, &[pal_off]);
                        dpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        dpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        dpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                    }
                }
            }
            let su = ShadowMainUniform {
                light_view_proj: light_vp.cols,
                shadow_map_size: SHADOW_MAP_SIZE as f32,
                shadow_bias: 0.003,
                _pad: [0.0; 2],
            };
            queue.write_buffer(&self.shadow.main_buffer, 0, bytemuck::bytes_of(&su));
        } else {
            queue.write_buffer(
                &self.shadow.main_buffer,
                0,
                bytemuck::bytes_of(&ShadowMainUniform::disabled()),
            );
        }
    }
}
