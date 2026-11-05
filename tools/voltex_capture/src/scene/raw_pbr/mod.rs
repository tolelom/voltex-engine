//! Scenes drawn with a **hand-built** wgpu PBR pipeline (`create_pbr_pipeline`
//! plus our own buffers and bind groups) instead of `BasicRenderer`. They check
//! the Cook-Torrance shader and its bind-group layout directly, without the
//! runtime renderer in between.
//!
//! - [`render_pbr_grid`]: 7×7 metallic/roughness sphere grid
//! - [`render_shadow_scene`]: two-pass directional shadow (depth pre-pass + PBR)
//! - [`render_multi_light`]: several point/spot/directional lights at once
//!
//! The `BasicRenderer` PBR path lives in `scene::pbr`.

use voltex_math::Mat4;
use voltex_renderer::{
    create_pbr_pipeline, create_pbr_texture_set_bind_group, pbr_full_texture_bind_group_layout,
    CameraUniform, GpuTexture, IblResources, LightsUniform, MaterialUniform, Mesh, PbrTextureSet,
    ShadowMap, ShadowUniform,
};
use wgpu::util::DeviceExt;

use crate::target::{align_up, OffscreenTarget, CAPTURE_FORMAT};

mod grid;
mod multi_light;
mod shadow;

pub use grid::render_pbr_grid;
pub use multi_light::render_multi_light;
pub use shadow::render_shadow_scene;

/// Bind group layout for group 0 of the PBR pipeline: binding 0 = per-object
/// `CameraUniform` (dynamic offset), binding 1 = `LightsUniform`.
pub(crate) fn camera_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera+Light BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<CameraUniform>() as u64
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

/// Byte distance between consecutive `T` slots in a dynamic-offset uniform
/// buffer (`size_of::<T>()` rounded up to the device's offset alignment).
pub(crate) fn uniform_stride<T>(device: &wgpu::Device) -> u32 {
    let alignment = device.limits().min_uniform_buffer_offset_alignment;
    align_up(std::mem::size_of::<T>() as u32, alignment)
}

/// Copy `value` into slot `index` of a dynamic-offset staging buffer.
pub(crate) fn write_slot<T: bytemuck::Pod>(staging: &mut [u8], stride: u32, index: usize, value: &T) {
    let start = index * stride as usize;
    staging[start..start + std::mem::size_of::<T>()].copy_from_slice(bytemuck::bytes_of(value));
}

/// `ShadowUniform` with the sky fields every raw scene shares (fixed sun
/// direction, turbidity 3, no SH ambient). `shadow_map_size` 0 means "no
/// shadow map".
pub(crate) fn shadow_uniform(
    light_view_proj: [[f32; 4]; 4],
    shadow_map_size: f32,
    shadow_bias: f32,
) -> ShadowUniform {
    ShadowUniform {
        light_view_proj,
        shadow_map_size,
        shadow_bias,
        _padding: [0.0; 2],
        sun_direction: [0.5, -0.7, 0.5],
        turbidity: 3.0,
        sh_coefficients: [[0.0; 4]; 7],
    }
}

/// Which shared mesh an object draws.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Cube,
    Sphere,
}

/// The two meshes the raw scenes draw from.
pub(crate) struct SceneMeshes {
    pub cube: Mesh,
    pub sphere: Mesh,
}

impl SceneMeshes {
    pub fn get(&self, shape: Shape) -> &Mesh {
        match shape {
            Shape::Cube => &self.cube,
            Shape::Sphere => &self.sphere,
        }
    }
}

/// Bind `mesh`'s vertex/index buffers and draw it once.
pub(crate) fn draw_mesh(pass: &mut wgpu::RenderPass<'_>, mesh: &Mesh) {
    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
}

/// One drawable: its model matrix and material.
pub(crate) struct RawPbrObject {
    pub model: Mat4,
    pub material: MaterialUniform,
}

/// Everything a raw PBR scene binds besides its meshes: the pipeline,
/// per-object camera/material uniforms (one dynamic-offset slot per object),
/// the lights, default 1×1 PBR maps, and the shadow/IBL group.
///
/// Bind groups: 0 = camera+lights, 1 = PBR textures, 2 = material,
/// 3 = shadow map + BRDF LUT.
pub(crate) struct RawPbrResources {
    pub pipeline: wgpu::RenderPipeline,
    /// The shadow map sampled by group 3 (a shadow scene renders into it).
    pub shadow_map: ShadowMap,
    camera_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    camera_stride: u32,
    material_stride: u32,
    object_count: usize,
    camera_light_bind_group: wgpu::BindGroup,
    pbr_texture_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
}

impl RawPbrResources {
    /// Create the pipeline and all shared buffers/bind groups for
    /// `object_count` objects. `label` prefixes every GPU object label.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        object_count: usize,
        lights: &LightsUniform,
        shadow: &ShadowUniform,
    ) -> Self {
        let camera_stride = uniform_stride::<CameraUniform>(device);
        let material_stride = uniform_stride::<MaterialUniform>(device);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Camera UBO")),
            size: (camera_stride as usize * object_count) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Light UBO")),
            contents: bytemuck::cast_slice(&[*lights]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label} Material UBO")),
            size: (material_stride as usize * object_count) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_light_layout = camera_light_bind_group_layout(device);
        let pbr_tex_layout = pbr_full_texture_bind_group_layout(device);
        let material_layout = MaterialUniform::bind_group_layout(device);

        let camera_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} Camera+Light BG")),
            layout: &camera_light_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &camera_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        let pbr_texture_bind_group = default_pbr_texture_bind_group(device, queue, &pbr_tex_layout);

        let ibl = IblResources::new(device, queue);

        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label} Material BG")),
            layout: &material_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &material_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<MaterialUniform>() as u64),
                }),
            }],
        });

        let shadow_map = ShadowMap::new(device);
        let shadow_layout = ShadowMap::bind_group_layout(device);
        let shadow_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Shadow UBO")),
            contents: bytemuck::cast_slice(&[*shadow]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let shadow_bind_group = shadow_map.create_bind_group(
            device,
            &shadow_layout,
            &shadow_uniform_buffer,
            &ibl.brdf_lut_view,
            &ibl.brdf_lut_sampler,
        );

        let pipeline = create_pbr_pipeline(
            device,
            CAPTURE_FORMAT,
            &camera_light_layout,
            &pbr_tex_layout,
            &material_layout,
            &shadow_layout,
        );

        Self {
            pipeline,
            shadow_map,
            camera_buffer,
            material_buffer,
            camera_stride,
            material_stride,
            object_count,
            camera_light_bind_group,
            pbr_texture_bind_group,
            material_bind_group,
            shadow_bind_group,
        }
    }

    /// Upload each object's camera (`view_proj` × its model) and material into
    /// its slot. `objects.len()` must equal the `object_count` given to `new`.
    pub fn write_objects(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: [f32; 3],
        objects: &[RawPbrObject],
    ) {
        assert_eq!(objects.len(), self.object_count, "one slot per object");
        let mut camera_staging = vec![0u8; self.object_count * self.camera_stride as usize];
        let mut material_staging = vec![0u8; self.object_count * self.material_stride as usize];
        for (i, object) in objects.iter().enumerate() {
            let camera = CameraUniform {
                view_proj: view_proj.cols,
                model: object.model.cols,
                camera_pos,
                _padding: 0.0,
            };
            write_slot(&mut camera_staging, self.camera_stride, i, &camera);
            write_slot(&mut material_staging, self.material_stride, i, &object.material);
        }
        queue.write_buffer(&self.camera_buffer, 0, &camera_staging);
        queue.write_buffer(&self.material_buffer, 0, &material_staging);
    }

    /// Set the pipeline and the per-frame groups (1 = textures, 3 = shadow).
    pub fn bind_shared(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &self.pbr_texture_bind_group, &[]);
        pass.set_bind_group(3, &self.shadow_bind_group, &[]);
    }

    /// Point groups 0 (camera) and 2 (material) at object `index`'s slot.
    pub fn bind_object(&self, pass: &mut wgpu::RenderPass<'_>, index: usize) {
        let index = index as u32;
        pass.set_bind_group(0, &self.camera_light_bind_group, &[index * self.camera_stride]);
        pass.set_bind_group(2, &self.material_bind_group, &[index * self.material_stride]);
    }
}

/// Group 1 bound to default 1×1 maps: white albedo, flat normal, white ORM,
/// black emissive — so only the material uniform drives shading.
fn default_pbr_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pbr_tex_layout: &wgpu::BindGroupLayout,
) -> wgpu::BindGroup {
    // 1×1 텍스처를 만들 때만 쓰는 단일 텍스처 bind group 레이아웃.
    let single_texture_layout = GpuTexture::bind_group_layout(device);
    let albedo = GpuTexture::white_1x1(device, queue, &single_texture_layout);
    let normal = GpuTexture::flat_normal_map_1x1(device, queue, &single_texture_layout);
    let orm = GpuTexture::white_1x1(device, queue, &single_texture_layout);
    let emissive = GpuTexture::black_1x1(device, queue, &single_texture_layout);
    create_pbr_texture_set_bind_group(
        device,
        pbr_tex_layout,
        PbrTextureSet {
            albedo: &albedo,
            normal: &normal,
            orm: &orm,
            emissive: &emissive,
        },
    )
}

/// Begin the main color pass into `target`, clearing color to `clear` and
/// depth to 1.0.
pub(crate) fn begin_color_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    target: &OffscreenTarget,
    label: &str,
    clear: wgpu::Color,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target.color_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &target.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    })
}
