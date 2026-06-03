//! BasicRenderer — mesh_shader.wgsl 기반 ECS-driven 렌더러.
//!
//! pipeline + per-entity dynamic UBO + light buffer + 1×1 default texture
//! 를 모두 owning. `render(...)` 한 메서드로 World 를 그린다.

use voltex_ecs::{World, WorldTransform};
use voltex_math::{Mat4, Vec3, Vec4, AABB};
use voltex_renderer::{
    create_shadow_pipeline,
    pipeline::{create_colored_mesh_pipeline, create_skinned_mesh_pipeline},
    shadow_main_bind_group_layout, shadow_pass_bind_group_layout, BonePalette, ColoredCameraUniform,
    GpuTexture, LightUniform, PointLight, PointLightsUniform, ShadowMainUniform, ShadowMap,
    ShadowPassUniform, SkinnedCameraUniform, SHADOW_MAP_SIZE,
};

use crate::assets::RenderAssets;
use crate::items::collect_render_items;
use crate::skinned::{SkinnedMeshHandle, SkinnedPose};

/// Max skinned entities drawn per frame. Each needs an 8 KB bone-palette slot,
/// so this is kept small + independent of `max_entities` to bound VRAM.
const MAX_SKINNED: usize = 64;

/// World-driven mesh renderer — Blinn-Phong shader, per-entity dynamic UBO,
/// camera + light shared bind group. Owns the wgpu pipeline objects.
pub struct BasicRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    default_texture: GpuTexture,
    aligned_size: u32,
    max_entities: usize,
    // Additive point lights (group 2 of the colored mesh shader).
    point_lights_buffer: wgpu::Buffer,
    point_lights_bind_group: wgpu::BindGroup,
    // Skinned pass: GPU vertex skinning for entities with SkinnedMeshHandle.
    skinned_pipeline: wgpu::RenderPipeline,
    skinned_cam_buffer: wgpu::Buffer,
    palette_buffer: wgpu::Buffer,
    skinned_cam_bind_group: wgpu::BindGroup,
    palette_bind_group: wgpu::BindGroup,
    skinned_cam_aligned: u32,
    palette_aligned: u32,
    // Directional shadow mapping (off by default; enable with set_shadows).
    shadows_enabled: bool,
    shadow_map: ShadowMap,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_pass_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    shadow_pass_aligned: u32,
    shadow_main_buffer: wgpu::Buffer,
    shadow_main_bind_group: wgpu::BindGroup,
}

impl BasicRenderer {
    /// `target_format` 은 render target 의 컬러 포맷 (offscreen viewport 또는 swapchain).
    /// `max_entities` 는 한 프레임에 처리 가능한 최대 entity 수 (UBO 크기 사전 할당).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        max_entities: usize,
    ) -> Self {
        let alignment = device.limits().min_uniform_buffer_offset_alignment;
        let cam_size = std::mem::size_of::<ColoredCameraUniform>() as u32;
        let aligned_size = cam_size.div_ceil(alignment) * alignment;

        let cl_layout = camera_light_bind_group_layout(device);
        let tex_layout = GpuTexture::bind_group_layout(device);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Camera Dynamic UBO"),
            size: (aligned_size as usize * max_entities) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Light UBO"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Camera+Light BG"),
            layout: &cl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &camera_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(cam_size as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        let default_texture = GpuTexture::white_1x1(device, queue, &tex_layout);

        // Point lights (group 2): single fragment-visible uniform, no dynamic offset.
        let pl_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        });
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
        let point_lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Point Lights BG"),
            layout: &pl_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: point_lights_buffer.as_entire_binding(),
            }],
        });

        // ── Shadow mapping resources (group 3 of the colored shader) ────────
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

        let pipeline = create_colored_mesh_pipeline(
            device,
            target_format,
            &cl_layout,
            &tex_layout,
            &pl_layout,
            &shadow_main_layout,
        );

        // ── Skinned pass resources ──────────────────────────────────────────
        let skinned_cam_aligned =
            (std::mem::size_of::<SkinnedCameraUniform>() as u32).div_ceil(alignment) * alignment;
        let palette_aligned =
            (std::mem::size_of::<BonePalette>() as u32).div_ceil(alignment) * alignment;

        let skinned_cam_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Skinned Camera BGL",
            std::mem::size_of::<SkinnedCameraUniform>() as u64,
        );
        let palette_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Bone Palette BGL",
            std::mem::size_of::<BonePalette>() as u64,
        );
        let skinned_pipeline = create_skinned_mesh_pipeline(
            device,
            target_format,
            &skinned_cam_layout,
            &palette_layout,
        );
        let skinned_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Skinned Camera Dynamic UBO"),
            size: (skinned_cam_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Bone Palette Dynamic UBO"),
            size: (palette_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skinned_cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Skinned Camera BG"),
            layout: &skinned_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &skinned_cam_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<SkinnedCameraUniform>() as u64),
                }),
            }],
        });
        let palette_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Bone Palette BG"),
            layout: &palette_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &palette_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<BonePalette>() as u64),
                }),
            }],
        });

        Self {
            pipeline,
            camera_buffer,
            light_buffer,
            camera_light_bind_group,
            default_texture,
            aligned_size,
            max_entities,
            point_lights_buffer,
            point_lights_bind_group,
            skinned_pipeline,
            skinned_cam_buffer,
            palette_buffer,
            skinned_cam_bind_group,
            palette_bind_group,
            skinned_cam_aligned,
            palette_aligned,
            shadows_enabled: false,
            shadow_map,
            shadow_pipeline,
            shadow_pass_buffer,
            shadow_pass_bind_group,
            shadow_pass_aligned,
            shadow_main_buffer,
            shadow_main_bind_group,
        }
    }

    /// Enable/disable directional shadow mapping for the non-skinned pass.
    /// Off by default. When off, the shadow uniform is disabled (`size 0`) so
    /// the shader returns full light — behaviour identical to no shadows.
    pub fn set_shadows(&mut self, enabled: bool) {
        self.shadows_enabled = enabled;
    }

    /// Set the additive point lights used by the non-skinned mesh pass. Excess
    /// lights beyond [`MAX_POINT_LIGHTS`](voltex_renderer::MAX_POINT_LIGHTS) are
    /// dropped. Call once per frame (cheap; just a uniform write).
    pub fn set_point_lights(&self, queue: &wgpu::Queue, lights: &[PointLight]) {
        let uniform = PointLightsUniform::from_slice(lights);
        queue.write_buffer(&self.point_lights_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// World 의 모든 MeshHandle 엔티티를 dynamic-offset draw 로 렌더링한다.
    /// 호출자는 encoder + target_view + depth_view 만 넘긴다 (내부에서 begin_render_pass).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        world: &mut World,
        assets: &RenderAssets,
        view_proj: Mat4,
        cam_pos: Vec3,
        light: LightUniform,
    ) {
        let mut items = collect_render_items(world, assets);
        // Frustum culling: drop entities whose world bounding sphere is fully
        // outside the camera frustum. Entities without `Bounds` are always
        // kept (can't cull conservatively).
        let frustum = voltex_renderer::extract_frustum(&view_proj);
        items.retain(|it| match world.get::<voltex_ecs::Bounds>(it.entity) {
            Some(b) => {
                let (center, radius) = world_bounding_sphere(it.model, b.0);
                voltex_renderer::sphere_vs_frustum(center, radius, &frustum)
            }
            None => true,
        });
        let n = items.len().min(self.max_entities);

        // ── Directional shadow depth pre-pass ───────────────────────────────
        if self.shadows_enabled {
            let light_vp = directional_light_vp(light.direction);
            let sp_aligned = self.shadow_pass_aligned as usize;
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
                queue.write_buffer(&self.shadow_pass_buffer, 0, &sp_staging);
            }
            {
                let mut dpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("BasicRenderer Shadow Depth Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_map.view,
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
                dpass.set_pipeline(&self.shadow_pipeline);
                for (i, item) in items.iter().take(n).enumerate() {
                    let mesh = assets.mesh(item.mesh).expect("collect skipped stale");
                    let off = (i as u32) * self.shadow_pass_aligned;
                    dpass.set_bind_group(0, &self.shadow_pass_bind_group, &[off]);
                    dpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    dpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    dpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                }
            }
            let su = ShadowMainUniform {
                light_view_proj: light_vp.cols,
                shadow_map_size: SHADOW_MAP_SIZE as f32,
                shadow_bias: 0.003,
                _pad: [0.0; 2],
            };
            queue.write_buffer(&self.shadow_main_buffer, 0, bytemuck::bytes_of(&su));
        } else {
            queue.write_buffer(
                &self.shadow_main_buffer,
                0,
                bytemuck::bytes_of(&ShadowMainUniform::disabled()),
            );
        }

        let aligned = self.aligned_size as usize;

        // staging: one CameraUniform per entity, padded to alignment.
        let mut staging = vec![0u8; n * aligned];
        for (i, item) in items.iter().take(n).enumerate() {
            let uniform = ColoredCameraUniform {
                view_proj: view_proj.cols,
                model: item.model.cols,
                camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                _padding: 0.0,
                base_color: item.material.base_color,
            };
            let bytes = bytemuck::bytes_of(&uniform);
            let offset = i * aligned;
            staging[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        if n > 0 {
            queue.write_buffer(&self.camera_buffer, 0, &staging);
        }
        queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[light]));

        // ── Non-skinned (colored Blinn-Phong) pass ──────────────────────────
        {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Pass"),
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

        pass.set_pipeline(&self.pipeline);
        // Point lights (group 2) + shadow map (group 3) are shared across draws.
        pass.set_bind_group(2, &self.point_lights_bind_group, &[]);
        pass.set_bind_group(3, &self.shadow_main_bind_group, &[]);

        for (i, item) in items.iter().take(n).enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
            // Per-entity diffuse texture, or the white default.
            let tex_bg = item
                .texture
                .and_then(|h| assets.texture(h))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture.bind_group);
            pass.set_bind_group(1, tex_bg, &[]);
            let dynamic_offset = (i as u32) * self.aligned_size;
            pass.set_bind_group(0, &self.camera_light_bind_group, &[dynamic_offset]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
        } // end non-skinned pass

        self.render_skinned(queue, encoder, target_view, depth_view, world, assets, view_proj);
    }

    /// Second pass: GPU vertex skinning for entities carrying both a
    /// [`SkinnedMeshHandle`] and a [`SkinnedPose`]. Draws over the 3D scene
    /// with `LoadOp::Load` (color + depth), so it composes with the
    /// non-skinned pass. Capped at [`MAX_SKINNED`] entities per frame.
    #[allow(clippy::too_many_arguments)]
    fn render_skinned(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        world: &mut World,
        assets: &RenderAssets,
        view_proj: Mat4,
    ) {
        // (model matrix, mesh handle, bone palette) per skinned entity.
        let items: Vec<(Mat4, SkinnedMeshHandle, BonePalette)> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, handle, pose)| (wt.0, *handle, pose.palette()))
            .collect();
        let n = items.len().min(MAX_SKINNED);
        if n == 0 {
            return;
        }

        let cam_aligned = self.skinned_cam_aligned as usize;
        let pal_aligned = self.palette_aligned as usize;
        let mut cam_staging = vec![0u8; n * cam_aligned];
        let mut pal_staging = vec![0u8; n * pal_aligned];
        for (i, (model, _, palette)) in items.iter().take(n).enumerate() {
            let cam = SkinnedCameraUniform {
                view_proj: view_proj.cols,
                model: model.cols,
            };
            let cb = bytemuck::bytes_of(&cam);
            cam_staging[i * cam_aligned..i * cam_aligned + cb.len()].copy_from_slice(cb);
            let pb = bytemuck::bytes_of(palette);
            pal_staging[i * pal_aligned..i * pal_aligned + pb.len()].copy_from_slice(pb);
        }
        queue.write_buffer(&self.skinned_cam_buffer, 0, &cam_staging);
        queue.write_buffer(&self.palette_buffer, 0, &pal_staging);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Skinned Pass"),
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
        pass.set_pipeline(&self.skinned_pipeline);
        for (i, (_, handle, _)) in items.iter().take(n).enumerate() {
            let Some(mesh) = assets.skinned_mesh(*handle) else {
                continue;
            };
            let cam_off = (i * cam_aligned) as u32;
            let pal_off = (i * pal_aligned) as u32;
            pass.set_bind_group(0, &self.skinned_cam_bind_group, &[cam_off]);
            pass.set_bind_group(1, &self.palette_bind_group, &[pal_off]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}

/// Orthographic light view-projection for a directional light, framing a fixed
/// area around the origin. Conservative coverage for typical small scenes;
/// per-scene tuning (fit to camera frustum) is future work.
fn directional_light_vp(light_dir: [f32; 3]) -> Mat4 {
    let dir = Vec3::new(light_dir[0], light_dir[1], light_dir[2]).normalize_or_zero();
    let dir = if dir == Vec3::ZERO {
        Vec3::new(0.0, -1.0, 0.0)
    } else {
        dir
    };
    const EXTENT: f32 = 30.0;
    let center = Vec3::ZERO;
    let light_pos = center - dir * EXTENT;
    let up = if dir.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Mat4::look_at(light_pos, center, up);
    let proj = Mat4::orthographic(-EXTENT, EXTENT, -EXTENT, EXTENT, 0.1, EXTENT * 2.5);
    proj * view
}

/// Conservative world-space bounding sphere of a local-space [`AABB`] under a
/// model matrix: transform the center, and scale the AABB's half-diagonal by
/// the largest model basis-vector length (handles rotation + scale).
fn world_bounding_sphere(model: Mat4, local: AABB) -> (Vec3, f32) {
    let c = local.center();
    let wc = model * Vec4::new(c.x, c.y, c.z, 1.0);
    let half = (local.max - local.min) * 0.5;
    let r = (half.x * half.x + half.y * half.y + half.z * half.z).sqrt();
    let col_len = |i: usize| {
        let c = model.cols[i];
        (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
    };
    let scale = col_len(0).max(col_len(1)).max(col_len(2));
    (Vec3::new(wc.x, wc.y, wc.z), r * scale)
}

/// A dynamic-offset uniform bind group layout with a single vertex-visible
/// buffer binding of `min_size` bytes.
fn dynamic_uniform_layout(
    device: &wgpu::Device,
    label: &str,
    min_size: u64,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(min_size),
            },
            count: None,
        }],
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_bounding_sphere_identity_unit_cube() {
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let (c, r) = world_bounding_sphere(Mat4::IDENTITY, aabb);
        assert!((c - Vec3::ZERO).length() < 1e-5);
        assert!((r - 3.0_f32.sqrt()).abs() < 1e-4, "r={r}");
    }

    #[test]
    fn world_bounding_sphere_translation_and_scale() {
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let m = Mat4::translation(5.0, 0.0, 0.0) * Mat4::scale(2.0, 2.0, 2.0);
        let (c, r) = world_bounding_sphere(m, aabb);
        assert!((c - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-4, "c={c:?}");
        assert!((r - 2.0 * 3.0_f32.sqrt()).abs() < 1e-3, "r={r}");
    }

    #[test]
    fn frustum_keeps_front_culls_behind() {
        let cam = voltex_renderer::Camera::new(Vec3::new(0.0, 0.0, 5.0), 1.0); // looks -Z
        let frustum = voltex_renderer::extract_frustum(&cam.view_projection());
        assert!(
            voltex_renderer::sphere_vs_frustum(Vec3::ZERO, 1.0, &frustum),
            "object in front should be visible"
        );
        assert!(
            !voltex_renderer::sphere_vs_frustum(Vec3::new(0.0, 0.0, 50.0), 1.0, &frustum),
            "object behind the camera should be culled"
        );
    }
}
