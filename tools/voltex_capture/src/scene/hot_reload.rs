//! Hot-reload scenes: a texture and a mesh swapped between two rendered frames.

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Quat, Vec3};
use voltex_renderer::{Camera, LightUniform, MaterialUniform, Mesh};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use super::{draw_and_read, unit_cube};
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Hot-reload regression helper. Renders one cube textured from `png_a`, then
/// replaces the texture **in place** behind the same `TextureHandle` with
/// `png_b` (no re-attach to the entity) and renders again. Returns
/// `(frame_a, frame_b)`. Proves `replace_texture_png` updates what an entity
/// samples without touching the entity or its handle.
pub fn render_texture_hot_reload(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    png_a: &[u8],
    png_b: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new());
    let tex_h = assets
        .add_texture_png(&gpu.device, &gpu.queue, png_a)
        .map_err(|e| format!("add_texture_png failed: {e}"))?;

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));
    world.add(e, tex_h);
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.4;

    let render_once = |assets: &RenderAssets, world: &mut World| -> Vec<u8> {
        draw_and_read(
            gpu,
            &target,
            &renderer,
            "hot reload",
            wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
            world,
            assets,
            camera.view_projection(),
            camera.position,
            LightUniform::new(),
        )
    };

    let frame_a = render_once(&assets, &mut world);

    // Hot-reload: swap the texture data at the SAME handle. The entity still
    // holds `tex_h` — no re-attach.
    assets
        .replace_texture_png(&gpu.device, &gpu.queue, tex_h, png_b)
        .map_err(|e| format!("replace_texture_png failed: {e}"))?;

    let frame_b = render_once(&assets, &mut world);
    Ok((frame_a, frame_b))
}

/// Mesh hot-reload regression helper. Renders one small cube, then replaces
/// the `Mesh` **in place** behind the same `MeshHandle` with a much larger
/// cube (no entity re-attach) and renders again. Returns `(frame_a, frame_b)`.
/// Proves `replace_mesh` updates the geometry an entity draws without
/// touching the entity or its handle.
pub fn render_mesh_hot_reload(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(Mesh::cube(&gpu.device, 0.6));
    let mat_h = assets.add_material(MaterialUniform::with_params([0.8, 0.3, 0.3, 1.0], 0.0, 0.8));

    let mut world = World::new();
    let e = world.spawn();
    let mut t = Transform::from_position(Vec3::ZERO);
    t.rotation = Quat::from_euler_yxz(Vec3::new(0.3, 0.6, 0.0));
    world.add(e, t);
    world.add(e, MeshHandle(mesh_h.0));
    world.add(e, MaterialHandle(mat_h.0));
    voltex_ecs::propagate_transforms(&mut world);

    let mut camera = Camera::new(Vec3::new(2.0, 1.5, 2.5), width as f32 / height as f32);
    camera.yaw = -0.7;
    camera.pitch = -0.4;

    let render_once = |assets: &RenderAssets, world: &mut World| -> Vec<u8> {
        draw_and_read(
            gpu,
            &target,
            &renderer,
            "mesh hot reload",
            wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
            world,
            assets,
            camera.view_projection(),
            camera.position,
            LightUniform::new(),
        )
    };

    let frame_a = render_once(&assets, &mut world);

    // Hot-reload: swap the geometry at the SAME handle. The entity still
    // holds `mesh_h` — no re-attach.
    assets
        .replace_mesh(mesh_h, Mesh::cube(&gpu.device, 2.0))
        .map_err(|e| format!("replace_mesh failed: {e}"))?;

    let frame_b = render_once(&assets, &mut world);
    Ok((frame_a, frame_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asset hot-reload regression: replacing a texture in place behind an
    /// existing `TextureHandle` (no entity re-attach) changes what renders.
    /// Mirrors the editor flow where an applied `.png` is edited on disk.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn texture_hot_reload_swaps_rendered_pixels() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };

        // mean (R, B) over non-background pixels.
        fn mean_rb(rgba: &[u8]) -> (f64, f64) {
            let bg = [rgba[0], rgba[1], rgba[2]];
            let (mut r, mut b, mut n) = (0u64, 0u64, 0u64);
            for p in rgba.as_chunks::<4>().0.iter() {
                if (p[0] as i32 - bg[0] as i32).abs() > 20 || (p[2] as i32 - bg[2] as i32).abs() > 20 {
                    r += p[0] as u64;
                    b += p[2] as u64;
                    n += 1;
                }
            }
            let n = n.max(1) as f64;
            (r as f64 / n, b as f64 / n)
        }

        let red = crate::encode_rgba8(2, 2, &[[220u8, 30, 30, 255]; 4].concat());
        let blue = crate::encode_rgba8(2, 2, &[[30u8, 30, 220, 255]; 4].concat());

        let (frame_a, frame_b) =
            render_texture_hot_reload(&gpu, 256, 256, &red, &blue).expect("hot reload render");

        let (ar, ab) = mean_rb(&frame_a);
        let (br, bb) = mean_rb(&frame_b);
        // Before reload: red-dominant. After in-place swap: blue-dominant.
        assert!(ar > ab, "pre-reload frame should be red-dominant: R={ar} B={ab}");
        assert!(bb > br, "post-reload frame should be blue-dominant: R={br} B={bb}");
    }

    /// Mesh hot-reload regression: replacing the geometry in place behind an
    /// existing `MeshHandle` (no entity re-attach) changes the rendered
    /// silhouette. Mirrors the editor flow where an opened `.obj` is edited
    /// on disk.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn mesh_hot_reload_swaps_rendered_geometry() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };

        // Count pixels that differ from the clear color (top-left corner is
        // guaranteed background — both cubes are centered).
        fn coverage(rgba: &[u8]) -> usize {
            let bg = [rgba[0], rgba[1], rgba[2]];
            rgba.as_chunks::<4>().0.iter()
                .filter(|p| {
                    (p[0] as i32 - bg[0] as i32).abs() > 20
                        || (p[1] as i32 - bg[1] as i32).abs() > 20
                        || (p[2] as i32 - bg[2] as i32).abs() > 20
                })
                .count()
        }

        let (frame_a, frame_b) =
            render_mesh_hot_reload(&gpu, 256, 256).expect("mesh hot reload render");

        let (cov_a, cov_b) = (coverage(&frame_a), coverage(&frame_b));
        assert!(cov_a > 0, "small cube must be visible: coverage={cov_a}");
        assert!(
            cov_b > cov_a * 2,
            "post-reload silhouette must grow substantially (same handle, new geometry): \
             before={cov_a} after={cov_b}"
        );
    }
}
