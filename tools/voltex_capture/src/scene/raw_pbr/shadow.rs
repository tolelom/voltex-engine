//! Shadow-mapping capture — the two-pass directional shadow path
//! (`create_shadow_pipeline` depth pass + `create_pbr_pipeline` sampling the
//! shadow map). Shadows are a classic source of subtle bugs (acne,
//! peter-panning, wrong light projection), so a visual artifact is the only
//! reliable check.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_shadow_pipeline, generate_sphere_mesh, shadow_pass_bind_group_layout, Camera, LightData,
    LightsUniform, MaterialUniform, ShadowPassUniform, SHADOW_MAP_SIZE,
};

use super::{
    begin_color_pass, draw_mesh, shadow_uniform, uniform_stride, write_slot, RawPbrObject,
    RawPbrResources, SceneMeshes, Shape,
};
use crate::headless::HeadlessGpu;
use crate::scene::unit_cube;
use crate::target::OffscreenTarget;

const NUM_OBJECTS: usize = 6;

/// One object of the shadow scene.
struct ShadowSceneObject {
    shape: Shape,
    model: Mat4,
    base_color: [f32; 4],
    metallic: f32,
    roughness: f32,
}

/// Ground slab + 3 spheres + 2 cubes.
fn scene_objects() -> [ShadowSceneObject; NUM_OBJECTS] {
    let object = |shape, model, base_color, metallic, roughness| ShadowSceneObject {
        shape,
        model,
        base_color,
        metallic,
        roughness,
    };
    [
        object(
            Shape::Cube,
            Mat4::translation(0.0, -0.5, 0.0).mul_mat4(&Mat4::scale(15.0, 0.1, 15.0)), // ground
            [0.7, 0.7, 0.7, 1.0],
            0.0,
            0.8,
        ),
        object(Shape::Sphere, Mat4::translation(-3.0, 1.0, 0.0), [0.9, 0.2, 0.2, 1.0], 0.3, 0.4),
        object(Shape::Sphere, Mat4::translation(0.0, 1.5, 0.0), [0.2, 0.9, 0.2, 1.0], 0.5, 0.3),
        object(Shape::Sphere, Mat4::translation(3.0, 0.8, 0.0), [0.2, 0.2, 0.9, 1.0], 0.1, 0.6),
        object(Shape::Cube, Mat4::translation(-1.5, 0.5, -2.0), [0.9, 0.8, 0.2, 1.0], 0.7, 0.2),
        object(Shape::Cube, Mat4::translation(1.5, 0.5, 2.0), [0.8, 0.3, 0.8, 1.0], 0.2, 0.5),
    ]
}

/// Render the 6-object shadow scene (ground + 3 spheres + 2 cubes) lit by one
/// directional light, with a real shadow depth pre-pass. Returns RGBA8.
pub fn render_shadow_scene(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let meshes = SceneMeshes {
        cube: unit_cube(device),
        sphere: generate_sphere_mesh(0.5, 32, 16).upload(device),
    };

    // Camera at (8,8,12) looking at origin.
    let mut camera = Camera::new(Vec3::new(8.0, 8.0, 12.0), width as f32 / height as f32);
    camera.pitch = -0.4;
    let to_origin = Vec3::ZERO - camera.position;
    camera.yaw = to_origin.x.atan2(-to_origin.z);
    let view_proj = camera.view_projection();
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];

    let mut lights = LightsUniform::new();
    lights.ambient_color = [0.05, 0.05, 0.05];
    lights.add_light(LightData::directional(
        [-1.0, -2.0, -1.0],
        [1.0, 1.0, 1.0],
        2.0,
    ));

    // Light view-projection (directional, orthographic).
    let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
    let light_pos = Vec3::ZERO - light_dir * 20.0;
    let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
    let light_vp = light_proj * light_view;

    let resources = RawPbrResources::new(
        device,
        queue,
        "Shadow",
        NUM_OBJECTS,
        &lights,
        &shadow_uniform(light_vp.cols, SHADOW_MAP_SIZE as f32, 0.005),
    );

    // Depth pre-pass resources: one light_vp × model slot per object.
    let shadow_pass_stride = uniform_stride::<ShadowPassUniform>(device);
    let shadow_pass_layout = shadow_pass_bind_group_layout(device);
    let shadow_pass_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Pass UBO"),
        size: (shadow_pass_stride as usize * NUM_OBJECTS) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shadow_pass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Pass BG"),
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
    let shadow_pipeline = create_shadow_pipeline(device, &shadow_pass_layout);

    let objects = scene_objects();
    let mut shadow_pass_staging = vec![0u8; shadow_pass_stride as usize * NUM_OBJECTS];
    for (i, object) in objects.iter().enumerate() {
        let uniform = ShadowPassUniform {
            light_vp_model: (light_vp * object.model).cols,
        };
        write_slot(&mut shadow_pass_staging, shadow_pass_stride, i, &uniform);
    }
    queue.write_buffer(&shadow_pass_buffer, 0, &shadow_pass_staging);
    let draws: Vec<RawPbrObject> = objects
        .iter()
        .map(|object| RawPbrObject {
            model: object.model,
            material: MaterialUniform::with_params(
                object.base_color,
                object.metallic,
                object.roughness,
            ),
        })
        .collect();
    resources.write_objects(queue, view_proj, cam_pos, &draws);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Shadow capture"),
    });

    // Pass 1: shadow depth.
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &resources.shadow_map.view,
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
        pass.set_pipeline(&shadow_pipeline);
        for (i, object) in objects.iter().enumerate() {
            pass.set_bind_group(0, &shadow_pass_bind_group, &[(i as u32) * shadow_pass_stride]);
            draw_mesh(&mut pass, meshes.get(object.shape));
        }
    }

    // Pass 2: PBR color sampling the shadow map.
    {
        let clear = wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.15,
            a: 1.0,
        };
        let mut pass = begin_color_pass(&mut encoder, &target, "Color Pass", clear);
        resources.bind_shared(&mut pass);
        for (i, object) in objects.iter().enumerate() {
            resources.bind_object(&mut pass, i);
            draw_mesh(&mut pass, meshes.get(object.shape));
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
    fn shadow_scene_renders() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_shadow_scene(&gpu, 640, 480);
        let stats = ImageStats::analyze(&rgba);
        assert!(!stats.looks_blank(), "shadow scene blank: {stats:?}");
    }
}
