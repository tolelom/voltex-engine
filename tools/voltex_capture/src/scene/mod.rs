//! Reusable demo scenes. Each returns a tightly-packed RGBA8 buffer so a bin
//! can save a PNG and a (GPU-gated) test can assert the frame isn't blank.
//!
//! 렌더 경로별 구성:
//! - `BasicRenderer` 경로 (런타임과 같은 ECS 렌더러): 이 파일의 기본 큐브 씬과
//!   [`declarative`], [`lighting`], [`pbr`], [`texture`], [`sprite`],
//!   [`hot_reload`], [`playthrough`], [`skinned`] 의 런타임 씬. 모두 `scene::` 로
//!   평평하게 다시 내보낸다.
//! - 손으로 조립한 wgpu PBR 파이프라인: [`raw_pbr`] (`scene::raw_pbr::*`).
//! - 에디터 렌더러: [`gizmo`] (`GizmoRenderer`), [`ui`] (`UiRenderer`).
//! - 스킨드 전용 파이프라인: [`skinned`] 의 `render_skinned_rest`/`render_skinned_bent`.

use voltex_ecs::{Bounds, MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Quat, Vec3, AABB};
use voltex_renderer::{Camera, LightUniform, MaterialUniform, Mesh, MeshVertex};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

mod declarative;
mod gizmo;
mod hot_reload;
mod lighting;
mod pbr;
mod playthrough;
pub mod raw_pbr;
mod skinned;
mod sprite;
mod texture;
mod ui;

pub use declarative::{
    render_boids_swarm, render_data_authored_scene, render_materialized_primitives,
    render_survivor_seed_scene, render_world, render_world_from, render_world_ortho, CameraPose,
    Projection,
};
pub use gizmo::{render_gizmo_rotate, render_gizmo_scale, render_gizmo_translate};
pub use hot_reload::{render_mesh_hot_reload, render_texture_hot_reload};
pub use lighting::{render_sphere_point_lit, render_shadow_demo, render_spot_lit};
pub use pbr::{render_pbr_basic_renderer, render_pbr_sphere_normalmap};
pub use playthrough::{render_demo_frame, render_demo_frame_mode, DemoMoment, DemoShading};
pub use skinned::{
    render_skinned_bent, render_skinned_bent_demo, render_skinned_rest, render_skinned_runtime,
    render_skinned_runtime_mode, render_skinned_shadow_scene,
};
pub use sprite::{render_billboard_sprite, render_particle_burst, render_sprite_sheet_frame};
pub use texture::{
    render_cube_with_png, render_cube_with_texture, render_obj_model, render_textured_cube,
};
pub use ui::{render_editor_ui, render_ui_image_widget};

/// 무광 유전체 머티리얼 (metallic 0, roughness 1) — 색만 보면 되는 씬용.
pub(crate) fn matte(base_color: [f32; 4]) -> MaterialUniform {
    MaterialUniform::with_params(base_color, 0.0, 1.0)
}

/// Up-facing (+Y normal) ground quad on y=0 spanning `±half_extent` on X/Z,
/// wound CCW when seen from above so an overhead light lights it via N·L.
/// Corner UVs run 0..`uv_span` (pass 0.0 when the material has no texture).
pub(crate) fn ground_quad_mesh(device: &wgpu::Device, half_extent: f32, uv_span: f32) -> Mesh {
    let corner = |x: f32, z: f32, u: f32, v: f32| MeshVertex {
        position: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        uv: [u * uv_span, v * uv_span],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    let h = half_extent;
    let vertices = [
        corner(-h, -h, 0.0, 0.0),
        corner(h, -h, 1.0, 0.0),
        corner(h, h, 1.0, 1.0),
        corner(-h, h, 0.0, 1.0),
    ];
    let indices = [0u32, 2, 1, 0, 3, 2];
    Mesh::new(device, &vertices, &indices)
}

/// Build a unit cube mesh (24 verts, 36 indices, outward normals).
pub(crate) fn unit_cube(device: &wgpu::Device) -> Mesh {
    Mesh::cube(device, 1.0)
}

/// What one captured frame looks like: encoder label, background, viewpoint and
/// directional light. The drawn content comes from the world and its assets.
pub(crate) struct FrameSpec<'a> {
    pub label: &'a str,
    pub clear_color: wgpu::Color,
    pub camera: &'a Camera,
    pub light: LightUniform,
}

/// Record one `BasicRenderer::render` into a fresh encoder, submit it and read
/// the target back as RGBA8 (the shared tail of the scene functions below).
pub(crate) fn draw_and_read(
    gpu: &HeadlessGpu,
    target: &OffscreenTarget,
    renderer: &BasicRenderer,
    world: &mut World,
    assets: &RenderAssets,
    frame: FrameSpec,
) -> Vec<u8> {
    let FrameSpec {
        label,
        clear_color,
        camera,
        light,
    } = frame;
    let view_proj = camera.view_projection();
    let cam_pos = camera.position;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    renderer.render(
        &gpu.queue,
        &mut encoder,
        &target.color_view,
        &target.depth_view,
        clear_color,
        world,
        assets,
        view_proj,
        cam_pos,
        light,
    );
    gpu.queue.submit(Some(encoder.finish()));
    target.read_rgba8(&gpu.device, &gpu.queue)
}

/// Render a 3×3 grid of cubes lit by the default directional light, viewed
/// from a slightly elevated angle. Returns RGBA8 (`width*height*4`).
pub fn render_cube_grid(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 64);

    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());
    let bounds = Bounds(AABB::new(
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
    ));

    let mut world = World::new();
    for gx in -1..=1 {
        for gz in -1..=1 {
            let e = world.spawn();
            world.add(
                e,
                Transform::from_position(Vec3::new(gx as f32 * 2.0, 0.0, gz as f32 * 2.0)),
            );
            world.add(e, MeshHandle(mesh_h.0));
            world.add(e, MaterialHandle(mat_h.0));
            world.add(e, bounds);
        }
    }

    // Camera looks down and forward at the grid.
    let mut camera = Camera::new(Vec3::new(0.0, 4.0, 8.0), width as f32 / height as f32);
    camera.pitch = -0.45;
    let light = LightUniform::new();

    draw_and_read(
        gpu,
        &target,
        &renderer,
        &mut world,
        &assets,
        FrameSpec {
            label: "capture cube_grid",
            clear_color: wgpu::Color { r: 0.1, g: 0.12, b: 0.18, a: 1.0 },
            camera: &camera,
            light,
        },
    )
}

/// Render a single large centered cube — the simplest "did anything draw"
/// smoke scene. Returns RGBA8.
pub fn render_single_cube(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);

    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));

    // Corner-on view so three faces are visible.
    let mut camera = Camera::new(Vec3::new(2.2, 1.8, 2.6), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.45;

    // Asymmetric light so each face receives a distinct diffuse term
    // (the default (0,-1,-1) lights +Y and +Z identically).
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        &mut world,
        &assets,
        FrameSpec {
            label: "capture single_cube",
            clear_color: wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
            camera: &camera,
            light,
        },
    )
}

/// Render one white cube at world `depth` (more negative = farther from the
/// camera at z=8) under either an orthographic or perspective projection.
/// Returns RGBA8. Used to show that orthographic footprints are
/// depth-independent while perspective foreshortens.
pub fn render_cube_at_depth(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    ortho: bool,
    depth: f32,
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let cube = assets.add_mesh(unit_cube(&gpu.device));
    let mat = assets.add_material(matte([1.0, 1.0, 1.0, 1.0]));

    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(0.0, 0.0, depth)));
    world.add(e, MeshHandle(cube.0));
    world.add(e, MaterialHandle(mat.0));
    voltex_ecs::propagate_transforms(&mut world);

    let aspect = width as f32 / height as f32;
    let camera = if ortho {
        Camera::orthographic(Vec3::new(0.0, 0.0, 8.0), aspect, 10.0)
    } else {
        Camera::new(Vec3::new(0.0, 0.0, 8.0), aspect)
    };
    let mut light = LightUniform::new();
    light.direction = [0.2, -0.3, -1.0];
    light.ambient_strength = 0.3;

    draw_and_read(
        gpu,
        &target,
        &renderer,
        &mut world,
        &assets,
        FrameSpec {
            label: "capture proj",
            clear_color: wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            camera: &camera,
            light,
        },
    )
}

/// Render only the given debug line segments (no meshes) on a black background,
/// viewed by a fixed front camera. Exercises `BasicRenderer::set_debug_lines` +
/// the line pipeline. Returns RGBA8.
pub fn render_debug_lines(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    verts: &[voltex_renderer::LineVertex],
) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let assets = RenderAssets::new();
    let mut world = World::new();

    let camera = Camera::new(Vec3::new(0.0, 0.0, 4.0), width as f32 / height as f32);
    let light = LightUniform::new();

    renderer.set_debug_lines(&gpu.queue, verts);
    draw_and_read(
        gpu,
        &target,
        &renderer,
        &mut world,
        &assets,
        FrameSpec {
            label: "capture lines",
            clear_color: wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            camera: &camera,
            light,
        },
    )
}

#[cfg(test)]
mod tests;
