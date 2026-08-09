//! BasicRenderer — mesh_shader.wgsl 기반 ECS-driven 렌더러.
//!
//! pipeline + per-entity dynamic UBO + light buffer + 1×1 default texture
//! 를 모두 owning. `render(...)` 한 메서드로 World 를 그린다.

use voltex_ecs::{MaterialHandle, World, WorldTransform};
use voltex_math::{Mat4, Vec3, Vec4, AABB};
use voltex_renderer::{
    create_line_pipeline, create_pbr_full_texture_bind_group, create_pbr_pipeline,
    create_pbr_skinned_pipeline, create_shadow_pipeline, create_skinned_shadow_pipeline,
    create_sprite_pipeline,
    line_camera_bind_group_layout,
    pbr_full_texture_bind_group_layout,
    pipeline::{create_colored_mesh_pipeline, create_skinned_mesh_pipeline},
    shadow_main_bind_group_layout, shadow_pass_bind_group_layout, sprite_uniform_bind_group_layout,
    BonePalette, CameraUniform, ColoredCameraUniform, GpuTexture, IblResources, LightData,
    LightUniform, LightsUniform, LineCameraUniform, LineVertex, MaterialUniform, PointLight,
    PointLightsUniform, ShadowMainUniform, ShadowMap, ShadowPassUniform, ShadowUniform,
    SkinnedCameraUniform, SpriteUniform, SPRITE_INDICES, SPRITE_QUAD, SHADOW_MAP_SIZE,
};

use crate::sprite::Sprite;
use std::cell::{Cell, RefCell};

/// Max debug-line vertices uploaded per frame (2 per segment). Fixed-capacity
/// so `set_debug_lines` can stay `&self` (mirrors `set_point_lights`).
const MAX_DEBUG_LINE_VERTICES: usize = 8192;

/// Max billboard sprites drawn per frame (each needs one dynamic-UBO slot).
const MAX_SPRITES: usize = 256;

use crate::assets::RenderAssets;
use crate::items::collect_render_items;
use crate::skinned::{SkinnedMeshHandle, SkinnedPose};

/// Max skinned entities drawn per frame. Each needs an 8 KB bone-palette slot,
/// so this is kept small + independent of `max_entities` to bound VRAM.
const MAX_SKINNED: usize = 64;

/// Per-skinned-entity render data gathered for one frame.
struct SkinnedItem {
    model: Mat4,
    handle: SkinnedMeshHandle,
    palette: BonePalette,
    material: MaterialUniform,
    texture: Option<crate::assets::TextureHandle>,
    pbr: Option<crate::assets::PbrTextures>,
}

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
    // Skinned shadow caster: depth-only skinning pipeline + a light-space camera
    // UBO (reuses the palette buffer + skinned_cam/palette layouts). Lets skinned
    // meshes write into the directional shadow map alongside static meshes.
    skinned_shadow_pipeline: wgpu::RenderPipeline,
    skinned_shadow_cam_buffer: wgpu::Buffer,
    skinned_shadow_cam_bind_group: wgpu::BindGroup,
    // Directional shadow mapping (off by default; enable with set_shadows).
    shadows_enabled: bool,
    shadow_map: ShadowMap,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_pass_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    shadow_pass_aligned: u32,
    shadow_main_buffer: wgpu::Buffer,
    shadow_main_bind_group: wgpu::BindGroup,
    // Immediate-mode debug lines (collider/nav/ray gizmos). Drawn last, over the
    // 3D scene, depth-tested but not depth-writing.
    line_pipeline: wgpu::RenderPipeline,
    line_camera_buffer: wgpu::Buffer,
    line_camera_bind_group: wgpu::BindGroup,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_count: Cell<u32>,
    // Billboard sprites (camera-facing quads). Drawn after meshes + lines,
    // alpha-blended, depth-tested but not depth-writing.
    sprite_pipeline: wgpu::RenderPipeline,
    sprite_quad_vbuf: wgpu::Buffer,
    sprite_quad_ibuf: wgpu::Buffer,
    sprite_uniform_buffer: wgpu::Buffer,
    sprite_uniform_bind_group: wgpu::BindGroup,
    sprite_uniform_aligned: u32,
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
    sprite_overflow_warned: Cell<bool>,
    skinned_overflow_warned: Cell<bool>,
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

        // Skinned shadow caster: depth-only pipeline (reuses the skinned camera +
        // palette layouts) and a dedicated light-space camera UBO. The palette
        // buffer is shared — render() uploads identical pose data for the shadow
        // pass and the main skinned pass each frame.
        let skinned_shadow_pipeline =
            create_skinned_shadow_pipeline(device, &skinned_cam_layout, &palette_layout);
        let skinned_shadow_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Skinned Shadow Camera Dynamic UBO"),
            size: (skinned_cam_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skinned_shadow_cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Skinned Shadow Camera BG"),
            layout: &skinned_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &skinned_shadow_cam_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<SkinnedCameraUniform>() as u64),
                }),
            }],
        });

        // ── Debug line resources ────────────────────────────────────────────
        let line_cam_layout = line_camera_bind_group_layout(device);
        let line_pipeline = create_line_pipeline(device, target_format, &line_cam_layout);
        let line_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Debug Line Camera UBO"),
            size: std::mem::size_of::<LineCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let line_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Debug Line Camera BG"),
            layout: &line_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: line_camera_buffer.as_entire_binding(),
            }],
        });
        let line_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Debug Line Vertex Buffer"),
            size: (MAX_DEBUG_LINE_VERTICES * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Billboard sprite resources ──────────────────────────────────────
        let sprite_uniform_layout = sprite_uniform_bind_group_layout(device);
        let sprite_pipeline =
            create_sprite_pipeline(device, target_format, &sprite_uniform_layout, &tex_layout);
        let sprite_quad_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Sprite Quad VB"),
            size: std::mem::size_of_val(&SPRITE_QUAD) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&sprite_quad_vbuf, 0, bytemuck::cast_slice(&SPRITE_QUAD));
        let sprite_quad_ibuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Sprite Quad IB"),
            size: std::mem::size_of_val(&SPRITE_INDICES) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&sprite_quad_ibuf, 0, bytemuck::cast_slice(&SPRITE_INDICES));
        let sprite_uniform_aligned =
            (std::mem::size_of::<SpriteUniform>() as u32).div_ceil(alignment) * alignment;
        let sprite_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Sprite Uniform Dynamic UBO"),
            size: (sprite_uniform_aligned as usize * MAX_SPRITES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sprite_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Sprite Uniform BG"),
            layout: &sprite_uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &sprite_uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<SpriteUniform>() as u64),
                }),
            }],
        });

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
        let pbr_shadow_bind_group = shadow_map.create_bind_group(
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
                        buffer: &palette_buffer,
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
            skinned_pipeline,
            skinned_cam_buffer,
            palette_buffer,
            skinned_cam_bind_group,
            palette_bind_group,
            skinned_cam_aligned,
            palette_aligned,
            skinned_shadow_pipeline,
            skinned_shadow_cam_buffer,
            skinned_shadow_cam_bind_group,
            shadows_enabled: false,
            shadow_map,
            shadow_pipeline,
            shadow_pass_buffer,
            shadow_pass_bind_group,
            shadow_pass_aligned,
            shadow_main_buffer,
            shadow_main_bind_group,
            line_pipeline,
            line_camera_buffer,
            line_camera_bind_group,
            line_vertex_buffer,
            line_vertex_count: Cell::new(0),
            sprite_pipeline,
            sprite_quad_vbuf,
            sprite_quad_ibuf,
            sprite_uniform_buffer,
            sprite_uniform_bind_group,
            sprite_uniform_aligned,
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
            sprite_overflow_warned: Cell::new(false),
            skinned_overflow_warned: Cell::new(false),
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
        let n = verts.len().min(MAX_DEBUG_LINE_VERTICES) & !1; // even count (pairs)
        if n > 0 {
            queue.write_buffer(
                &self.line_vertex_buffer,
                0,
                bytemuck::cast_slice(&verts[..n]),
            );
        }
        self.line_vertex_count.set(n as u32);
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
            // Skinned casters: upload light-space camera + shared palette, then
            // draw into the same depth map after the static meshes.
            let skinned_casters = self.gather_skinned_shadow(queue, world, light_vp);
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
                if !skinned_casters.is_empty() {
                    dpass.set_pipeline(&self.skinned_shadow_pipeline);
                    for (i, handle) in skinned_casters.iter().enumerate() {
                        let Some(mesh) = assets.skinned_mesh(*handle) else {
                            continue;
                        };
                        let cam_off = (i as u32) * self.skinned_cam_aligned;
                        let pal_off = (i as u32) * self.palette_aligned;
                        dpass.set_bind_group(0, &self.skinned_shadow_cam_bind_group, &[cam_off]);
                        dpass.set_bind_group(1, &self.palette_bind_group, &[pal_off]);
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
            queue.write_buffer(&self.shadow_main_buffer, 0, bytemuck::bytes_of(&su));
        } else {
            queue.write_buffer(
                &self.shadow_main_buffer,
                0,
                bytemuck::bytes_of(&ShadowMainUniform::disabled()),
            );
        }

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
        self.render_sprites(queue, encoder, target_view, depth_view, world, assets, view_proj, cam_pos);
        self.render_debug_lines(queue, encoder, target_view, depth_view, view_proj);
    }

    /// Billboard pass: draws every entity carrying a [`WorldTransform`] + a
    /// [`Sprite`] as a camera-facing quad at its world position (alpha-blended,
    /// depth-tested, no depth write). Camera right/up are recovered from the
    /// inverse view-projection, so no extra camera state is threaded in. Sprites
    /// are sorted back-to-front by distance from `cam_pos` so overlapping
    /// transparency blends correctly regardless of spawn order. Capped at
    /// [`MAX_SPRITES`] per frame.
    #[allow(clippy::too_many_arguments)]
    fn render_sprites(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        world: &mut World,
        assets: &RenderAssets,
        view_proj: Mat4,
        cam_pos: Vec3,
    ) {
        // (world position, sprite) for each visible sprite entity.
        let mut items: Vec<(Vec3, Sprite)> = world
            .query2::<WorldTransform, Sprite>()
            .filter(|(e, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, s)| {
                let c = wt.0.cols[3];
                (Vec3::new(c[0], c[1], c[2]), *s)
            })
            .collect();
        if items.is_empty() {
            return;
        }
        // World-space camera right/up/forward from the inverse VP (clip axes
        // map back to camera axes for a shear-free perspective/ortho proj).
        let inv = view_proj.inverse().unwrap_or(Mat4::IDENTITY);
        let dir = |v: Vec4| {
            let r = inv * v;
            Vec3::new(r.x, r.y, r.z).normalize_or_zero()
        };
        let right = dir(Vec4::new(1.0, 0.0, 0.0, 0.0));
        let up = dir(Vec4::new(0.0, 1.0, 0.0, 0.0));
        let forward = dir(Vec4::new(0.0, 0.0, 1.0, 0.0));

        warn_overflow_once(&self.sprite_overflow_warned, "sprites", items.len(), MAX_SPRITES);
        let n = cull_and_sort_sprites(&mut items, cam_pos, forward, MAX_SPRITES);

        let aligned = self.sprite_uniform_aligned as usize;
        let mut staging = vec![0u8; n * aligned];
        for (i, (pos, sprite)) in items.iter().take(n).enumerate() {
            let u = SpriteUniform {
                view_proj: view_proj.cols,
                camera_right: [right.x, right.y, right.z, 0.0],
                camera_up: [up.x, up.y, up.z, 0.0],
                center: [pos.x, pos.y, pos.z, 0.0],
                color: sprite.color,
                size: [sprite.size[0], sprite.size[1], 0.0, 0.0],
                uv_rect: sprite.effective_uv_rect(),
            };
            let b = bytemuck::bytes_of(&u);
            staging[i * aligned..i * aligned + b.len()].copy_from_slice(b);
        }
        queue.write_buffer(&self.sprite_uniform_buffer, 0, &staging);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Sprite Pass"),
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
        pass.set_pipeline(&self.sprite_pipeline);
        pass.set_vertex_buffer(0, self.sprite_quad_vbuf.slice(..));
        pass.set_index_buffer(self.sprite_quad_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        for (i, (_, sprite)) in items.iter().take(n).enumerate() {
            let tex_bg = sprite
                .texture
                .and_then(|h| assets.texture(h))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture.bind_group);
            pass.set_bind_group(1, tex_bg, &[]);
            let off = (i as u32) * self.sprite_uniform_aligned;
            pass.set_bind_group(0, &self.sprite_uniform_bind_group, &[off]);
            pass.draw_indexed(0..SPRITE_INDICES.len() as u32, 0, 0..1);
        }
    }

    /// Final pass: draw the debug line segments set via [`set_debug_lines`]
    /// (`LineList`, depth-tested against the scene, no depth write). No-op when
    /// no lines are queued.
    fn render_debug_lines(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view_proj: Mat4,
    ) {
        let count = self.line_vertex_count.get();
        if count == 0 {
            return;
        }
        let cam = LineCameraUniform {
            view_proj: view_proj.cols,
        };
        queue.write_buffer(&self.line_camera_buffer, 0, bytemuck::bytes_of(&cam));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Debug Line Pass"),
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
        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(0, &self.line_camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
        pass.draw(0..count, 0..1);
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
        pass.set_bind_group(3, &self.shadow_main_bind_group, &[]);
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

    /// Upload skinned shadow-caster data (light-space camera + shared palette)
    /// and return the per-entity mesh handles in draw order (capped at
    /// [`MAX_SKINNED`]). Mirrors the query in [`Self::render_skinned`] so the
    /// same entities both cast and receive. The palette buffer is shared with
    /// the main skinned pass; both upload identical pose data each frame, so the
    /// last write (whichever pass runs second) is byte-identical.
    fn gather_skinned_shadow(
        &self,
        queue: &wgpu::Queue,
        world: &World,
        light_vp: Mat4,
    ) -> Vec<SkinnedMeshHandle> {
        let items: Vec<(SkinnedMeshHandle, Mat4, voltex_renderer::BonePalette)> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, handle, pose)| (*handle, wt.0, pose.palette()))
            .collect();
        let n = items.len().min(MAX_SKINNED);
        if n == 0 {
            return Vec::new();
        }
        let cam_aligned = self.skinned_cam_aligned as usize;
        let pal_aligned = self.palette_aligned as usize;
        let mut cam_staging = vec![0u8; n * cam_aligned];
        let mut pal_staging = vec![0u8; n * pal_aligned];
        for (i, (_, model, palette)) in items.iter().take(n).enumerate() {
            let cam = SkinnedCameraUniform { view_proj: light_vp.cols, model: model.cols };
            let cb = bytemuck::bytes_of(&cam);
            cam_staging[i * cam_aligned..i * cam_aligned + cb.len()].copy_from_slice(cb);
            let pb = bytemuck::bytes_of(palette);
            pal_staging[i * pal_aligned..i * pal_aligned + pb.len()].copy_from_slice(pb);
        }
        queue.write_buffer(&self.skinned_shadow_cam_buffer, 0, &cam_staging);
        queue.write_buffer(&self.palette_buffer, 0, &pal_staging);
        items.into_iter().take(n).map(|(h, _, _)| h).collect()
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
        cam_pos: Vec3,
    ) {
        // Per skinned entity: model, mesh, palette, + (PBR only) material maps.
        let pbr = self.pbr_enabled;
        let items: Vec<SkinnedItem> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(e, wt, handle, pose)| {
                let material = world
                    .get::<MaterialHandle>(e)
                    .and_then(|h| assets.material(*h).copied())
                    .unwrap_or_else(|| {
                        MaterialUniform::with_params([0.8, 0.8, 0.82, 1.0], 0.0, 0.6)
                    });
                SkinnedItem {
                    model: wt.0,
                    handle: *handle,
                    palette: pose.palette(),
                    material,
                    texture: world.get::<crate::assets::TextureHandle>(e).copied(),
                    pbr: if pbr { world.get::<crate::assets::PbrTextures>(e).copied() } else { None },
                }
            })
            .collect();
        warn_overflow_once(&self.skinned_overflow_warned, "skinned meshes", items.len(), MAX_SKINNED);
        let n = items.len().min(MAX_SKINNED);
        if n == 0 {
            return;
        }

        // Palettes are shared by both skinned paths.
        let pal_aligned = self.palette_aligned as usize;
        let mut pal_staging = vec![0u8; n * pal_aligned];
        for (i, it) in items.iter().take(n).enumerate() {
            let pb = bytemuck::bytes_of(&it.palette);
            pal_staging[i * pal_aligned..i * pal_aligned + pb.len()].copy_from_slice(pb);
        }
        queue.write_buffer(&self.palette_buffer, 0, &pal_staging);

        // Per-entity camera uniform: PBR uses CameraUniform (w/ camera_pos), the
        // colored path uses SkinnedCameraUniform.
        let cam_aligned = if self.pbr_enabled {
            let a = self.pbr_skinned_cam_aligned as usize;
            let mut staging = vec![0u8; n * a];
            for (i, it) in items.iter().take(n).enumerate() {
                let cam = CameraUniform {
                    view_proj: view_proj.cols,
                    model: it.model.cols,
                    camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                    _padding: 0.0,
                };
                let cb = bytemuck::bytes_of(&cam);
                staging[i * a..i * a + cb.len()].copy_from_slice(cb);
            }
            queue.write_buffer(&self.pbr_skinned_cam_buffer, 0, &staging);
            a
        } else {
            let a = self.skinned_cam_aligned as usize;
            let mut staging = vec![0u8; n * a];
            for (i, it) in items.iter().take(n).enumerate() {
                let cam = SkinnedCameraUniform {
                    view_proj: view_proj.cols,
                    model: it.model.cols,
                };
                let cb = bytemuck::bytes_of(&cam);
                staging[i * a..i * a + cb.len()].copy_from_slice(cb);
            }
            queue.write_buffer(&self.skinned_cam_buffer, 0, &staging);
            a
        };

        // PBR skinned: stage per-entity material + build per-entity texture
        // bind groups (defaults for absent maps), mirroring the static pass.
        let mat_aligned = self.pbr_skinned_material_aligned as usize;
        let skinned_tex_bgs: Vec<Option<wgpu::BindGroup>> = if self.pbr_enabled {
            let mut mat_staging = vec![0u8; n * mat_aligned];
            for (i, it) in items.iter().take(n).enumerate() {
                let mb = bytemuck::bytes_of(&it.material);
                mat_staging[i * mat_aligned..i * mat_aligned + mb.len()].copy_from_slice(mb);
            }
            queue.write_buffer(&self.pbr_skinned_material_buffer, 0, &mat_staging);

            let resolve = |h: Option<crate::assets::TextureHandle>| {
                h.and_then(|h| assets.texture(h)).map(|t| (&t.view, &t.sampler))
            };
            items
                .iter()
                .take(n)
                .map(|it| {
                    let has_maps = it.texture.is_some()
                        || it.pbr.map(|p| p.normal.or(p.orm).or(p.emissive).is_some())
                            == Some(true);
                    if !has_maps {
                        return None;
                    }
                    let (av, asamp) = resolve(it.texture)
                        .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                    let (nv, nsamp) = resolve(it.pbr.and_then(|p| p.normal))
                        .unwrap_or((&self.pbr_flat_normal.1, &self.pbr_flat_normal.2));
                    let (ov, osamp) = resolve(it.pbr.and_then(|p| p.orm))
                        .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                    let (ev, esamp) = resolve(it.pbr.and_then(|p| p.emissive))
                        .unwrap_or((&self.pbr_black.view, &self.pbr_black.sampler));
                    Some(create_pbr_full_texture_bind_group(
                        &self.device, &self.pbr_tex_layout, av, asamp, nv, nsamp, ov, osamp, ev,
                        esamp,
                    ))
                })
                .collect()
        } else {
            Vec::new()
        };

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

        if self.pbr_enabled {
            pass.set_pipeline(&self.pbr_skinned_pipeline);
            pass.set_bind_group(3, &self.pbr_shadow_bind_group, &[]);
            for (i, it) in items.iter().take(n).enumerate() {
                let Some(mesh) = assets.skinned_mesh(it.handle) else {
                    continue;
                };
                let cam_off = (i * cam_aligned) as u32;
                let pal_off = (i * pal_aligned) as u32;
                let mat_off = (i * mat_aligned) as u32;
                // g0 has two dynamic bindings (camera @0, palette @2).
                pass.set_bind_group(0, &self.pbr_skinned_g0_bind_group, &[cam_off, pal_off]);
                let tex_bg = skinned_tex_bgs[i]
                    .as_ref()
                    .unwrap_or(&self.pbr_default_texture_bind_group);
                pass.set_bind_group(1, tex_bg, &[]);
                pass.set_bind_group(2, &self.pbr_skinned_material_bind_group, &[mat_off]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        } else {
            pass.set_pipeline(&self.skinned_pipeline);
            for (i, it) in items.iter().take(n).enumerate() {
                let Some(mesh) = assets.skinned_mesh(it.handle) else {
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
fn warn_overflow_once(flag: &Cell<bool>, kind: &str, count: usize, cap: usize) {
    if count > cap && !flag.get() {
        flag.set(true);
        eprintln!(
            "voltex_renderer_ecs: {count} {kind} exceed the per-frame cap of {cap}; \
             extra {kind} are not drawn (this warning fires once)."
        );
    }
}

/// Cull to the `max` sprites **nearest** the camera, then sort those
/// back-to-front, both by view depth `(pos - cam_pos)·forward` (not euclidean
/// distance — radial distance flips layering for coplanar sprites under an
/// orthographic camera). Returns the draw count; `items[..n]` is the ordered
/// draw list.
fn cull_and_sort_sprites(
    items: &mut [(Vec3, Sprite)],
    cam_pos: Vec3,
    forward: Vec3,
    max: usize,
) -> usize {
    let depth = |p: Vec3| (p - cam_pos).dot(forward);
    let n = items.len().min(max);
    if items.len() > max {
        // 가까운(depth 작은) n개를 앞으로 — 초과분은 먼 것부터 버린다.
        items.select_nth_unstable_by(n - 1, |a, b| {
            depth(a.0)
                .partial_cmp(&depth(b.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    items[..n].sort_by(|a, b| {
        depth(b.0)
            .partial_cmp(&depth(a.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    n
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
    fn sprite_cull_keeps_nearest_and_sorts_back_to_front() {
        // 카메라 원점, forward -Z: z가 더 음수일수록 멀다? 아니 — depth =
        // (pos-cam)·forward 이므로 forward=(0,0,-1)일 때 z=-5가 depth 5(멀고),
        // z=-1이 depth 1(가깝다).
        let cam = Vec3::ZERO;
        let fwd = Vec3::new(0.0, 0.0, -1.0);
        let at = |z: f32| (Vec3::new(0.0, 0.0, z), Sprite::default());
        // 5개 중 3개만 그릴 수 있음 — 가까운 1,2,3이 살아남고 4,5는 컬링.
        let mut items = vec![at(-4.0), at(-1.0), at(-5.0), at(-3.0), at(-2.0)];
        let n = super::cull_and_sort_sprites(&mut items, cam, fwd, 3);
        assert_eq!(n, 3);
        let depths: Vec<f32> = items[..n].iter().map(|(p, _)| -p.z).collect();
        assert_eq!(depths, vec![3.0, 2.0, 1.0], "back-to-front, 가까운 3개만");
        // cap 미만이면 전량 back-to-front.
        let mut items = vec![at(-1.0), at(-3.0), at(-2.0)];
        let n = super::cull_and_sort_sprites(&mut items, cam, fwd, 256);
        assert_eq!(n, 3);
        let depths: Vec<f32> = items.iter().map(|(p, _)| -p.z).collect();
        assert_eq!(depths, vec![3.0, 2.0, 1.0]);
        // 직교 동일평면 시나리오: z=0 평면, 카메라 (0,4,10) forward -Z.
        // 유클리드 정렬이면 y에 따라 순서가 갈라졌겠지만 depth는 전부 동일 —
        // 순서가 안정적으로 유지되고 패닉 없음.
        let cam = Vec3::new(0.0, 4.0, 10.0);
        let mut items = vec![
            (Vec3::new(0.0, 6.0, 0.0), Sprite::default()),
            (Vec3::new(0.0, 1.0, 0.0), Sprite::default()),
        ];
        let n = super::cull_and_sort_sprites(&mut items, cam, fwd, 256);
        assert_eq!(n, 2);
    }

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
