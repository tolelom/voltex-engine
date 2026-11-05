//! BasicRenderer — mesh_colored.wgsl(Blinn-Phong) 기반 ECS-driven 렌더러.
//!
//! pipeline + per-entity dynamic UBO + light buffer + 1×1 default texture
//! 를 모두 owning. `render(...)` 한 메서드로 World 를 그린다.
//!
//! 한 프레임의 패스 순서 (`render`):
//! 1. 그림자 깊이 전처리 (`shadow.rs`, 켜졌을 때만)
//! 2. 메시 패스 — Blinn-Phong (`blinn_phong.rs`) 또는 PBR (`pbr.rs`). 타깃을 지운다.
//! 3. 스킨드 메시 (`skinned.rs`)
//! 4. 타일맵 → 스프라이트 → 디버그 라인 (crate 의 `*_pass.rs`)

use voltex_ecs::World;
use voltex_math::{Mat4, Vec3, Vec4, AABB};
use voltex_renderer::{GpuTexture, LightUniform, LineVertex, PointLight, PointLightsUniform};

use std::cell::{Cell, RefCell};

mod blinn_phong;
mod pbr;
mod shadow;
mod skinned;

use blinn_phong::BlinnPhongResources;
use pbr::{PbrResources, PbrSkinnedResources};
use shadow::ShadowResources;
use skinned::SkinnedResources;

use crate::assets::RenderAssets;
use crate::items::collect_render_items;

/// Max skinned entities drawn per frame. Each needs an 8 KB bone-palette slot,
/// so this is kept small + independent of `max_entities` to bound VRAM.
const MAX_SKINNED: usize = 64;

/// 그림자 맵 깊이 비교 bias — 표면이 자기 그림자를 받는 shadow acne 를 막는다.
const SHADOW_BIAS: f32 = 0.003;

/// World-driven mesh renderer — Blinn-Phong shader, per-entity dynamic UBO,
/// camera + light shared bind group. Owns the wgpu pipeline objects.
pub struct BasicRenderer {
    // ── Default Blinn-Phong mesh path (camera/light/point lights) ──────────
    blinn_phong: BlinnPhongResources,
    // 1×1 white albedo for entities without a texture (also the white ORM
    // default of the PBR path).
    default_texture: GpuTexture,
    max_entities: usize,
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
    /// Batched `Tilemap` pass (drawn before sprites).
    tilemap_pass: crate::tilemap_pass::TilemapPass,
    // ── Full PBR mesh path (opt-in via set_pbr_enabled) ─────────────────────
    pbr_enabled: bool,
    pbr: PbrResources,
    // ── Skinned PBR (used when both pbr_enabled and skinned entities exist) ──
    pbr_skinned: PbrSkinnedResources,
    // Last-set point lights, kept CPU-side so the PBR pass can fold them into
    // the multi-light `LightsUniform` (the Blinn-Phong pass uses the GPU buffer).
    point_lights_cpu: RefCell<Vec<PointLight>>,
    // Device handle (cheap Arc clone) so the PBR pass can build per-entity
    // 4-map texture bind groups, which need the device at build time.
    device: wgpu::Device,
    // One-time overflow warnings (so silent per-frame caps don't read as
    // "everything drew"). Set the first time each cap is exceeded.
    mesh_overflow_warned: Cell<bool>,
    skinned_overflow_warned: Cell<bool>,
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
        let texture_layout = GpuTexture::bind_group_layout(device);
        let default_texture = GpuTexture::white_1x1(device, queue, &texture_layout);

        let shadow = ShadowResources::new(device, queue, alignment, max_entities);
        let blinn_phong = BlinnPhongResources::new(
            device,
            queue,
            target_format,
            alignment,
            max_entities,
            &texture_layout,
            &shadow.main_layout,
        );
        let skinned = SkinnedResources::new(device, target_format, alignment);
        let pbr = PbrResources::new(
            device,
            queue,
            target_format,
            alignment,
            max_entities,
            &texture_layout,
            &default_texture,
            &shadow.map,
        );
        let pbr_skinned =
            PbrSkinnedResources::new(device, target_format, alignment, &pbr, &skinned.palettes);

        let tilemap_pass =
            crate::tilemap_pass::TilemapPass::new(device, queue, target_format, &texture_layout);
        let sprite_pass = crate::sprite_pass::SpritePass::new(
            device,
            queue,
            target_format,
            &texture_layout,
            alignment,
        );
        let debug_line_pass = crate::debug_line_pass::DebugLinePass::new(device, target_format);

        Self {
            blinn_phong,
            default_texture,
            max_entities,
            skinned,
            shadows_enabled: false,
            shadow,
            debug_line_pass,
            sprite_pass,
            tilemap_pass,
            pbr_enabled: false,
            pbr,
            pbr_skinned,
            point_lights_cpu: RefCell::new(Vec::new()),
            device: device.clone(),
            mesh_overflow_warned: Cell::new(false),
            skinned_overflow_warned: Cell::new(false),
        }
    }

    /// Enable the full PBR mesh path (Cook-Torrance + multi-light + material
    /// maps + IBL) instead of the default Blinn-Phong pass. Off by default; the
    /// Blinn-Phong path is byte-identical when disabled. Sprites, particles,
    /// debug lines and shadow passes are unaffected; skinned meshes switch to
    /// skinned PBR too.
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
        queue.write_buffer(
            &self.blinn_phong.point_lights_buffer,
            0,
            bytemuck::bytes_of(&uniform),
        );
        // Keep a CPU copy so the PBR pass can fold them into its multi-light
        // LightsUniform (the Blinn-Phong pass reads only the GPU buffer above).
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
        items.truncate(self.max_entities);

        self.shadow_prepass(queue, encoder, world, assets, &items, light);

        if self.pbr_enabled {
            self.pbr_mesh_pass(
                queue, encoder, target_view, depth_view, clear_color, &items, view_proj,
                cam_pos, light, assets,
            );
        } else {
            self.blinn_phong_mesh_pass(
                queue, encoder, target_view, depth_view, clear_color, &items, view_proj,
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

/// A bind group with one uniform buffer at binding 0.
fn single_buffer_bind_group(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    resource: wgpu::BindingResource<'_>,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource,
        }],
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
