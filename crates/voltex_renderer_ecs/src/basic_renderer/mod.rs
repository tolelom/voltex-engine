//! BasicRenderer — mesh_shader.wgsl 기반 ECS-driven 렌더러.
//!
//! pipeline + per-entity dynamic UBO + light buffer + 1×1 default texture
//! 를 모두 owning. `render(...)` 한 메서드로 World 를 그린다.

use voltex_ecs::World;
use voltex_math::{Mat4, Vec3, Vec4, AABB};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline,
    create_pbr_skinned_pipeline,
    pbr_full_texture_bind_group_layout,
    pipeline::create_colored_mesh_pipeline,
    BonePalette, CameraUniform, ColoredCameraUniform, GpuTexture, IblResources, LightData,
    LightUniform, LightsUniform, LineVertex, MaterialUniform, PointLight,
    PointLightsUniform, ShadowMap, ShadowUniform,
    SHADOW_MAP_SIZE,
};

use std::cell::{Cell, RefCell};

mod shadow;
mod skinned;

use shadow::ShadowResources;
use skinned::SkinnedResources;

use crate::assets::RenderAssets;
use crate::items::collect_render_items;

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
    // Skinned pass (GPU vertex skinning) + skinned shadow casters.
    skinned: SkinnedResources,
    // Directional shadow mapping (off by default; enable with set_shadows).
    shadows_enabled: bool,
    shadow: ShadowResources,
    // Immediate-mode debug lines (collider/nav/ray gizmos). Drawn last, over the
    // 3D scene, depth-tested but not depth-writing.
    debug_line_pass: crate::debug_line_pass::DebugLinePass,
    // Billboard sprites (camera-facing quads). Drawn after meshes + lines,
    // alpha-blended, depth-tested but not depth-writing.
    sprite_pass: crate::sprite_pass::SpritePass,
    // ── Full PBR mesh path (opt-in via set_pbr_enabled) ─────────────────────
    pbr_enabled: bool,
    pbr_pipeline: wgpu::RenderPipeline,
    pbr_tex_layout: wgpu::BindGroupLayout,
    pbr_camera_buffer: wgpu::Buffer,
    pbr_camera_aligned: u32,
    pbr_lights_buffer: wgpu::Buffer,
    pbr_camera_light_bind_group: wgpu::BindGroup,
    pbr_material_buffer: wgpu::Buffer,
    pbr_material_aligned: u32,
    pbr_material_bind_group: wgpu::BindGroup,
    pbr_shadow_uniform_buffer: wgpu::Buffer,
    pbr_shadow_bind_group: wgpu::BindGroup,
    // Default 1×1 maps for absent PBR slots (flat normal / white ORM / black emissive).
    pbr_flat_normal: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
    pbr_black: GpuTexture,
    pbr_default_texture_bind_group: wgpu::BindGroup,
    // Last-set point lights, kept CPU-side so the PBR pass can fold them into
    // the multi-light `LightsUniform` (the colored pass uses the GPU buffer).
    point_lights_cpu: RefCell<Vec<PointLight>>,
    // Device handle (cheap Arc clone) so the PBR pass can build per-entity
    // 4-map texture bind groups, which need the device at build time.
    device: wgpu::Device,
    // One-time overflow warnings (so silent per-frame caps don't read as
    // "everything drew"). Set the first time each cap is exceeded.
    mesh_overflow_warned: Cell<bool>,
    skinned_overflow_warned: Cell<bool>,
    /// Batched `Tilemap` pass (drawn before sprites).
    tilemap_pass: crate::tilemap_pass::TilemapPass,
    // ── Skinned PBR (used when both pbr_enabled and skinned entities exist) ──
    pbr_skinned_pipeline: wgpu::RenderPipeline,
    pbr_skinned_cam_buffer: wgpu::Buffer,
    pbr_skinned_cam_aligned: u32,
    pbr_skinned_g0_bind_group: wgpu::BindGroup,
    pbr_skinned_material_buffer: wgpu::Buffer,
    pbr_skinned_material_aligned: u32,
    pbr_skinned_material_bind_group: wgpu::BindGroup,
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
        let (shadow, shadow_main_layout) =
            ShadowResources::new(device, queue, alignment, max_entities);

        let pipeline = create_colored_mesh_pipeline(
            device,
            target_format,
            &cl_layout,
            &tex_layout,
            &pl_layout,
            &shadow_main_layout,
        );

        // ── Skinned pass resources ──────────────────────────────────────────
        let skinned = SkinnedResources::new(device, target_format, alignment);

        // ── Debug line resources ────────────────────────────────────────────
        let debug_line_pass = crate::debug_line_pass::DebugLinePass::new(device, target_format);

        let tilemap_pass =
            crate::tilemap_pass::TilemapPass::new(device, queue, target_format, &tex_layout);

        // ── Billboard sprite resources ──────────────────────────────────────
        let sprite_pass = crate::sprite_pass::SpritePass::new(
            device, queue, target_format, &tex_layout, alignment,
        );

        // ── Full PBR mesh path resources (dormant until set_pbr_enabled) ────
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
        let pbr_black = GpuTexture::black_1x1(device, queue, &tex_layout);
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
        let pbr_shadow_bind_group = shadow.map.create_bind_group(
            device,
            &pbr_shadow_layout,
            &pbr_shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );

        // ── Skinned PBR: g0 = camera(dyn) + lights(static) + palette(dyn) ────
        let pbr_skinned_g0_layout = pbr_skinned_g0_layout(device);
        let pbr_skinned_pipeline = create_pbr_skinned_pipeline(
            device,
            target_format,
            &pbr_skinned_g0_layout,
            &pbr_tex_layout,
            &pbr_mat_layout,
            &pbr_shadow_layout,
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
                    resource: pbr_lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &skinned.palette_buffer,
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
            layout: &pbr_mat_layout,
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
            pipeline,
            camera_buffer,
            light_buffer,
            camera_light_bind_group,
            default_texture,
            aligned_size,
            max_entities,
            point_lights_buffer,
            point_lights_bind_group,
            skinned,
            shadows_enabled: false,
            shadow,
            debug_line_pass,
            sprite_pass,
            pbr_enabled: false,
            pbr_pipeline,
            pbr_tex_layout,
            pbr_camera_buffer,
            pbr_camera_aligned,
            pbr_lights_buffer,
            pbr_camera_light_bind_group,
            pbr_material_buffer,
            pbr_material_aligned,
            pbr_material_bind_group,
            pbr_shadow_uniform_buffer,
            pbr_shadow_bind_group,
            pbr_flat_normal,
            pbr_black,
            pbr_default_texture_bind_group,
            point_lights_cpu: RefCell::new(Vec::new()),
            device: device.clone(),
            mesh_overflow_warned: Cell::new(false),
            skinned_overflow_warned: Cell::new(false),
            tilemap_pass,
            pbr_skinned_pipeline,
            pbr_skinned_cam_buffer,
            pbr_skinned_cam_aligned,
            pbr_skinned_g0_bind_group,
            pbr_skinned_material_buffer,
            pbr_skinned_material_aligned,
            pbr_skinned_material_bind_group,
        }
    }

    /// Enable the full PBR mesh path (Cook-Torrance + multi-light + material
    /// maps + IBL) instead of the default colored Blinn-Phong pass. Off by
    /// default; the colored path is byte-identical when disabled. Sprites,
    /// particles, debug lines, skinned, and shadow passes are unaffected.
    pub fn set_pbr_enabled(&mut self, enabled: bool) {
        self.pbr_enabled = enabled;
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
        // Keep a CPU copy so the PBR pass can fold them into its multi-light
        // LightsUniform (the colored pass reads only the GPU buffer above).
        *self.point_lights_cpu.borrow_mut() = lights.to_vec();
    }

    /// Set the immediate-mode debug line segments for the next frame. `verts`
    /// is a flat list of endpoints (2 per segment); excess beyond
    /// `MAX_DEBUG_LINE_VERTICES` is dropped. Pass an empty slice to clear.
    /// Call once per frame before [`render`](Self::render) (cheap uniform/vertex
    /// write).
    pub fn set_debug_lines(&self, queue: &wgpu::Queue, verts: &[LineVertex]) {
        self.debug_line_pass.set_lines(queue, verts);
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
        warn_overflow_once(&self.mesh_overflow_warned, "mesh entities", items.len(), self.max_entities);
        let n = items.len().min(self.max_entities);

        self.shadow_prepass(queue, encoder, world, assets, &items, n, light);

        if self.pbr_enabled {
            // PBR shadow group uses the full ShadowUniform (also carries the sky
            // direction + zeroed SH → shader falls back to procedural ambient).
            let su = ShadowUniform {
                light_view_proj: directional_light_vp(light.direction).cols,
                shadow_map_size: if self.shadows_enabled { SHADOW_MAP_SIZE as f32 } else { 0.0 },
                shadow_bias: 0.003,
                _padding: [0.0; 2],
                sun_direction: light.direction,
                turbidity: 3.0,
                sh_coefficients: [[0.0; 4]; 7],
            };
            queue.write_buffer(&self.pbr_shadow_uniform_buffer, 0, bytemuck::bytes_of(&su));
            self.pbr_color_pass(
                queue, encoder, target_view, depth_view, clear_color, &items, n, view_proj,
                cam_pos, light, assets,
            );
        } else {
            self.colored_color_pass(
                queue, encoder, target_view, depth_view, clear_color, &items, n, view_proj,
                cam_pos, light, assets,
            );
        }

        self.render_skinned(queue, encoder, target_view, depth_view, world, assets, view_proj, cam_pos);
        self.tilemap_pass.render(
            queue, encoder, target_view, depth_view, world, assets, &self.default_texture, view_proj,
        );
        self.sprite_pass.render(
            queue, encoder, target_view, depth_view, world, assets, &self.default_texture,
            view_proj, cam_pos,
        );
        self.debug_line_pass.render(queue, encoder, target_view, depth_view, view_proj);
    }

    /// The default colored Blinn-Phong mesh pass (clears the frame). Extracted
    /// so `render` can pick it or the PBR pass.
    #[allow(clippy::too_many_arguments)]
    fn colored_color_pass(
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
        let aligned = self.aligned_size as usize;
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
        pass.set_bind_group(2, &self.point_lights_bind_group, &[]);
        pass.set_bind_group(3, &self.shadow.main_bind_group, &[]);
        for (i, item) in items.iter().take(n).enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
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
    }

    /// The full PBR (Cook-Torrance + multi-light + material maps + IBL) mesh
    /// pass (clears the frame). Folds the directional `light` + the CPU copy of
    /// the point lights into a multi-light `LightsUniform`, stages per-entity
    /// `CameraUniform`/`MaterialUniform`, builds a 4-map texture bind group per
    /// entity (defaults for absent slots), and draws.
    #[allow(clippy::too_many_arguments)]
    fn pbr_color_pass(
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
        queue.write_buffer(&self.pbr_lights_buffer, 0, bytemuck::bytes_of(&lights));

        // ── Per-entity camera + material staging ────────────────────────────
        let cam_aligned = self.pbr_camera_aligned as usize;
        let mat_aligned = self.pbr_material_aligned as usize;
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
            queue.write_buffer(&self.pbr_camera_buffer, 0, &cam_staging);
            queue.write_buffer(&self.pbr_material_buffer, 0, &mat_staging);
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
                    .unwrap_or((&self.pbr_flat_normal.1, &self.pbr_flat_normal.2));
                let (ov, osamp) = resolve(item.pbr.and_then(|p| p.orm))
                    .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                let (ev, esamp) = resolve(item.pbr.and_then(|p| p.emissive))
                    .unwrap_or((&self.pbr_black.view, &self.pbr_black.sampler));
                Some(create_pbr_full_texture_bind_group(
                    &self.device,
                    &self.pbr_tex_layout,
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
        pass.set_pipeline(&self.pbr_pipeline);
        pass.set_bind_group(3, &self.pbr_shadow_bind_group, &[]);
        for (i, item) in items.iter().take(n).enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
            pass.set_bind_group(0, &self.pbr_camera_light_bind_group, &[(i as u32) * self.pbr_camera_aligned]);
            let tex_bg = tex_bgs[i].as_ref().unwrap_or(&self.pbr_default_texture_bind_group);
            pass.set_bind_group(1, tex_bg, &[]);
            pass.set_bind_group(2, &self.pbr_material_bind_group, &[(i as u32) * self.pbr_material_aligned]);
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

/// Emit a one-time `eprintln!` the first time a per-frame draw cap is exceeded,
/// so silently-dropped objects don't read as "everything rendered". `flag` is
/// flipped after the first warning.
pub(crate) fn warn_overflow_once(flag: &Cell<bool>, kind: &str, count: usize, cap: usize) {
    if count > cap && !flag.get() {
        flag.set(true);
        eprintln!(
            "voltex_renderer_ecs: {count} {kind} exceed the per-frame cap of {cap}; \
             extra {kind} are not drawn (this warning fires once)."
        );
    }
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
    fn warn_overflow_once_fires_only_on_overflow_and_only_once() {
        let flag = Cell::new(false);
        // Within cap → no warn, flag stays false.
        warn_overflow_once(&flag, "sprites", 100, 256);
        assert!(!flag.get());
        // Over cap → warns once, flag set.
        warn_overflow_once(&flag, "sprites", 300, 256);
        assert!(flag.get());
        // Already warned → stays set, no re-fire path (flag unchanged).
        warn_overflow_once(&flag, "sprites", 999, 256);
        assert!(flag.get());
    }

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
