//! The canonical PBR material test: a sphere grid sweeping metallic and
//! roughness.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{generate_sphere, Camera, LightData, LightsUniform, MaterialUniform, Mesh};

use super::{begin_color_pass, draw_mesh, shadow_uniform, RawPbrObject, RawPbrResources};
use crate::headless::HeadlessGpu;
use crate::target::OffscreenTarget;

/// Render the canonical PBR test: a 7×7 grid of spheres with metallic varying
/// left→right and roughness varying bottom→top, lit by one directional light.
/// Exercises the full Cook-Torrance pipeline (`create_pbr_pipeline`) plus the
/// IBL / shadow bind groups. Returns RGBA8.
pub fn render_pbr_grid(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    const GRID: usize = 7;
    const SPHERE_COUNT: usize = GRID * GRID;
    const SPACING: f32 = 1.2;

    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let (sphere_vertices, sphere_indices) = generate_sphere(0.4, 32, 16);
    let mesh = Mesh::new(device, &sphere_vertices, &sphere_indices);

    let camera = Camera::new(Vec3::new(0.0, 0.0, 12.0), width as f32 / height as f32);
    let view_proj = camera.view_projection();
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];

    let mut lights = LightsUniform::new();
    lights.add_light(LightData::directional(
        [-1.0, -1.0, -1.0],
        [1.0, 1.0, 1.0],
        1.0,
    ));

    // 그림자 없음 (shadow_map_size 0).
    let resources = RawPbrResources::new(
        device,
        queue,
        "PBR",
        SPHERE_COUNT,
        &lights,
        &shadow_uniform([[0.0; 4]; 4], 0.0, 0.0),
    );

    // Row-major: object index = row * GRID + col.
    let half_grid = (GRID as f32 - 1.0) * SPACING * 0.5;
    let mut spheres = Vec::with_capacity(SPHERE_COUNT);
    for row in 0..GRID {
        for col in 0..GRID {
            let x = col as f32 * SPACING - half_grid;
            let y = row as f32 * SPACING - half_grid;
            let metallic = col as f32 / (GRID as f32 - 1.0);
            let roughness = 0.05 + row as f32 * (0.95 / (GRID as f32 - 1.0));
            spheres.push(RawPbrObject {
                model: Mat4::translation(x, y, 0.0),
                material: MaterialUniform::with_params([0.8, 0.2, 0.2, 1.0], metallic, roughness),
            });
        }
    }
    resources.write_objects(queue, view_proj, cam_pos, &spheres);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("PBR capture"),
    });
    {
        let clear = wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.15,
            a: 1.0,
        };
        let mut pass = begin_color_pass(&mut encoder, &target, "PBR Pass", clear);
        resources.bind_shared(&mut pass);
        for i in 0..SPHERE_COUNT {
            resources.bind_object(&mut pass, i);
            draw_mesh(&mut pass, &mesh);
        }
    }
    queue.submit(Some(encoder.finish()));

    target.read_rgba8(device, queue)
}
