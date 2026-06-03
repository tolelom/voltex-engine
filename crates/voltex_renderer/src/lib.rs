//! # voltex_renderer
//!
//! wgpu-backed rendering stack: GPU context, mesh + material assets,
//! PBR/Blinn-Phong pipelines, shadow systems (CSM/point/spot + RT),
//! deferred + forward passes, post-process chain (HDR/bloom/tonemap/TAA/SSR/
//! DoF/motion-blur), IBL, ray-traced shadows/reflections/AO, animation +
//! skinning.
//!
//! Asset parsing (PNG/JPG/glTF/OBJ) is bundled here for the engine's
//! dependency-free pipeline. The public surface is large; each module has
//! its own narrative `//!` header.

#![warn(missing_docs)]

pub mod auto_exposure;
pub mod bilateral_blur;
pub mod bloom;
pub mod brdf_lut;
pub mod camera;
pub mod csm;
pub mod deferred_pipeline;
pub mod dof;
pub mod error;
pub mod forward_pass;
pub mod frustum;
pub mod fullscreen_quad;
pub mod gbuffer;
pub mod gbuffer_compress;
pub mod gpu;
pub mod debug_line;
pub mod hdr;
pub mod ibl;
pub mod instancing;
pub mod light;
pub mod material;
pub mod mesh;
pub mod motion_blur;
pub mod pbr_pipeline;
pub mod pipeline;
pub mod point_shadow;
pub mod rt_accel;
pub mod rt_ao;
pub mod rt_point_shadow;
pub mod rt_reflections;
pub mod rt_shadow;
pub mod sh;
pub mod shadow;
pub mod shadow_pipeline;
pub mod sphere;
pub mod spot_shadow;
pub mod sprite;
pub mod ssgi;
pub mod ssr;
pub mod taa;
pub mod temporal_accum;
pub mod texture;
pub mod tonemap;
pub mod vertex;

pub use error::{RendererError, Result};
// Asset decoders now live in the GPU-free `voltex_image` crate. Re-export
// them so existing paths (`voltex_renderer::parse_png`, `crate::gltf::*`,
// etc.) keep working.
pub use auto_exposure::AutoExposure;
pub use bilateral_blur::BilateralBlur;
pub use bloom::{mip_sizes, BloomResources, BloomUniform, BLOOM_MIP_COUNT};
pub use camera::{Camera, FpsController};
pub use csm::{
    compute_cascade_matrices, CascadedShadowMap, CsmUniform, CSM_CASCADE_COUNT, CSM_FORMAT,
    CSM_MAP_SIZE,
};
pub use deferred_pipeline::{
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
    create_gbuffer_pipeline, create_lighting_pipeline, create_rt_shadow_pipeline,
    create_ssgi_pipeline, create_tonemap_pipeline, gbuffer_camera_bind_group_layout,
    lighting_gbuffer_bind_group_layout, lighting_lights_bind_group_layout,
    lighting_shadow_bind_group_layout, rt_shadow_data_bind_group_layout,
    rt_shadow_gbuffer_bind_group_layout, ssgi_data_bind_group_layout,
    ssgi_gbuffer_bind_group_layout, tonemap_bind_group_layout,
};
pub use dof::DepthOfField;
pub use forward_pass::{sort_transparent_back_to_front, ForwardPass};
pub use frustum::{cull_lights, extract_frustum, sphere_vs_frustum, Frustum, Plane};
pub use fullscreen_quad::{create_fullscreen_vertex_buffer, FullscreenVertex};
pub use gbuffer::GBuffer;
pub use gpu::{GpuContext, DEPTH_FORMAT};
pub use hdr::{HdrTarget, HDR_FORMAT};
pub use ibl::IblResources;
pub use debug_line::{
    create_line_pipeline, line_camera_bind_group_layout, LineCameraUniform, LineVertex,
    LINE_VERTEX_LAYOUT,
};
pub use instancing::{create_instanced_pipeline, InstanceBuffer, InstanceData};
pub use light::{
    CameraUniform, ColoredCameraUniform, LightData, LightUniform, LightsUniform, PointLight,
    PointLightsUniform, LIGHT_DIRECTIONAL, LIGHT_POINT, LIGHT_SPOT, MAX_LIGHTS, MAX_POINT_LIGHTS,
};
pub use material::MaterialUniform;
pub use mesh::Mesh;
pub use motion_blur::MotionBlur;
pub use pbr_pipeline::create_pbr_pipeline;
pub use pipeline::{
    bone_palette_bind_group_layout, create_colored_mesh_pipeline, create_skinned_mesh_pipeline,
    skinned_camera_bind_group_layout,
};
pub use point_shadow::{
    point_shadow_projection, point_shadow_view_matrices, PointShadowMap, POINT_SHADOW_FORMAT,
    POINT_SHADOW_SIZE,
};
pub use rt_accel::{mat4_to_tlas_transform, BlasMeshData, RtAccel, RtInstance};
pub use rt_ao::RtAo;
pub use rt_point_shadow::RtPointShadow;
pub use rt_reflections::RtReflections;
pub use rt_shadow::{RtShadowResources, RtShadowUniform, RT_SHADOW_FORMAT};
pub use sh::{compute_sh_coefficients, evaluate_sh_cpu, pack_sh_coefficients};
pub use shadow::{
    shadow_main_bind_group_layout, ShadowMainUniform, ShadowMap, ShadowPassUniform, ShadowUniform,
    SHADOW_FORMAT, SHADOW_MAP_SIZE,
};
pub use shadow_pipeline::{create_shadow_pipeline, shadow_pass_bind_group_layout};
pub use sphere::generate_sphere;
pub use spot_shadow::{spot_shadow_matrix, SpotShadowMap};
pub use sprite::{
    create_sprite_pipeline, sprite_uniform_bind_group_layout, SpriteUniform, SpriteVertex,
    SPRITE_INDICES, SPRITE_QUAD, SPRITE_VERTEX_LAYOUT,
};
pub use ssgi::{SsgiResources, SsgiUniform, SSGI_OUTPUT_FORMAT};
pub use ssr::Ssr;
pub use taa::Taa;
pub use temporal_accum::TemporalAccumulation;
pub use texture::{
    create_pbr_full_texture_bind_group, create_pbr_texture_bind_group,
    pbr_full_texture_bind_group_layout, pbr_texture_bind_group_layout, GpuTexture,
};
pub use tonemap::{aces_tonemap, TonemapUniform};
pub use vertex::MESH_VERTEX_LAYOUT;
pub use vertex::{MeshVertex, Vertex};
pub use voltex_image::{deflate, gltf, jpg, json_parser, obj, png, progressive_jpeg};
pub use voltex_image::{parse_gltf, parse_jpg, parse_png, GltfData, GltfMaterial, GltfMesh};
pub mod bilateral_bloom;
pub mod half_res_ssgi;
pub mod stencil_opt;

pub mod blas_update;
pub mod soft_rt_shadow;
pub use blas_update::BlasTracker;
pub mod rt_fallback;
pub use rt_fallback::{RenderMode, RtCapabilities};
pub mod light_probes;
pub use light_probes::{LightProbe, LightProbeGrid};
pub mod light_volumes;
pub use light_volumes::LightVolume;
pub mod skinned_vertex;
pub use skinned_vertex::{
    skinned_vertices_from_gltf, BonePalette, SkinnedCameraUniform, SkinnedMesh, SkinnedMeshVertex,
    MAX_BONES,
};
pub mod animation;
pub use animation::{build_trs, mat4_mul, sample_channel, AnimationPlayer};
pub mod skeleton;
pub use skeleton::{JointPose, Skeleton};
// `Quat` was historically defined in `animation.rs`; it now lives in
// `voltex_math` (E3a). Keep the `voltex_renderer::Quat` path for back-compat
// — animation.rs and external callers (skinning, glTF) keep working.
pub use voltex_math::Quat;
