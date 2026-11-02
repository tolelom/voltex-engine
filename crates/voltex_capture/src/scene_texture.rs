//! Texture and model loading scenes: PNG/GPU textures on a cube, per-entity
//! textures through `BasicRenderer`, and an OBJ model.

use voltex_ecs::{MaterialHandle, MeshHandle, Transform, World};
use voltex_math::{Quat, Vec3};
use voltex_renderer::{Camera, LightUniform, MaterialUniform};
use voltex_renderer_ecs::{BasicRenderer, RenderAssets};

use crate::headless::HeadlessGpu;
use crate::scene::{draw_and_read, unit_cube};
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render a single cube with a pre-built [`GpuTexture`](voltex_renderer::GpuTexture) via a runtime
/// `TextureHandle` + `BasicRenderer`. Proves per-entity textures are sampled.
pub fn render_cube_with_texture(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    tex: voltex_renderer::GpuTexture,
) -> Vec<u8> {
    use voltex_renderer_ecs::TextureHandle;

    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new()); // white → texture shows through
    let tex_h: TextureHandle = assets.add_texture(tex);

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

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture tex",
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        LightUniform::new(),
    )
}

/// Render a single cube whose diffuse texture is decoded from **PNG bytes**
/// via [`RenderAssets::add_texture_png`] — the exact path the editor asset
/// pipeline uses when a `.png` is opened from the browser (file bytes →
/// decode → `TextureHandle` → `BasicRenderer`). Distinct from
/// [`render_cube_with_texture`], which takes a pre-built `GpuTexture` and so
/// skips the PNG decode + sRGB upload. Returns RGBA8, or `Err` if decode fails.
pub fn render_cube_with_png(
    gpu: &HeadlessGpu,
    width: u32,
    height: u32,
    png_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(unit_cube(&gpu.device));
    let mat_h = assets.add_material(MaterialUniform::new()); // white → texture shows through
    let tex_h = assets
        .add_texture_png(&gpu.device, &gpu.queue, png_bytes)
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

    Ok(draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture png tex",
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        LightUniform::new(),
    ))
}

/// Render a cube textured with a 2×2 solid `color` (RGBA).
pub fn render_textured_cube(gpu: &HeadlessGpu, width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    use voltex_renderer::GpuTexture;
    let layout = GpuTexture::bind_group_layout(&gpu.device);
    let pixels: Vec<u8> = color.iter().cycle().take(2 * 2 * 4).copied().collect();
    let tex = GpuTexture::from_rgba(&gpu.device, &gpu.queue, 2, 2, &pixels, &layout);
    render_cube_with_texture(gpu, width, height, tex)
}

/// Load a Wavefront OBJ source via `Mesh::from_obj` and render it as a single
/// centered model through `BasicRenderer`. Returns RGBA8.
pub fn render_obj_model(gpu: &HeadlessGpu, width: u32, height: u32, obj_src: &str) -> Vec<u8> {
    let target = OffscreenTarget::new(&gpu.device, width, height);
    let renderer = BasicRenderer::new(&gpu.device, &gpu.queue, CAPTURE_FORMAT, 8);
    let mut assets = RenderAssets::new();
    let mesh_h = assets.add_mesh(voltex_renderer::Mesh::from_obj(&gpu.device, obj_src));
    let mat_h = assets.add_material(MaterialUniform::with_params([0.6, 0.7, 0.9, 1.0], 0.0, 0.6));

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
    let mut light = LightUniform::new();
    light.direction = [-0.4, -1.0, -0.6];

    draw_and_read(
        gpu,
        &target,
        &renderer,
        "capture obj",
        wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 },
        &mut world,
        &assets,
        camera.view_projection(),
        camera.position,
        light,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    /// The repo's `assets/cube.obj`, embedded for the loader regression.
    const CUBE_OBJ: &str = include_str!("../../../assets/cube.obj");

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn basic_renderer_samples_per_entity_texture() {
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
            let mut r = 0u64;
            let mut b = 0u64;
            let mut n = 0u64;
            for p in rgba.as_chunks::<4>().0.iter() {
                if (p[0] as i32 - bg[0] as i32).abs() > 20
                    || (p[2] as i32 - bg[2] as i32).abs() > 20
                {
                    r += p[0] as u64;
                    b += p[2] as u64;
                    n += 1;
                }
            }
            let n = n.max(1) as f64;
            (r as f64 / n, b as f64 / n)
        }

        let red = render_textured_cube(&gpu, 256, 256, [220, 30, 30, 255]);
        let blue = render_textured_cube(&gpu, 256, 256, [30, 30, 220, 255]);
        let (rr, rb) = mean_rb(&red);
        let (br, bb) = mean_rb(&blue);
        assert!(rr > rb, "red-textured cube should be red-dominant: R={rr} B={rb}");
        assert!(bb > br, "blue-textured cube should be blue-dominant: R={br} B={bb}");
    }

    /// Editor asset-pipeline regression: opening a `.png` from the browser
    /// loads it via `RenderAssets::add_texture_png` (decode + sRGB upload) and
    /// attaches a `TextureHandle`. This guards that exact decode path renders —
    /// the part that could silently fail where the solid-RGBA test above can't.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn editor_pipeline_loaded_png_texture_renders() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };

        // 2×2 magenta/green checker → encode to PNG (the bytes a browser would read).
        let m = [220u8, 30, 220, 255]; // magenta
        let g = [30u8, 200, 60, 255]; // green
        let pixels: Vec<u8> = [m, g, g, m].concat();
        let png = crate::encode_rgba8(2, 2, &pixels);

        let rgba = render_cube_with_png(&gpu, 256, 256, &png)
            .expect("PNG-textured cube should render");

        // Non-blank, and both checker colors must be present on the cube.
        assert!(!ImageStats::analyze(&rgba).looks_blank(), "textured cube frame is blank");
        let bg = [rgba[0], rgba[1], rgba[2]];
        let (mut magenta_px, mut green_px) = (0u32, 0u32);
        for p in rgba.as_chunks::<4>().0.iter() {
            let off_bg = (p[0] as i32 - bg[0] as i32).abs()
                + (p[1] as i32 - bg[1] as i32).abs()
                + (p[2] as i32 - bg[2] as i32).abs()
                > 40;
            if !off_bg {
                continue;
            }
            if p[0] > 120 && p[2] > 120 && p[1] < 120 {
                magenta_px += 1;
            } else if p[1] > 120 && p[0] < 120 {
                green_px += 1;
            }
        }
        assert!(magenta_px > 50, "expected magenta checker texels, got {magenta_px}");
        assert!(green_px > 50, "expected green checker texels, got {green_px}");

        // Persist a visual artifact for manual inspection.
        crate::write_visual_record("textured_entity.png", 256, 256, &rgba);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn loads_png_into_texture_and_renders() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        // Encode a 4×4 red image to PNG, decode via GpuTexture::from_png, render.
        let red: Vec<u8> = [200u8, 40, 40, 255].iter().cycle().take(4 * 4 * 4).copied().collect();
        let png = voltex_image::png_encode::encode_rgba8(4, 4, &red);
        let layout = voltex_renderer::GpuTexture::bind_group_layout(&gpu.device);
        let tex = voltex_renderer::GpuTexture::from_png(&gpu.device, &gpu.queue, &png, &layout)
            .expect("decode PNG");
        let rgba = render_cube_with_texture(&gpu, 256, 256, tex);

        // The cube should be red-dominant (texture sampled through the runtime).
        let bg = [rgba[0], rgba[1], rgba[2]];
        let (mut r, mut b, mut n) = (0u64, 0u64, 0u64);
        for p in rgba.as_chunks::<4>().0.iter() {
            if (p[0] as i32 - bg[0] as i32).abs() > 20 || (p[2] as i32 - bg[2] as i32).abs() > 20 {
                r += p[0] as u64;
                b += p[2] as u64;
                n += 1;
            }
        }
        let n = n.max(1);
        assert!(r / n > b / n, "PNG-textured cube should be red-dominant: R={} B={}", r / n, b / n);
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn loads_and_renders_obj_model() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skip: {e}");
                return;
            }
        };
        let rgba = render_obj_model(&gpu, 256, 256, CUBE_OBJ);
        let stats = ImageStats::analyze(&rgba);
        assert!(
            !stats.looks_blank(),
            "loaded OBJ model should render (not blank): {stats:?}"
        );
    }
}
