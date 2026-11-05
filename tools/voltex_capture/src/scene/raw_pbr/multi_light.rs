//! Multi-light capture — the PBR pipeline driven by `LightsUniform` with
//! several point/spot/directional lights at once. The visual check is that
//! distinct colored point lights produce distinct colored illumination on a
//! neutral ground + spheres (correct per-light contribution + attenuation).

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{generate_sphere_mesh, Camera, LightData, LightsUniform, MaterialUniform};

use super::{
    begin_color_pass, draw_mesh, shadow_uniform, RawPbrObject, RawPbrResources, SceneMeshes,
    Shape,
};
use crate::headless::HeadlessGpu;
use crate::scene::unit_cube;
use crate::target::OffscreenTarget;

const NUM_OBJECTS: usize = 6; // ground + 5 spheres

/// Ground slab (cube) first, then the five spheres.
fn scene_objects() -> [(Shape, Mat4); NUM_OBJECTS] {
    [
        (
            Shape::Cube,
            Mat4::translation(0.0, -0.5, 0.0).mul_mat4(&Mat4::scale(10.0, 0.1, 10.0)), // ground
        ),
        (Shape::Sphere, Mat4::translation(0.0, 0.0, 0.0)),
        (Shape::Sphere, Mat4::translation(3.0, 0.0, 0.0)),
        (Shape::Sphere, Mat4::translation(-3.0, 0.0, 0.0)),
        (Shape::Sphere, Mat4::translation(0.0, 0.0, 3.0)),
        (Shape::Sphere, Mat4::translation(0.0, 0.0, -3.0)),
    ]
}

/// Render a neutral ground + 5 spheres lit by a dim directional plus four
/// colored point lights (red/green/blue/yellow) and a white overhead spot.
/// Returns RGBA8.
pub fn render_multi_light(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let meshes = SceneMeshes {
        cube: unit_cube(device),
        sphere: generate_sphere_mesh(0.5, 32, 16).upload(device),
    };

    let mut camera = Camera::new(Vec3::new(0.0, 5.0, 12.0), width as f32 / height as f32);
    camera.pitch = -0.3;
    let view_proj = camera.view_projection();
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];

    let mut lights = LightsUniform::new();
    lights.ambient_color = [0.02, 0.02, 0.02];
    lights.add_light(LightData::directional(
        [0.0, -1.0, -0.5],
        [1.0, 1.0, 1.0],
        0.3,
    ));
    lights.add_light(LightData::point(
        [5.0, 2.0, 0.0],
        [1.0, 0.0, 0.0],
        15.0,
        15.0,
    ));
    lights.add_light(LightData::point(
        [0.0, 2.0, 5.0],
        [0.0, 1.0, 0.0],
        15.0,
        15.0,
    ));
    lights.add_light(LightData::point(
        [-5.0, 2.0, 0.0],
        [0.0, 0.0, 1.0],
        15.0,
        15.0,
    ));
    lights.add_light(LightData::point(
        [0.0, 2.0, -5.0],
        [1.0, 1.0, 0.0],
        15.0,
        15.0,
    ));
    lights.add_light(LightData::spot(
        [0.0, 6.0, 0.0],
        [0.0, -1.0, 0.0],
        [1.0, 1.0, 1.0],
        20.0,
        12.0,
        20.0,
        35.0,
    ));

    // 그림자 없음 (shadow_map_size 0).
    let resources = RawPbrResources::new(
        device,
        queue,
        "ML",
        NUM_OBJECTS,
        &lights,
        &shadow_uniform(Mat4::IDENTITY.cols, 0.0, 0.0),
    );

    // Neutral mid-gray, fairly rough dielectric so colored lights read clearly.
    let objects = scene_objects();
    let draws: Vec<RawPbrObject> = objects
        .iter()
        .map(|&(_, model)| RawPbrObject {
            model,
            material: MaterialUniform::with_params([0.8, 0.8, 0.8, 1.0], 0.0, 0.6),
        })
        .collect();
    resources.write_objects(queue, view_proj, cam_pos, &draws);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Multi-light capture"),
    });
    {
        let clear = wgpu::Color {
            r: 0.02,
            g: 0.02,
            b: 0.03,
            a: 1.0,
        };
        let mut pass = begin_color_pass(&mut encoder, &target, "ML Pass", clear);
        resources.bind_shared(&mut pass);
        for (i, &(shape, _)) in objects.iter().enumerate() {
            resources.bind_object(&mut pass, i);
            draw_mesh(&mut pass, meshes.get(shape));
        }
    }
    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    #[test]
    #[ignore]
    fn multi_light_renders_color() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_multi_light(&gpu, 640, 480);
        let stats = ImageStats::analyze(&rgba);
        assert!(!stats.looks_blank(), "multi-light blank: {stats:?}");
    }
}
