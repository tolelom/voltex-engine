//! Shadow-mapping capture — the two-pass directional shadow path
//! (`create_shadow_pipeline` depth pass + `create_pbr_pipeline` sampling the
//! shadow map). Shadows are a classic source of subtle bugs (acne,
//! peter-panning, wrong light projection), so a visual artifact is the only
//! reliable check.

use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_pbr_pipeline, create_shadow_pipeline,
    generate_sphere, pbr_full_texture_bind_group_layout, CameraUniform, Camera, GpuTexture,
    IblResources, LightData, LightsUniform, MaterialUniform, Mesh, ShadowMap, ShadowPassUniform,
    ShadowUniform, SHADOW_MAP_SIZE, shadow_pass_bind_group_layout,
};
use wgpu::util::DeviceExt;

use crate::headless::HeadlessGpu;
use crate::scene::{camera_light_bgl, unit_cube};
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

const NUM_OBJECTS: usize = 6;

fn align_up(size: u32, alignment: u32) -> u32 {
    size.div_ceil(alignment) * alignment
}

fn object_models() -> [Mat4; NUM_OBJECTS] {
    [
        Mat4::translation(0.0, -0.5, 0.0).mul_mat4(&Mat4::scale(15.0, 0.1, 15.0)), // ground
        Mat4::translation(-3.0, 1.0, 0.0),
        Mat4::translation(0.0, 1.5, 0.0),
        Mat4::translation(3.0, 0.8, 0.0),
        Mat4::translation(-1.5, 0.5, -2.0),
        Mat4::translation(1.5, 0.5, 2.0),
    ]
}

fn object_materials() -> [([f32; 4], f32, f32); NUM_OBJECTS] {
    [
        ([0.7, 0.7, 0.7, 1.0], 0.0, 0.8),
        ([0.9, 0.2, 0.2, 1.0], 0.3, 0.4),
        ([0.2, 0.9, 0.2, 1.0], 0.5, 0.3),
        ([0.2, 0.2, 0.9, 1.0], 0.1, 0.6),
        ([0.9, 0.8, 0.2, 1.0], 0.7, 0.2),
        ([0.8, 0.3, 0.8, 1.0], 0.2, 0.5),
    ]
}

fn is_cube(i: usize) -> bool {
    i == 0 || i == 4 || i == 5
}

/// Render the 6-object shadow scene (ground + 3 spheres + 2 cubes) lit by one
/// directional light, with a real shadow depth pre-pass. Returns RGBA8.
pub fn render_shadow_scene(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let alignment = device.limits().min_uniform_buffer_offset_alignment;
    let cam_aligned = align_up(std::mem::size_of::<CameraUniform>() as u32, alignment);
    let mat_aligned = align_up(std::mem::size_of::<MaterialUniform>() as u32, alignment);
    let sp_aligned = align_up(std::mem::size_of::<ShadowPassUniform>() as u32, alignment);

    let (sv, si) = generate_sphere(0.5, 32, 16);
    let sphere_mesh = Mesh::new(device, &sv, &si);
    let cube_mesh = unit_cube(device);

    // Camera at (8,8,12) looking at origin.
    let mut camera = Camera::new(Vec3::new(8.0, 8.0, 12.0), width as f32 / height as f32);
    camera.pitch = -0.4;
    let to_origin = Vec3::ZERO - camera.position;
    camera.yaw = to_origin.x.atan2(-to_origin.z);
    let view_proj = camera.view_projection();
    let cam_pos = [camera.position.x, camera.position.y, camera.position.z];

    let mut lights = LightsUniform::new();
    lights.ambient_color = [0.05, 0.05, 0.05];
    lights.add_light(LightData::directional([-1.0, -2.0, -1.0], [1.0, 1.0, 1.0], 2.0));

    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Camera UBO"),
        size: (cam_aligned as usize * NUM_OBJECTS) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Shadow Light UBO"),
        contents: bytemuck::cast_slice(&[lights]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Material UBO"),
        size: (mat_aligned as usize * NUM_OBJECTS) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let cl_layout = camera_light_bgl(device);
    let pbr_tex_layout = pbr_full_texture_bind_group_layout(device);
    let mat_layout = MaterialUniform::bind_group_layout(device);

    let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Camera+Light BG"),
        layout: &cl_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &camera_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                }),
            },
            wgpu::BindGroupEntry { binding: 1, resource: light_buffer.as_entire_binding() },
        ],
    });

    let old_tex_layout = GpuTexture::bind_group_layout(device);
    let albedo = GpuTexture::white_1x1(device, queue, &old_tex_layout);
    let normal = GpuTexture::flat_normal_1x1(device, queue);
    let orm = GpuTexture::white_1x1(device, queue, &old_tex_layout);
    let emissive = GpuTexture::black_1x1(device, queue, &old_tex_layout);
    let pbr_texture_bind_group = create_pbr_full_texture_bind_group(
        device, &pbr_tex_layout, &albedo.view, &albedo.sampler, &normal.1, &normal.2,
        &orm.view, &orm.sampler, &emissive.view, &emissive.sampler,
    );

    let ibl = IblResources::new(device, queue);

    let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Material BG"),
        layout: &mat_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &material_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
            }),
        }],
    });

    // Light view-projection (directional, orthographic).
    let light_dir = Vec3::new(-1.0, -2.0, -1.0).normalize();
    let light_pos = Vec3::ZERO - light_dir * 20.0;
    let light_view = Mat4::look_at(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = Mat4::orthographic(-15.0, 15.0, -15.0, 15.0, 0.1, 50.0);
    let light_vp = light_proj * light_view;

    let shadow_map = ShadowMap::new(device);
    let shadow_layout = ShadowMap::bind_group_layout(device);
    let shadow_uniform = ShadowUniform {
        light_view_proj: light_vp.cols,
        shadow_map_size: SHADOW_MAP_SIZE as f32,
        shadow_bias: 0.005,
        _padding: [0.0; 2],
        sun_direction: [0.5, -0.7, 0.5],
        turbidity: 3.0,
        sh_coefficients: [[0.0; 4]; 7],
    };
    let shadow_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Shadow Uniform"),
        contents: bytemuck::cast_slice(&[shadow_uniform]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let shadow_bind_group = shadow_map.create_bind_group(
        device, &shadow_layout, &shadow_uniform_buffer, &ibl.brdf_lut_view, &ibl.brdf_lut_sampler,
    );

    let sp_layout = shadow_pass_bind_group_layout(device);
    let shadow_pass_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Pass UBO"),
        size: (sp_aligned as usize * NUM_OBJECTS) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shadow_pass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Shadow Pass BG"),
        layout: &sp_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &shadow_pass_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<ShadowPassUniform>() as u64),
            }),
        }],
    });

    let shadow_pipeline = create_shadow_pipeline(device, &sp_layout);
    let pbr_pipeline = create_pbr_pipeline(
        device, CAPTURE_FORMAT, &cl_layout, &pbr_tex_layout, &mat_layout, &shadow_layout,
    );

    // Staging.
    let models = object_models();
    let materials = object_materials();
    let mut sp_staging = vec![0u8; sp_aligned as usize * NUM_OBJECTS];
    let mut cam_staging = vec![0u8; cam_aligned as usize * NUM_OBJECTS];
    let mut mat_staging = vec![0u8; mat_aligned as usize * NUM_OBJECTS];
    for i in 0..NUM_OBJECTS {
        let sp = ShadowPassUniform { light_vp_model: (light_vp * models[i]).cols };
        let o = i * sp_aligned as usize;
        sp_staging[o..o + std::mem::size_of::<ShadowPassUniform>()]
            .copy_from_slice(bytemuck::bytes_of(&sp));

        let cam = CameraUniform {
            view_proj: view_proj.cols,
            model: models[i].cols,
            camera_pos: cam_pos,
            _padding: 0.0,
        };
        let o = i * cam_aligned as usize;
        cam_staging[o..o + std::mem::size_of::<CameraUniform>()]
            .copy_from_slice(bytemuck::bytes_of(&cam));

        let (color, metallic, roughness) = materials[i];
        let mat = MaterialUniform::with_params(color, metallic, roughness);
        let o = i * mat_aligned as usize;
        mat_staging[o..o + std::mem::size_of::<MaterialUniform>()]
            .copy_from_slice(bytemuck::bytes_of(&mat));
    }
    queue.write_buffer(&shadow_pass_buffer, 0, &sp_staging);
    queue.write_buffer(&camera_buffer, 0, &cam_staging);
    queue.write_buffer(&material_buffer, 0, &mat_staging);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Shadow capture"),
    });

    // Pass 1: shadow depth.
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &shadow_map.view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&shadow_pipeline);
        for i in 0..NUM_OBJECTS {
            pass.set_bind_group(0, &shadow_pass_bind_group, &[(i as u32) * sp_aligned]);
            let mesh = if is_cube(i) { &cube_mesh } else { &sphere_mesh };
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }

    // Pass 2: PBR color sampling the shadow map.
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Color Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pbr_pipeline);
        pass.set_bind_group(1, &pbr_texture_bind_group, &[]);
        pass.set_bind_group(3, &shadow_bind_group, &[]);
        for i in 0..NUM_OBJECTS {
            pass.set_bind_group(0, &camera_light_bind_group, &[(i as u32) * cam_aligned]);
            pass.set_bind_group(2, &material_bind_group, &[(i as u32) * mat_aligned]);
            let mesh = if is_cube(i) { &cube_mesh } else { &sphere_mesh };
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
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
