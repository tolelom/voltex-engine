//! # voltex_renderer
//!
//! wgpu-backed rendering stack: GPU context, mesh + material assets,
//! PBR/Blinn-Phong pipelines, shadow systems (CSM/point/spot + RT),
//! deferred + forward passes, post-process chain (HDR/bloom/tonemap/TAA/SSR/
//! DoF/motion-blur), IBL, ray-traced shadows/reflections/AO, animation +
//! skinning.
//!
//! Asset decoding (PNG/JPG/BMP/glTF/OBJ) lives in the GPU-free `voltex_image`
//! crate; the old `voltex_renderer::parse_png` etc. paths are re-exported
//! below. The public surface is large; each module has its own `//!` header.

#![warn(missing_docs)]

pub mod error;
mod compute;

// ── 코어 GPU: 컨텍스트, 정점, 메시, 텍스처, 카메라 ─────────────────────────
pub mod camera;
pub mod fullscreen_quad;
pub mod gpu;
pub mod mesh;
pub mod pipeline;
pub mod primitives;
pub mod sphere;
pub mod texture;
pub mod vertex;

// ── 2D / 디버그: 스프라이트, 타일맵, 디버그 라인 ───────────────────────────
pub mod debug_line;
pub mod sprite;
pub mod tilemap;

// ── 머티리얼 / 조명 / 그림자 ─────────────────────────────────────────────
pub mod csm;
pub mod frustum;
pub mod light;
pub mod material;
pub mod pbr_pipeline;
pub mod point_shadow;
pub mod shadow;
pub mod shadow_pipeline;
pub mod spot_shadow;

// ── IBL / 구면 조화 / 라이트 프로브 ──────────────────────────────────────
pub mod brdf_lut;
pub mod ibl;
pub mod light_probes;
pub mod sh;

// ── 애니메이션 / 스키닝 ─────────────────────────────────────────────────
pub mod animation;
pub mod skeleton;
pub mod skinned_vertex;

// ── 디퍼드 / 포워드 / 인스턴싱 ──────────────────────────────────────────
pub mod deferred_pipeline;
pub mod forward_pass;
pub mod gbuffer;
pub mod gbuffer_compress;
pub mod instancing;
pub mod light_volumes;
pub mod stencil_opt;

// ── 레이 트레이싱 ──────────────────────────────────────────────────────
pub mod blas_update;
pub mod rt_accel;
pub mod rt_ao;
pub mod rt_fallback;
pub mod rt_point_shadow;
pub mod rt_reflections;
pub mod rt_shadow;
pub mod soft_rt_shadow;

// ── 화면 공간 효과 / 포스트 프로세스 ────────────────────────────────────
pub mod auto_exposure;
pub mod bilateral_bloom;
pub mod bilateral_blur;
pub mod bloom;
pub mod dof;
pub mod half_res_ssgi;
pub mod hdr;
pub mod motion_blur;
pub mod ssgi;
pub mod ssr;
pub mod taa;
pub mod temporal_accum;
pub mod tonemap;

// ════════════════════════════ re-exports ════════════════════════════════

pub use error::{RendererError, Result};

// 코어 GPU
pub use camera::{Camera, FpsController};
pub use fullscreen_quad::{create_fullscreen_vertex_buffer, FullscreenVertex};
pub use gpu::{align_up, GpuContext, DEPTH_FORMAT};
pub use mesh::Mesh;
pub use pipeline::{
    bone_palette_bind_group_layout, create_colored_mesh_pipeline, create_skinned_mesh_pipeline,
    create_skinned_shadow_pipeline, skinned_camera_bind_group_layout,
};
pub use primitives::{generate_cube, generate_plane, generate_quad};
pub use sphere::generate_sphere;
pub use texture::{
    create_pbr_full_texture_bind_group, create_pbr_texture_bind_group,
    pbr_full_texture_bind_group_layout, pbr_texture_bind_group_layout, GpuTexture,
};
#[allow(deprecated)]
pub use vertex::Vertex;
pub use vertex::{MeshVertex, PositionColorVertex, MESH_VERTEX_LAYOUT};

// 2D / 디버그
pub use debug_line::{
    create_line_pipeline, line_camera_bind_group_layout, LineCameraUniform, LineVertex,
    LINE_VERTEX_LAYOUT,
};
pub use sprite::{
    create_sprite_pipeline, sprite_uniform_bind_group_layout, SpriteUniform, SpriteVertex,
    SPRITE_INDICES, SPRITE_QUAD, SPRITE_VERTEX_LAYOUT,
};
pub use tilemap::{
    create_tilemap_pipeline, tilemap_quad_indices, tilemap_uniform_bind_group_layout,
    TilemapUniform, TilemapVertex, TILEMAP_VERTEX_LAYOUT,
};

// 머티리얼 / 조명 / 그림자
pub use csm::{
    compute_cascade_matrices, CascadedShadowMap, CsmUniform, CSM_CASCADE_COUNT, CSM_FORMAT,
    CSM_MAP_SIZE,
};
pub use frustum::{cull_lights, extract_frustum, sphere_vs_frustum, Frustum, Plane};
pub use light::{
    CameraUniform, ColoredCameraUniform, LightData, LightUniform, LightsUniform, PointLight,
    PointLightsUniform, LIGHT_DIRECTIONAL, LIGHT_POINT, LIGHT_SPOT, MAX_LIGHTS, MAX_POINT_LIGHTS,
};
pub use material::MaterialUniform;
pub use pbr_pipeline::{create_pbr_pipeline, create_pbr_skinned_pipeline};
pub use point_shadow::{
    point_shadow_projection, point_shadow_view_matrices, PointShadowMap, POINT_SHADOW_FORMAT,
    POINT_SHADOW_SIZE,
};
pub use shadow::{
    shadow_main_bind_group_layout, ShadowMainUniform, ShadowMap, ShadowPassUniform, ShadowUniform,
    SHADOW_FORMAT, SHADOW_MAP_SIZE,
};
pub use shadow_pipeline::{create_shadow_pipeline, shadow_pass_bind_group_layout};
pub use spot_shadow::{spot_shadow_matrix, SpotShadowMap};

// IBL / 구면 조화 / 라이트 프로브
pub use ibl::IblResources;
pub use light_probes::{LightProbe, LightProbeGrid};
pub use sh::{compute_sh_coefficients, evaluate_sh_cpu, pack_sh_coefficients};

// 애니메이션 / 스키닝
pub use animation::{build_trs, mat4_mul, sample_channel, AnimationPlayer};
pub use skeleton::{JointPose, Skeleton};
pub use skinned_vertex::{
    skinned_vertices_from_gltf, BonePalette, SkinnedCameraUniform, SkinnedMesh, SkinnedMeshVertex,
    MAX_BONES,
};
// `Quat` 는 원래 animation.rs 에 있다가 voltex_math 로 옮겨졌다(E3a).
// `voltex_renderer::Quat` 경로는 역호환으로 유지.
pub use voltex_math::Quat;

// 디퍼드 / 포워드 / 인스턴싱
pub use deferred_pipeline::{
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
    create_gbuffer_pipeline, create_lighting_pipeline, create_rt_shadow_pipeline,
    create_ssgi_pipeline, create_tonemap_pipeline, gbuffer_camera_bind_group_layout,
    lighting_gbuffer_bind_group_layout, lighting_lights_bind_group_layout,
    lighting_shadow_bind_group_layout, rt_shadow_data_bind_group_layout,
    rt_shadow_gbuffer_bind_group_layout, ssgi_data_bind_group_layout,
    ssgi_gbuffer_bind_group_layout, tonemap_bind_group_layout,
};
pub use forward_pass::{sort_transparent_back_to_front, ForwardPass};
pub use gbuffer::GBuffer;
pub use instancing::{create_instanced_pipeline, InstanceBuffer, InstanceData};
pub use light_volumes::LightVolume;

// 레이 트레이싱
pub use blas_update::BlasTracker;
pub use rt_accel::{mat4_to_tlas_transform, BlasMeshData, RtAccel, RtInstance};
pub use rt_ao::RtAo;
pub use rt_fallback::{RenderMode, RtCapabilities};
pub use rt_point_shadow::RtPointShadow;
pub use rt_reflections::RtReflections;
pub use rt_shadow::{RtShadowResources, RtShadowUniform, RT_SHADOW_FORMAT};

// 화면 공간 효과 / 포스트 프로세스
pub use auto_exposure::AutoExposure;
pub use bilateral_blur::BilateralBlur;
pub use bloom::{mip_sizes, BloomResources, BloomUniform, BLOOM_MIP_COUNT};
pub use dof::DepthOfField;
pub use hdr::{HdrTarget, HDR_FORMAT};
pub use motion_blur::MotionBlur;
pub use ssgi::{SsgiResources, SsgiUniform, SSGI_OUTPUT_FORMAT};
pub use ssr::Ssr;
pub use taa::Taa;
pub use temporal_accum::TemporalAccumulation;
pub use tonemap::{aces_tonemap, TonemapUniform};

// 에셋 디코더는 GPU 무관 crate `voltex_image` 에 있다. 기존 경로
// (`voltex_renderer::parse_png`, `voltex_renderer::gltf::*` 등) 역호환 재export.
pub use voltex_image::{deflate, gltf, jpg, json_parser, obj, png, progressive_jpeg};
#[allow(deprecated)]
pub use voltex_image::{parse_gltf, parse_jpg, parse_png, GltfData, GltfMaterial, GltfMesh};
pub use voltex_image::{decode_jpg, decode_png, DecodedImage};
