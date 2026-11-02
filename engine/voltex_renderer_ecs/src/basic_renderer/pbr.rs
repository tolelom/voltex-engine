//! The opt-in PBR path of `BasicRenderer` (Cook-Torrance, multi-light,
//! 4-map materials, IBL BRDF LUT) for static and skinned meshes.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, create_pbr_skinned_pipeline,
    pbr_full_texture_bind_group_layout, BonePalette, CameraUniform, GpuTexture, IblResources,
    LightData, LightUniform, LightsUniform, MaterialUniform, ShadowMap, ShadowUniform,
};

use super::{BasicRenderer, MAX_SKINNED};
use crate::assets::RenderAssets;

pub(super) struct PbrResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) tex_layout: wgpu::BindGroupLayout,
    pub(super) camera_buffer: wgpu::Buffer,
    pub(super) camera_aligned: u32,
    pub(super) lights_buffer: wgpu::Buffer,
    pub(super) camera_light_bind_group: wgpu::BindGroup,
    pub(super) material_buffer: wgpu::Buffer,
    pub(super) material_aligned: u32,
    pub(super) material_bind_group: wgpu::BindGroup,
    pub(super) shadow_uniform_buffer: wgpu::Buffer,
    pub(super) shadow_bind_group: wgpu::BindGroup,
    // Default 1×1 maps for absent PBR slots (flat normal / white ORM / black emissive).
    pub(super) flat_normal: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
    pub(super) black: GpuTexture,
    pub(super) default_texture_bind_group: wgpu::BindGroup,
}

impl PbrResources {
    /// Also returns the material and shadow layouts, which the skinned PBR
    /// pipeline is built with.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        alignment: u32,
        max_entities: usize,
        tex_layout: &wgpu::BindGroupLayout,
        default_texture: &GpuTexture,
        shadow_map: &ShadowMap,
    ) -> (Self, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let pbr_cl_layout = pbr_camera_light_layout(device);
        let pbr_tex_layout = pbr_full_texture_bind_group_layout(device);
        let pbr_mat_layout = MaterialUniform::bind_group_layout(device);
        let pbr_shadow_layout = ShadowMap::bind_group_layout(device);
        let pbr_pipeline = create_pbr_pipeline(
            device,
            target_format,
            &pbr_cl_layout,
            &pbr_tex_layout,
            &pbr_mat_layout,
            &pbr_shadow_layout,
        );

        let pbr_camera_aligned =
            (std::mem::size_of::<CameraUniform>() as u32).div_ceil(alignment) * alignment;
        let pbr_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Camera Dynamic UBO"),
            size: (pbr_camera_aligned as usize * max_entities) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pbr_lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Lights UBO"),
            size: std::mem::size_of::<LightsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&pbr_lights_buffer, 0, bytemuck::bytes_of(&LightsUniform::new()));
        let pbr_camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer PBR Camera+Light BG"),
            layout: &pbr_cl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &pbr_camera_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pbr_lights_buffer.as_entire_binding(),
                },
            ],
        });

        let pbr_material_aligned =
            (std::mem::size_of::<MaterialUniform>() as u32).div_ceil(alignment) * alignment;
        let pbr_material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Material Dynamic UBO"),
            size: (pbr_material_aligned as usize * max_entities) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pbr_material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer PBR Material BG"),
            layout: &pbr_mat_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &pbr_material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
                }),
            }],
        });

        // IBL BRDF LUT + default PBR maps.
        let ibl = IblResources::new(device, queue);
        let pbr_flat_normal = GpuTexture::flat_normal_1x1(device, queue);
        let pbr_black = GpuTexture::black_1x1(device, queue, tex_layout);
        let pbr_default_texture_bind_group = create_pbr_full_texture_bind_group(
            device,
            &pbr_tex_layout,
            &default_texture.view, // white albedo
            &default_texture.sampler,
            &pbr_flat_normal.1,
            &pbr_flat_normal.2,
            &default_texture.view, // white ORM
            &default_texture.sampler,
            &pbr_black.view, // black emissive
            &pbr_black.sampler,
        );

        // PBR shadow group: depth map + comparison sampler + ShadowUniform + BRDF LUT.
        let pbr_shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Shadow UBO"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &pbr_shadow_uniform_buffer,
            0,
            bytemuck::bytes_of(&pbr_shadow_disabled()),
        );
        let pbr_shadow_bind_group = shadow_map.create_bind_group(
            device,
            &pbr_shadow_layout,
            &pbr_shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );
        let resources = Self {
            pipeline: pbr_pipeline,
            tex_layout: pbr_tex_layout,
            camera_buffer: pbr_camera_buffer,
            camera_aligned: pbr_camera_aligned,
            lights_buffer: pbr_lights_buffer,
            camera_light_bind_group: pbr_camera_light_bind_group,
            material_buffer: pbr_material_buffer,
            material_aligned: pbr_material_aligned,
            material_bind_group: pbr_material_bind_group,
            shadow_uniform_buffer: pbr_shadow_uniform_buffer,
            shadow_bind_group: pbr_shadow_bind_group,
            flat_normal: pbr_flat_normal,
            black: pbr_black,
            default_texture_bind_group: pbr_default_texture_bind_group,
        };
        (resources, pbr_mat_layout, pbr_shadow_layout)
    }
}

pub(super) struct PbrSkinnedResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) cam_buffer: wgpu::Buffer,
    pub(super) cam_aligned: u32,
    pub(super) g0_bind_group: wgpu::BindGroup,
    pub(super) material_buffer: wgpu::Buffer,
    pub(super) material_aligned: u32,
    pub(super) material_bind_group: wgpu::BindGroup,
}

impl PbrSkinnedResources {
    /// Group 0 binds the PBR lights buffer and the shared skinned bone palette.
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        alignment: u32,
        pbr: &PbrResources,
        pbr_mat_layout: &wgpu::BindGroupLayout,
        pbr_shadow_layout: &wgpu::BindGroupLayout,
        palette_buffer: &wgpu::Buffer,
    ) -> Self {
        let pbr_skinned_g0_layout = pbr_skinned_g0_layout(device);
        let pbr_skinned_pipeline = create_pbr_skinned_pipeline(
            device,
            target_format,
            &pbr_skinned_g0_layout,
            &pbr.tex_layout,
            pbr_mat_layout,
            pbr_shadow_layout,
        );
        let pbr_skinned_cam_aligned =
            (std::mem::size_of::<CameraUniform>() as u32).div_ceil(alignment) * alignment;
        let pbr_skinned_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Skinned Camera UBO"),
            size: (pbr_skinned_cam_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pbr_skinned_g0_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer PBR Skinned g0 BG"),
            layout: &pbr_skinned_g0_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &pbr_skinned_cam_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pbr.lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: palette_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<BonePalette>() as u64),
                    }),
                },
            ],
        });
        // Per-skinned-entity material (dynamic offset, MAX_SKINNED slots).
        let pbr_skinned_material_aligned =
            (std::mem::size_of::<MaterialUniform>() as u32).div_ceil(alignment) * alignment;
        let pbr_skinned_material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Skinned Material Dynamic UBO"),
            size: (pbr_skinned_material_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pbr_skinned_material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer PBR Skinned Material BG"),
            layout: pbr_mat_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &pbr_skinned_material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
                }),
            }],
        });
        Self {
            pipeline: pbr_skinned_pipeline,
            cam_buffer: pbr_skinned_cam_buffer,
            cam_aligned: pbr_skinned_cam_aligned,
            g0_bind_group: pbr_skinned_g0_bind_group,
            material_buffer: pbr_skinned_material_buffer,
            material_aligned: pbr_skinned_material_aligned,
            material_bind_group: pbr_skinned_material_bind_group,
        }
    }
}

impl BasicRenderer {
    /// The full PBR (Cook-Torrance + multi-light + material maps + IBL) mesh
    /// pass (clears the frame). Folds the directional `light` + the CPU copy of
    /// the point lights into a multi-light `LightsUniform`, stages per-entity
    /// `CameraUniform`/`MaterialUniform`, builds a 4-map texture bind group per
    /// entity (defaults for absent slots), and draws.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pbr_color_pass(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        items: &[crate::items::RenderItem],
        n: usize,
        view_proj: Mat4,
        cam_pos: Vec3,
        light: LightUniform,
        assets: &RenderAssets,
    ) {
        // ── Multi-light uniform: directional sun + folded point/spot lights ──
        let mut lights = LightsUniform::new();
        lights.add_light(LightData::directional(light.direction, light.color, 1.0));
        for pl in self.point_lights_cpu.borrow().iter() {
            if pl.spot_cos_outer < -1.5 {
                lights.add_light(LightData::point(pl.position, pl.color, pl.intensity, pl.range));
            } else {
                let outer_deg = pl.spot_cos_outer.clamp(-1.0, 1.0).acos().to_degrees();
                let inner_deg = (outer_deg - 5.0).max(0.0);
                lights.add_light(LightData::spot(
                    pl.position, pl.direction, pl.color, pl.intensity, pl.range, inner_deg,
                    outer_deg,
                ));
            }
        }
        lights.ambient_color = [
            light.color[0] * light.ambient_strength,
            light.color[1] * light.ambient_strength,
            light.color[2] * light.ambient_strength,
        ];
        queue.write_buffer(&self.pbr.lights_buffer, 0, bytemuck::bytes_of(&lights));

        // ── Per-entity camera + material staging ────────────────────────────
        let cam_aligned = self.pbr.camera_aligned as usize;
        let mat_aligned = self.pbr.material_aligned as usize;
        let mut cam_staging = vec![0u8; n * cam_aligned];
        let mut mat_staging = vec![0u8; n * mat_aligned];
        for (i, item) in items.iter().take(n).enumerate() {
            let cam = CameraUniform {
                view_proj: view_proj.cols,
                model: item.model.cols,
                camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                _padding: 0.0,
            };
            let cb = bytemuck::bytes_of(&cam);
            cam_staging[i * cam_aligned..i * cam_aligned + cb.len()].copy_from_slice(cb);
            let mb = bytemuck::bytes_of(&item.material);
            mat_staging[i * mat_aligned..i * mat_aligned + mb.len()].copy_from_slice(mb);
        }
        if n > 0 {
            queue.write_buffer(&self.pbr.camera_buffer, 0, &cam_staging);
            queue.write_buffer(&self.pbr.material_buffer, 0, &mat_staging);
        }

        // ── Build a 4-map texture bind group per entity (defaults per slot) ──
        // `None` ⇒ the entity has no custom maps → bind the shared default group
        // (avoids a per-frame allocation for the common untextured case).
        let resolve = |h: Option<crate::assets::TextureHandle>| {
            h.and_then(|h| assets.texture(h)).map(|t| (&t.view, &t.sampler))
        };
        let tex_bgs: Vec<Option<wgpu::BindGroup>> = items
            .iter()
            .take(n)
            .map(|item| {
                let has_maps = item.texture.is_some()
                    || item.pbr.map(|p| p.normal.or(p.orm).or(p.emissive).is_some())
                        == Some(true);
                if !has_maps {
                    return None;
                }
                let (av, asamp) = resolve(item.texture)
                    .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                let (nv, nsamp) = resolve(item.pbr.and_then(|p| p.normal))
                    .unwrap_or((&self.pbr.flat_normal.1, &self.pbr.flat_normal.2));
                let (ov, osamp) = resolve(item.pbr.and_then(|p| p.orm))
                    .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                let (ev, esamp) = resolve(item.pbr.and_then(|p| p.emissive))
                    .unwrap_or((&self.pbr.black.view, &self.pbr.black.sampler));
                Some(create_pbr_full_texture_bind_group(
                    &self.device,
                    &self.pbr.tex_layout,
                    av, asamp, nv, nsamp, ov, osamp, ev, esamp,
                ))
            })
            .collect();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer PBR Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
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
        pass.set_pipeline(&self.pbr.pipeline);
        pass.set_bind_group(3, &self.pbr.shadow_bind_group, &[]);
        for (i, item) in items.iter().take(n).enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
            pass.set_bind_group(0, &self.pbr.camera_light_bind_group, &[(i as u32) * self.pbr.camera_aligned]);
            let tex_bg = tex_bgs[i].as_ref().unwrap_or(&self.pbr.default_texture_bind_group);
            pass.set_bind_group(1, tex_bg, &[]);
            pass.set_bind_group(2, &self.pbr.material_bind_group, &[(i as u32) * self.pbr.material_aligned]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}

/// PBR group-0 layout: dynamic-offset `CameraUniform` (binding 0, vertex +
/// fragment) + a shared `LightsUniform` (binding 1, fragment). Mirrors the
/// layout `create_pbr_pipeline` expects.
fn pbr_camera_light_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer PBR Camera+Light BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<CameraUniform>() as u64,
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

/// Skinned-PBR group-0 layout: camera (binding 0, dynamic, vertex+fragment) +
/// lights (binding 1, fragment) + bone palette (binding 2, dynamic, vertex).
fn pbr_skinned_g0_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer PBR Skinned g0 BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<CameraUniform>() as u64,
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<BonePalette>() as u64,
                    ),
                },
                count: None,
            },
        ],
    })
}

/// A `ShadowUniform` with shadows off (`shadow_map_size = 0`) for the PBR pass.
fn pbr_shadow_disabled() -> ShadowUniform {
    ShadowUniform {
        light_view_proj: Mat4::IDENTITY.cols,
        shadow_map_size: 0.0,
        shadow_bias: 0.0,
        _padding: [0.0; 2],
        sun_direction: [0.5, -0.7, 0.5],
        turbidity: 3.0,
        sh_coefficients: [[0.0; 4]; 7],
    }
}
