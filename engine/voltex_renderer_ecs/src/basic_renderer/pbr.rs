//! The opt-in PBR path of `BasicRenderer` (Cook-Torrance, multi-light,
//! 4-map materials, IBL BRDF LUT) for static and skinned meshes.
//!
//! Bind groups: 0 = camera (dynamic) + lights (+ bone palette when skinned),
//! 1 = albedo/normal/ORM/emissive maps, 2 = material (dynamic), 3 = shadow
//! map + `ShadowUniform` + BRDF LUT.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, create_pbr_skinned_pipeline,
    pbr_full_texture_bind_group_layout, BonePalette, CameraUniform, GpuTexture, IblResources,
    LightData, LightUniform, LightsUniform, MaterialUniform, PointLight, ShadowMap,
    ShadowUniform, SHADOW_MAP_SIZE,
};

use super::{
    directional_light_vp, single_buffer_bind_group, BasicRenderer, MAX_SKINNED, SHADOW_BIAS,
};
use crate::assets::{PbrTextures, RenderAssets, TextureHandle};
use crate::dynamic_uniform::DynamicUniformBuffer;
use crate::items::RenderItem;
use crate::scene_pass::begin_clearing_pass;

use super::skinned::SkinnedItem;

/// Turbidity of the procedural sky the PBR shader uses for ambient light
/// (no SH probe is bound, so `sh_coefficients` stay zero).
const SKY_TURBIDITY: f32 = 3.0;

/// Sky/sun direction written before the first frame (overwritten each frame
/// with the directional light's direction).
const DEFAULT_SUN_DIRECTION: [f32; 3] = [0.5, -0.7, 0.5];

/// A point light whose `spot_cos_outer` is below this is a plain point light;
/// otherwise it is a spot light (`PointLight` packs both).
const POINT_LIGHT_SPOT_SENTINEL: f32 = -1.5;

/// Spot inner cone = outer cone minus this many degrees (soft edge).
const SPOT_SOFT_EDGE_DEG: f32 = 5.0;

pub(super) struct PbrResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    /// Group 1 layout (albedo/normal/ORM/emissive maps).
    pub(super) texture_layout: wgpu::BindGroupLayout,
    /// Group 2 layout, shared with the skinned PBR pipeline.
    pub(super) material_layout: wgpu::BindGroupLayout,
    /// Group 3 layout, shared with the skinned PBR pipeline.
    pub(super) shadow_layout: wgpu::BindGroupLayout,
    pub(super) cameras: DynamicUniformBuffer<CameraUniform>,
    pub(super) lights_buffer: wgpu::Buffer,
    pub(super) camera_light_bind_group: wgpu::BindGroup,
    pub(super) materials: DynamicUniformBuffer<MaterialUniform>,
    pub(super) material_bind_group: wgpu::BindGroup,
    pub(super) shadow_uniform_buffer: wgpu::Buffer,
    pub(super) shadow_bind_group: wgpu::BindGroup,
    // Default 1×1 maps for absent PBR slots (white albedo/ORM come from
    // `BasicRenderer::default_texture`).
    pub(super) flat_normal: GpuTexture,
    pub(super) black: GpuTexture,
    /// All four slots at their defaults — bound for entities without maps.
    pub(super) default_maps_bind_group: wgpu::BindGroup,
}

impl PbrResources {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        alignment: u32,
        max_entities: usize,
        texture_layout: &wgpu::BindGroupLayout,
        default_texture: &GpuTexture,
        shadow_map: &ShadowMap,
    ) -> Self {
        let camera_light_layout = pbr_camera_light_layout(device);
        let maps_layout = pbr_full_texture_bind_group_layout(device);
        let material_layout = MaterialUniform::bind_group_layout(device);
        let shadow_layout = ShadowMap::bind_group_layout(device);
        let pipeline = create_pbr_pipeline(
            device,
            target_format,
            &camera_light_layout,
            &maps_layout,
            &material_layout,
            &shadow_layout,
        );

        // Group 0: per-entity camera + shared multi-light uniform.
        let cameras = DynamicUniformBuffer::<CameraUniform>::new(
            device,
            "BasicRenderer PBR Camera Dynamic UBO",
            alignment,
            max_entities,
        );
        let lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Lights UBO"),
            size: std::mem::size_of::<LightsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&lights_buffer, 0, bytemuck::bytes_of(&LightsUniform::new()));
        let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer PBR Camera+Light BG"),
            layout: &camera_light_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cameras.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.as_entire_binding(),
                },
            ],
        });

        // Group 2: per-entity material.
        let materials = DynamicUniformBuffer::<MaterialUniform>::new(
            device,
            "BasicRenderer PBR Material Dynamic UBO",
            alignment,
            max_entities,
        );
        let material_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer PBR Material BG",
            &material_layout,
            materials.binding(),
        );

        // Group 1 defaults: white albedo, flat normal, white ORM, black emissive.
        let flat_normal = GpuTexture::flat_normal_map_1x1(device, queue, texture_layout);
        let black = GpuTexture::black_1x1(device, queue, texture_layout);
        let default_maps_bind_group = pbr_maps_bind_group(
            device,
            &maps_layout,
            default_texture,
            &flat_normal,
            default_texture,
            &black,
        );

        // Group 3: depth map + comparison sampler + ShadowUniform + BRDF LUT.
        let ibl = IblResources::new(device, queue);
        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer PBR Shadow UBO"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &shadow_uniform_buffer,
            0,
            bytemuck::bytes_of(&pbr_shadow_disabled()),
        );
        let shadow_bind_group = shadow_map.create_bind_group(
            device,
            &shadow_layout,
            &shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );
        Self {
            pipeline,
            texture_layout: maps_layout,
            material_layout,
            shadow_layout,
            cameras,
            lights_buffer,
            camera_light_bind_group,
            materials,
            material_bind_group,
            shadow_uniform_buffer,
            shadow_bind_group,
            flat_normal,
            black,
            default_maps_bind_group,
        }
    }
}

pub(super) struct PbrSkinnedResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) cameras: DynamicUniformBuffer<CameraUniform>,
    /// Group 0: camera (dynamic) + PBR lights + shared bone palette (dynamic).
    pub(super) camera_light_palette_bind_group: wgpu::BindGroup,
    pub(super) materials: DynamicUniformBuffer<MaterialUniform>,
    pub(super) material_bind_group: wgpu::BindGroup,
}

impl PbrSkinnedResources {
    /// Groups 1–3 reuse the static PBR layouts; group 0 binds the PBR lights
    /// buffer and the shared skinned bone palette.
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        alignment: u32,
        pbr: &PbrResources,
        palettes: &DynamicUniformBuffer<BonePalette>,
    ) -> Self {
        let camera_light_palette_layout = pbr_skinned_camera_light_palette_layout(device);
        let pipeline = create_pbr_skinned_pipeline(
            device,
            target_format,
            &camera_light_palette_layout,
            &pbr.texture_layout,
            &pbr.material_layout,
            &pbr.shadow_layout,
        );
        let cameras = DynamicUniformBuffer::<CameraUniform>::new(
            device,
            "BasicRenderer PBR Skinned Camera UBO",
            alignment,
            MAX_SKINNED,
        );
        let camera_light_palette_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("BasicRenderer PBR Skinned g0 BG"),
                layout: &camera_light_palette_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: cameras.binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: pbr.lights_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: palettes.binding(),
                    },
                ],
            });
        let materials = DynamicUniformBuffer::<MaterialUniform>::new(
            device,
            "BasicRenderer PBR Skinned Material Dynamic UBO",
            alignment,
            MAX_SKINNED,
        );
        let material_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer PBR Skinned Material BG",
            &pbr.material_layout,
            materials.binding(),
        );
        Self {
            pipeline,
            cameras,
            camera_light_palette_bind_group,
            materials,
            material_bind_group,
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
    pub(super) fn pbr_mesh_pass(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        items: &[RenderItem],
        view_proj: Mat4,
        cam_pos: Vec3,
        light: LightUniform,
        assets: &RenderAssets,
    ) {
        // Shadow group: full ShadowUniform (also carries the sky direction +
        // zeroed SH → shader falls back to procedural ambient).
        let shadow_map_size = if self.shadows_enabled { SHADOW_MAP_SIZE as f32 } else { 0.0 };
        let shadow_uniform = pbr_shadow_uniform(
            directional_light_vp(light.direction),
            shadow_map_size,
            SHADOW_BIAS,
            light.direction,
        );
        queue.write_buffer(&self.pbr.shadow_uniform_buffer, 0, bytemuck::bytes_of(&shadow_uniform));

        let lights = pbr_lights_uniform(light, &self.point_lights_cpu.borrow());
        queue.write_buffer(&self.pbr.lights_buffer, 0, bytemuck::bytes_of(&lights));

        let cameras: Vec<CameraUniform> =
            items.iter().map(|item| pbr_camera(view_proj, item.model, cam_pos)).collect();
        let materials: Vec<MaterialUniform> = items.iter().map(|item| item.material).collect();
        self.pbr.cameras.upload(queue, &cameras);
        self.pbr.materials.upload(queue, &materials);

        let maps_bind_groups: Vec<Option<wgpu::BindGroup>> = items
            .iter()
            .map(|item| self.entity_maps_bind_group(assets, item.texture, item.pbr))
            .collect();

        let mut pass = begin_clearing_pass(
            encoder,
            "BasicRenderer PBR Pass",
            target_view,
            depth_view,
            clear_color,
        );
        pass.set_pipeline(&self.pbr.pipeline);
        pass.set_bind_group(3, &self.pbr.shadow_bind_group, &[]);
        for (i, item) in items.iter().enumerate() {
            let mesh = assets
                .mesh(item.mesh)
                .expect("collect_render_items already skipped stale handles");
            pass.set_bind_group(0, &self.pbr.camera_light_bind_group, &[self.pbr.cameras.offset(i)]);
            let maps = maps_bind_groups[i].as_ref().unwrap_or(&self.pbr.default_maps_bind_group);
            pass.set_bind_group(1, maps, &[]);
            pass.set_bind_group(2, &self.pbr.material_bind_group, &[self.pbr.materials.offset(i)]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }

    /// Skinned PBR, before the skinned pass opens: upload per-entity camera +
    /// material and build each entity's 4-map bind group (`None` → defaults).
    pub(super) fn prepare_skinned_pbr(
        &self,
        queue: &wgpu::Queue,
        assets: &RenderAssets,
        items: &[SkinnedItem],
        view_proj: Mat4,
        cam_pos: Vec3,
    ) -> Vec<Option<wgpu::BindGroup>> {
        let res = &self.pbr_skinned;
        let cameras: Vec<CameraUniform> =
            items.iter().map(|it| pbr_camera(view_proj, it.model, cam_pos)).collect();
        res.cameras.upload(queue, &cameras);
        let materials: Vec<MaterialUniform> = items.iter().map(|it| it.material).collect();
        res.materials.upload(queue, &materials);
        items
            .iter()
            .map(|it| self.entity_maps_bind_group(assets, it.texture, it.pbr))
            .collect()
    }

    /// Skinned PBR draws inside the skinned pass (uniforms, palettes and
    /// `maps_bind_groups` from [`Self::prepare_skinned_pbr`]).
    pub(super) fn draw_skinned_pbr(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        assets: &RenderAssets,
        items: &[SkinnedItem],
        maps_bind_groups: &[Option<wgpu::BindGroup>],
    ) {
        let res = &self.pbr_skinned;
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(3, &self.pbr.shadow_bind_group, &[]);
        for (i, it) in items.iter().enumerate() {
            let Some(mesh) = assets.skinned_mesh(it.handle) else {
                continue;
            };
            // Group 0 has two dynamic bindings (camera @0, palette @2).
            pass.set_bind_group(
                0,
                &res.camera_light_palette_bind_group,
                &[res.cameras.offset(i), self.skinned.palettes.offset(i)],
            );
            let maps = maps_bind_groups[i].as_ref().unwrap_or(&self.pbr.default_maps_bind_group);
            pass.set_bind_group(1, maps, &[]);
            pass.set_bind_group(2, &res.material_bind_group, &[res.materials.offset(i)]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }

    /// One entity's albedo/normal/ORM/emissive bind group, defaults for absent
    /// slots. `None` when the entity has no maps at all → the caller binds the
    /// shared `default_maps_bind_group` (no per-frame allocation in the common
    /// untextured case).
    fn entity_maps_bind_group(
        &self,
        assets: &RenderAssets,
        albedo: Option<TextureHandle>,
        maps: Option<PbrTextures>,
    ) -> Option<wgpu::BindGroup> {
        let has_extra_maps = maps.is_some_and(|m| m.normal.or(m.orm).or(m.emissive).is_some());
        if albedo.is_none() && !has_extra_maps {
            return None;
        }
        let texture_or = |handle: Option<TextureHandle>, fallback| {
            handle.and_then(|h| assets.texture(h)).unwrap_or(fallback)
        };
        Some(pbr_maps_bind_group(
            &self.device,
            &self.pbr.texture_layout,
            texture_or(albedo, &self.default_texture),
            texture_or(maps.and_then(|m| m.normal), &self.pbr.flat_normal),
            texture_or(maps.and_then(|m| m.orm), &self.default_texture),
            texture_or(maps.and_then(|m| m.emissive), &self.pbr.black),
        ))
    }
}

fn pbr_camera(view_proj: Mat4, model: Mat4, cam_pos: Vec3) -> CameraUniform {
    CameraUniform {
        view_proj: view_proj.cols,
        model: model.cols,
        camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
        _padding: 0.0,
    }
}

/// Directional sun + the point/spot lights folded into one `LightsUniform`;
/// ambient = sun color × `ambient_strength`.
fn pbr_lights_uniform(sun: LightUniform, point_lights: &[PointLight]) -> LightsUniform {
    let mut lights = LightsUniform::new();
    lights.add_light(LightData::directional(sun.direction, sun.color, 1.0));
    for pl in point_lights {
        if pl.spot_cos_outer < POINT_LIGHT_SPOT_SENTINEL {
            lights.add_light(LightData::point(pl.position, pl.color, pl.intensity, pl.range));
        } else {
            let outer_deg = pl.spot_cos_outer.clamp(-1.0, 1.0).acos().to_degrees();
            let inner_deg = (outer_deg - SPOT_SOFT_EDGE_DEG).max(0.0);
            lights.add_light(LightData::spot(
                pl.position, pl.direction, pl.color, pl.intensity, pl.range, inner_deg,
                outer_deg,
            ));
        }
    }
    lights.ambient_color = [
        sun.color[0] * sun.ambient_strength,
        sun.color[1] * sun.ambient_strength,
        sun.color[2] * sun.ambient_strength,
    ];
    lights
}

/// Group-1 bind group from four textures (albedo, normal, ORM, emissive).
fn pbr_maps_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    albedo: &GpuTexture,
    normal: &GpuTexture,
    orm: &GpuTexture,
    emissive: &GpuTexture,
) -> wgpu::BindGroup {
    create_pbr_full_texture_bind_group(
        device,
        layout,
        &albedo.view,
        &albedo.sampler,
        &normal.view,
        &normal.sampler,
        &orm.view,
        &orm.sampler,
        &emissive.view,
        &emissive.sampler,
    )
}

/// PBR group-0 layout: dynamic-offset `CameraUniform` (binding 0, vertex +
/// fragment) + a shared `LightsUniform` (binding 1, fragment). Mirrors the
/// layout `create_pbr_pipeline` expects.
fn pbr_camera_light_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer PBR Camera+Light BGL"),
        entries: &[camera_layout_entry(), lights_layout_entry()],
    })
}

/// Skinned-PBR group-0 layout: camera (binding 0, dynamic, vertex+fragment) +
/// lights (binding 1, fragment) + bone palette (binding 2, dynamic, vertex).
fn pbr_skinned_camera_light_palette_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("BasicRenderer PBR Skinned g0 BGL"),
        entries: &[
            camera_layout_entry(),
            lights_layout_entry(),
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

fn camera_layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
        },
        count: None,
    }
}

fn lights_layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 1,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// `ShadowUniform` for the PBR shadow group. The sky uses `sun_direction`,
/// [`SKY_TURBIDITY`] and zero SH coefficients (procedural ambient).
fn pbr_shadow_uniform(
    light_view_proj: Mat4,
    shadow_map_size: f32,
    shadow_bias: f32,
    sun_direction: [f32; 3],
) -> ShadowUniform {
    ShadowUniform {
        light_view_proj: light_view_proj.cols,
        shadow_map_size,
        shadow_bias,
        _padding: [0.0; 2],
        sun_direction,
        turbidity: SKY_TURBIDITY,
        sh_coefficients: [[0.0; 4]; 7],
    }
}

/// Shadows off (`shadow_map_size = 0`) — the value before the first frame.
fn pbr_shadow_disabled() -> ShadowUniform {
    pbr_shadow_uniform(Mat4::IDENTITY, 0.0, 0.0, DEFAULT_SUN_DIRECTION)
}
