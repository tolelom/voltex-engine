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
    let camera = scene_camera(width, height);
    let light_vp = light_view_projection();

    let resources = RawPbrResources::new(
        device,
        queue,
        "Shadow",
        NUM_OBJECTS,
        &scene_lights(),
        &shadow_uniform(light_vp.cols, SHADOW_MAP_SIZE as f32, 0.005),
    );
    let objects = scene_objects();
    let depth_pass = ShadowDepthPass::new(device, queue, light_vp, &objects);
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
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];
    resources.write_objects(queue, camera.view_projection(), cam_pos, &draws);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Shadow capture"),
    });
    // Pass 1 fills the shadow map; pass 2 samples it while shading.
    depth_pass.record(&mut encoder, &resources.shadow_map.view, &meshes, &objects);
    record_color_pass(&mut encoder, &target, &resources, &meshes, &objects);

    queue.submit(Some(encoder.finish()));
    target.read_rgba8(device, queue)
}

/// Camera at (8,8,12) looking at origin.
fn scene_camera(width: u32, height: u32) -> Camera {
    let mut camera = Camera::new(Vec3::new(8.0, 8.0, 12.0), width as f32 / height as f32);
    camera.pitch = -0.4;
    let to_origin = Vec3::ZERO - camera.position;
    camera.yaw = to_origin.x.atan2(-to_origin.z);
    camera
}

/// Dim ambient + one white directional light (the shadow caster).
fn scene_lights() -> LightsUniform {
    let mut lights = LightsUniform::new();
    lights.ambient_color = [0.05, 0.05, 0.05];
    lights.add_light(LightData::directional(
        [-1.0, -2.0, -1.0],
        [1.0, 1.0, 1.0],
        2.0,
    ));
    lights
}

/// Light view-projection (directional, orthographic).
fn light_view_projection() -> Mat4 {
    let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
    let light_pos = Vec3::ZERO - light_dir * 20.0;
    let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
    light_proj * light_view
}

/// Depth pre-pass resources: one light_vp × model slot per object.
struct ShadowDepthPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    stride: u32,
}

impl ShadowDepthPass {
    /// Build the pipeline and upload each object's `light_vp × model`.
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        light_vp: Mat4,
        objects: &[ShadowSceneObject],
    ) -> Self {
        let stride = uniform_stride::<ShadowPassUniform>(device);
        let layout = shadow_pass_bind_group_layout(device);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Pass UBO"),
            size: (stride as usize * NUM_OBJECTS) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Pass BG"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ShadowPassUniform>() as u64),
                }),
            }],
        });
        let pipeline = create_shadow_pipeline(device, &layout);

        let mut staging = vec![0u8; stride as usize * NUM_OBJECTS];
        for (i, object) in objects.iter().enumerate() {
            let uniform = ShadowPassUniform {
                light_vp_model: (light_vp * object.model).cols,
            };
            write_slot(&mut staging, stride, i, &uniform);
        }
        queue.write_buffer(&buffer, 0, &staging);
        Self {
            pipeline,
            bind_group,
            stride,
        }
    }

    /// Pass 1: render every object's depth from the light into `shadow_map`.
    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        shadow_map: &wgpu::TextureView,
        meshes: &SceneMeshes,
        objects: &[ShadowSceneObject],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: shadow_map,
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
        for (i, object) in objects.iter().enumerate() {
            pass.set_bind_group(0, &self.bind_group, &[(i as u32) * self.stride]);
            draw_mesh(&mut pass, meshes.get(object.shape));
        }
    }
}

/// Pass 2: PBR color sampling the shadow map.
fn record_color_pass(
    encoder: &mut wgpu::CommandEncoder,
    target: &OffscreenTarget,
    resources: &RawPbrResources,
    meshes: &SceneMeshes,
    objects: &[ShadowSceneObject],
) {
    let clear = wgpu::Color {
        r: 0.1,
        g: 0.1,
        b: 0.15,
        a: 1.0,
    };
    let mut pass = begin_color_pass(encoder, target, "Color Pass", clear);
    resources.bind_shared(&mut pass);
    for (i, object) in objects.iter().enumerate() {
        resources.bind_object(&mut pass, i);
        draw_mesh(&mut pass, meshes.get(object.shape));
    }
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
