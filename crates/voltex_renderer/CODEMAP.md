# voltex_renderer

## 책임 (한 줄)
wgpu 기반 렌더링 — 메시/PBR/그림자/IBL/디퍼드/RT/post-process. 이미지 디코더(PNG/JPG) + glTF/OBJ 파서 + MeshVertex는 `voltex_image`(L0)로 이동; 역호환 `pub use` 재export 유지.

## 공개 API 진입점

### Core GPU
- `GpuContext`, `DEPTH_FORMAT` — wgpu 디바이스/큐/surface (src/gpu.rs)
- `Vertex`, `MeshVertex` — 정점 포맷 (src/vertex.rs)
- `SkinnedMeshVertex`, `BonePalette`, `MAX_BONES` — 스킨드 메시 정점 (src/skinned_vertex.rs)
- `Skeleton`, `JointPose` — 조인트 계층 + 포즈 → BonePalette (from_skin/apply_animation/compute_palette) (src/skeleton.rs)
- `create_skinned_mesh_pipeline`, `skinned_camera_bind_group_layout`, `bone_palette_bind_group_layout` — GPU 스키닝 파이프라인 (src/pipeline.rs, src/mesh_skinned.wgsl)
- `TilemapVertex`, `TilemapUniform`, `TILEMAP_VERTEX_LAYOUT`, `tilemap_quad_indices`, `tilemap_uniform_bind_group_layout`, `create_tilemap_pipeline` — 배치 타일맵(월드 좌표 쿼드 + 꼭짓점 색, 스프라이트와 같은 블렌드/깊이 상태; src/tilemap.rs, src/tilemap.wgsl)
- `ColoredCameraUniform`, `create_colored_mesh_pipeline` — per-entity base_color 틴트 Blinn-Phong (BasicRenderer 전용, src/light.rs, src/mesh_colored.wgsl)
- `Mesh` — 메시 데이터 + `Mesh::from_obj(device, source)` / `Mesh::from_gltf(device, bytes)` 모델 로더 (src/mesh.rs)
- `Camera`, `FpsController` — 카메라 (src/camera.rs)
- `GpuTexture`, `pbr_texture_bind_group_layout` 등 — GPU 텍스처 + PBR 바인딩 (src/texture.rs)

### Image / Model (voltex_image 재export)
- `parse_png`, `parse_jpg`, `parse_gltf`, `GltfData`, `GltfMesh`, `GltfMaterial`, `MeshVertex` 등 — `voltex_image` crate로 이동; `pub use voltex_image::{...}` 역호환 재export 유지 (src/lib.rs)
- `obj` — OBJ 파서 모듈 재export (`voltex_image::obj`) (src/obj.rs → voltex_image)
- `deflate` — zlib/deflate inflate 모듈 재export (`voltex_image::deflate`)
- `json_parser` — glTF JSON 파서 모듈 재export (`voltex_image::json_parser`)

### Vertex Layout
- `MESH_VERTEX_LAYOUT` — wgpu `VertexBufferLayout` for `MeshVertex` (src/vertex.rs의 free const). `MeshVertex` 자체는 voltex_image에 있어 orphan rule상 inherent impl 불가 — layout descriptor만 renderer에 유지.

### Materials / Lighting / Shadows
- `MaterialUniform` — PBR 머티리얼 (src/material.rs)
- `LightData`, `LightsUniform`, `MAX_LIGHTS`, `LIGHT_DIRECTIONAL/POINT/SPOT` (src/light.rs)
- `ShadowMap`, `ShadowUniform` (src/shadow.rs)
- `CascadedShadowMap`, `compute_cascade_matrices` (src/csm.rs)
- `PointShadowMap`, `SpotShadowMap` (src/point_shadow.rs, src/spot_shadow.rs)
- `Plane`, `Frustum`, `extract_frustum`, `cull_lights` (src/frustum.rs)
- `create_pbr_pipeline`, `create_shadow_pipeline` (src/pbr_pipeline.rs, src/shadow_pipeline.rs)
- `generate_sphere` — UV sphere generator (src/sphere.rs)
- `generate_cube` / `generate_plane` / `generate_quad` — primitive mesh generators → `(Vec<MeshVertex>, Vec<u32>)` for `Mesh::new` (src/primitives.rs). 올바른 per-face normal/UV/tangent (PBR 노멀맵 호환)

### IBL
- `IblResources` (src/ibl.rs)
- SH irradiance — `compute_sh_coefficients`, `pack_sh_coefficients`, `evaluate_sh_cpu` (src/sh.rs)
- BRDF LUT compute (src/brdf_lut.rs + brdf_lut_compute.wgsl)

### Animation / Skinning
- `AnimationPlayer`, `Quat`, `sample_channel`, `build_trs`, `mat4_mul` (src/animation.rs)

### Deferred
- `GBuffer` (src/gbuffer.rs)
- `gbuffer_compress` — 옥타헤드럴 노멀 인코딩 (src/gbuffer_compress.rs)
- `FullscreenVertex`, `create_fullscreen_vertex_buffer` (src/fullscreen_quad.rs)
- `create_gbuffer_pipeline`, `create_lighting_pipeline` 등 (src/deferred_pipeline/: gbuffer, lighting, ssgi, rt_shadow, bloom, tonemap)

### RT (Ray Tracing)
- `RtAccel`, `RtInstance`, `BlasMeshData` (src/rt_accel.rs)
- `RtShadowResources`, `RT_SHADOW_FORMAT` (src/rt_shadow.rs)
- `RtReflections`, `RtAo`, `RtPointShadow`, `soft_rt_shadow` (src/rt_*.rs)
- `BlasTracker` — dirty mesh 추적 (src/blas_update.rs)
- `RtCapabilities`, `RenderMode` — fallback (src/rt_fallback.rs)

### SSGI / Light Volumes / Probes
- `SsgiResources`, `SsgiUniform`, `SSGI_OUTPUT_FORMAT` (src/ssgi.rs)
- `half_res_ssgi` (src/half_res_ssgi.rs)
- `stencil_opt` — 스텐실 최적화 (src/stencil_opt.rs)
- `LightVolume` — Sphere/Cone/Fullscreen (src/light_volumes.rs)
- `LightProbe`, `LightProbeGrid` (src/light_probes.rs)

### Post-process
- `HdrTarget`, `HDR_FORMAT` (src/hdr.rs)
- `BloomResources`, `BloomUniform`, `mip_sizes`, `BLOOM_MIP_COUNT` (src/bloom.rs)
- `bilateral_bloom` (src/bilateral_bloom.rs)
- `TonemapUniform`, `aces_tonemap` (src/tonemap.rs)
- `ForwardPass`, `sort_transparent_back_to_front` (src/forward_pass.rs)
- `AutoExposure` (src/auto_exposure.rs)
- `InstanceData`, `InstanceBuffer`, `create_instanced_pipeline` (src/instancing.rs)
- `BilateralBlur` (src/bilateral_blur.rs)
- `TemporalAccumulation` (src/temporal_accum.rs)
- `Taa` (src/taa.rs)
- `Ssr` (src/ssr.rs)
- `MotionBlur` (src/motion_blur.rs)
- `DepthOfField` (src/dof.rs)

### Pipeline
- `pipeline` — 기본 pipeline 헬퍼 (src/pipeline.rs)

### Error
- `RendererError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조

대분류 (자세한 항목은 lib.rs의 `pub mod`/`pub use` 참조):
- 코어 GPU: gpu, vertex, mesh, camera, texture, sphere, fullscreen_quad
- 이미지/모델: voltex_image 재export (png, jpg, progressive_jpeg, deflate, obj, gltf, json_parser — 실제 파일은 voltex_image에 있음)
- 머티리얼/조명/그림자: material, light, shadow, csm, point_shadow, spot_shadow, frustum, shadow_pipeline, pbr_pipeline
- IBL: sh, brdf_lut, ibl
- 애니메이션: animation, skinned_vertex, skeleton (조인트 계층→bone palette)
- 디퍼드: gbuffer, gbuffer_compress, deferred_pipeline
- RT: rt_accel, rt_shadow, rt_reflections, rt_ao, rt_point_shadow, soft_rt_shadow, blas_update, rt_fallback
- SSGI/Probes/Volumes: ssgi, half_res_ssgi, stencil_opt, light_volumes, light_probes
- 포스트: hdr, bloom, bilateral_bloom, tonemap, forward_pass, auto_exposure, instancing, bilateral_blur, temporal_accum, taa, ssr, motion_blur, dof
- 셰이더: 각종 .wgsl 파일 (mesh, pbr, shadow, deferred, ssgi, rt, bloom, taa, ssr, motion_blur, dof, bilateral_blur, auto_exposure, brdf_lut_compute, instanced, forward, skinned, sprite, tilemap)

## 의존성
- voltex crate: voltex_math, voltex_platform, voltex_image
- 외부 crate: wgpu 28.0, bytemuck (workspace), winit (간접)

## 변경 시 주의점
- max_bind_groups=4 — 리소스를 합쳐 4개 이내로 유지
- WGSL vec3 alignment 16바이트 (Rust struct에 padding 필요)
- wgpu 28.0 전용 필드: immediate_size, multiview_mask, TexelCopyTextureInfo
- gltf, jpg, deferred_pipeline은 src/<name>/ 디렉토리로 분리됨 (M2 완료). texture.rs (514줄)는 spec 만족 (600줄 이하), 분리 대상 아님
- deferred_pipeline 하위 모듈에서 WGSL 셰이더 include 경로는 `include_str!("../<name>.wgsl")` (한 단계 깊음)
- 이미지 디코더/glTF 파서는 `voltex_image`(L0)로 분리 완료 (E1, 2026-05-28). `pub use` 재export로 역호환 유지
- `RendererError::Image(#[from] voltex_image::ImageError)` variant — 디코더 에러를 렌더러 에러 체인으로 연결
- Forward pass(mesh_shader.wgsl, Blinn-Phong)와 PBR/Deferred path는 별도 — 기존 예제(triangle, model_viewer)는 forward 사용

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_renderer --lib`
- 현재 테스트 개수: 180개 (skeleton +6). 스킨드 GPU 시각 검증은 voltex_capture::scene_skinned (#[ignore] GPU-gated)
