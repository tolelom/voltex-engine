# voltex_renderer

## 책임 (한 줄)
wgpu 기반 렌더링 — 메시/PBR/그림자/IBL/디퍼드/RT/post-process. 이미지 디코더(PNG/JPG) + glTF/OBJ 파서 + MeshVertex는 `voltex_image`(L0)로 이동; 역호환 `pub use` 재export 유지.

## 공개 API 진입점

### Core GPU
- `GpuContext`, `DEPTH_FORMAT` — wgpu 디바이스/큐/surface + `adapter_info`(로그·크래시 리포트용) (src/gpu.rs)
- `align_up(value, alignment)` — dynamic-offset UBO stride·텍스처 행 간격용 올림 (src/gpu.rs)
- `PositionColorVertex`(구 `Vertex`, deprecated alias), `MeshVertex` — 정점 포맷 (src/vertex.rs)
- `SkinnedMeshVertex`, `BonePalette`, `MAX_BONES` — 스킨드 메시 정점 (src/skinned_vertex.rs)
- `Skeleton`, `JointPose` — 조인트 계층 + 포즈 → BonePalette (from_skin/apply_animation/compute_palette) (src/skeleton.rs)
- `create_skinned_mesh_pipeline`, `skinned_camera_bind_group_layout`, `bone_palette_bind_group_layout` — GPU 스키닝 파이프라인 (src/pipeline.rs, src/mesh_skinned.wgsl)
- `TilemapVertex`, `TilemapUniform`, `TILEMAP_VERTEX_LAYOUT`, `tilemap_quad_indices`, `tilemap_uniform_bind_group_layout`, `create_tilemap_pipeline` — 배치 타일맵(월드 좌표 쿼드 + 꼭짓점 색, 스프라이트와 같은 블렌드/깊이 상태; src/tilemap.rs, src/tilemap.wgsl)
- `ColoredCameraUniform`, `create_colored_mesh_pipeline` — per-entity base_color 틴트 Blinn-Phong (BasicRenderer 전용, src/light.rs, src/mesh_colored.wgsl)
- `Mesh` — 메시 데이터 + `Mesh::from_obj(device, source)` / `Mesh::from_gltf(device, bytes)` 모델 로더 (src/mesh.rs)
- `Camera`, `FpsController` — 카메라 (src/camera.rs)
- `GpuTexture` (`white_1x1`/`black_1x1`/`flat_normal_map_1x1` 기본 맵; 구 `flat_normal_1x1` 튜플 반환은 deprecated), `pbr_texture_bind_group_layout` 등 — GPU 텍스처 + PBR 바인딩 (src/texture/mod.rs). `texture::parse_bmp`/`BmpImage` 는 `voltex_image::bmp` 래퍼

### Image / Model (voltex_image 재export)
- `decode_png`, `decode_jpg`, `DecodedImage`, `parse_gltf`, `GltfData`, `GltfMesh`, `GltfMaterial`, `MeshVertex` 등 — `voltex_image` crate로 이동; `pub use voltex_image::{...}` 역호환 재export 유지 (src/lib.rs). deprecated `parse_png`/`parse_jpg` 도 재export
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
- `MeshData { vertices, indices }` — CPU 메시 데이터, `upload(device) -> Mesh` (src/primitives.rs)
- `generate_sphere_mesh` — UV sphere generator → `MeshData` (src/sphere.rs)
- `generate_cube_mesh` / `generate_plane_mesh` / `generate_quad_mesh` — primitive mesh generators → `MeshData` (src/primitives.rs). 올바른 per-face normal/UV/tangent (PBR 노멀맵 호환)
- `generate_sphere` / `generate_cube` / `generate_plane` / `generate_quad` — deprecated: `(vertices, indices)` 튜플을 반환하는 구 API

### IBL
- `IblResources` (src/ibl.rs)
- SH irradiance — `compute_sh_coefficients`, `pack_sh_coefficients`, `evaluate_sh_cpu` (src/sh.rs). `evaluate_sh_cpu` 는 CPU 참조 구현으로 테스트에서만 호출된다
- BRDF LUT compute (src/brdf_lut.rs + brdf_lut_compute.wgsl)

### Animation / Skinning
- `AnimationPlayer`, `Quat`, `sample_channel`, `build_trs`, `mat4_mul` (src/animation.rs)

### Deferred
- `GBuffer` (src/gbuffer.rs)
- `FullscreenVertex`, `create_fullscreen_vertex_buffer` (src/fullscreen_quad.rs)
- `create_gbuffer_pipeline`, `create_lighting_pipeline` 등 (src/deferred_pipeline/: `*_pipeline.rs` — gbuffer, lighting, ssgi, rt_shadow, bloom, tonemap. 같은 이름의 최상위 리소스 모듈과 구분하려고 접미사)

### RT (Ray Tracing)
- `RtAccel`, `RtInstance`, `BlasMeshData` (src/rt_accel.rs)
- `RtShadowResources`, `RT_SHADOW_FORMAT` (src/rt_shadow.rs)
- `RtReflections`, `RtAo`, `RtPointShadow` (src/rt_*.rs)

### SSGI
- `SsgiResources`, `SsgiUniform`, `SSGI_OUTPUT_FORMAT` (src/ssgi.rs)

### Post-process
- `HdrTarget`, `HDR_FORMAT` (src/hdr.rs)
- `BloomResources`, `BloomUniform`, `mip_sizes`, `BLOOM_MIP_COUNT` (src/bloom.rs)
- `TonemapUniform`, `aces_tonemap` (src/tonemap.rs)
- `AutoExposure` (src/auto_exposure.rs)
- `Taa` (src/taa.rs)
- `Ssr` (src/ssr.rs)
- `MotionBlur` (src/motion_blur.rs)
- `DepthOfField` (src/dof.rs)

### Experimental (`experimental::*`, src/experimental/)
구현·단위 테스트는 되어 있지만 엔진 렌더러(BasicRenderer, 예제, voltex_capture)가 아직 쓰지 않는 모듈. API가 바뀌거나 없어질 수 있다. 옛 경로(`voltex_renderer::light_probes`, `voltex_renderer::ForwardPass` 등)는 src/deprecated_paths.rs 의 deprecated 별칭으로 남아 있다.
- `forward_pass` — `ForwardPass`(알파 블렌딩 투명 패스, HDR 타겟, depth read-only) + `sort_transparent_back_to_front` (+ forward_shader.wgsl)
- `instancing` — `InstanceData`, `InstanceBuffer`, `create_instanced_pipeline` (+ instanced_shader.wgsl)
- `gbuffer_compress` — 옥타헤드럴 노멀 인코딩 + depth 기반 위치 복원
- `light_volumes` — `LightVolume` (Sphere/Cone/Fullscreen)
- `stencil_opt` — 라이트 볼륨 스텐실 mark/test 상태
- `light_probes` — `LightProbe`, `LightProbeGrid` (SH 평가 + nearest)
- `blas_update` — `BlasTracker` (dirty mesh 추적)
- `rt_fallback` — `RtCapabilities`, `RenderMode`, `select_render_mode`
- `soft_rt_shadow` — `SoftShadowParams`, `penumbra_size`
- `bilateral_bloom` — luminance 기반 bilateral bloom weight
- `bilateral_blur` — `BilateralBlur` 컴퓨트 패스 (+ bilateral_blur.wgsl)
- `half_res_ssgi` — 반해상도 + depth-aware 업스케일 가중치
- `temporal_accum` — `TemporalAccumulation` 컴퓨트 패스 (+ temporal_accum.wgsl)

### Pipeline
- `pipeline::create_position_color_pipeline` — `PositionColorVertex` 무조명 파이프라인(triangle 예제, src/position_color.wgsl). 구 이름 `create_render_pipeline` 은 deprecated
- `pipeline::create_colored_mesh_pipeline` / `create_skinned_mesh_pipeline` / `create_skinned_shadow_pipeline` (src/pipeline.rs)
- 화면 공간 compute 패스(SSR·TAA·DoF·RT AO 등)는 `compute::dispatch_screen`(crate-private, 16×16 workgroup — 각 .wgsl 의 `@workgroup_size` 와 일치해야 함)으로 디스패치

### Error
- `RendererError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조

대분류 (자세한 항목은 lib.rs의 `pub mod`/`pub use` 참조):
- 코어 GPU: gpu, vertex, mesh, camera, texture, sphere, fullscreen_quad
- 이미지/모델: voltex_image 재export (png, jpg, progressive_jpeg, deflate, obj, gltf, json_parser, bmp — 실제 파일은 voltex_image에 있음)
- 머티리얼/조명/그림자: material, light, shadow, csm, point_shadow, spot_shadow, frustum, shadow_pipeline, pbr_pipeline
- IBL: sh, brdf_lut, ibl
- 애니메이션: animation, skinned_vertex, skeleton (조인트 계층→bone palette)
- 디퍼드: gbuffer, deferred_pipeline
- RT: rt_accel, rt_shadow, rt_reflections, rt_ao, rt_point_shadow
- SSGI: ssgi
- 포스트: hdr, bloom, tonemap, auto_exposure, taa, ssr, motion_blur, dof
- experimental (엔진 미사용): forward_pass, instancing, gbuffer_compress, light_volumes, stencil_opt, light_probes, blas_update, rt_fallback, soft_rt_shadow, bilateral_bloom, bilateral_blur, half_res_ssgi, temporal_accum
- 셰이더: 각종 .wgsl 파일 (mesh, pbr, shadow, deferred, ssgi, rt, bloom, taa, ssr, motion_blur, dof, auto_exposure, brdf_lut_compute, sprite, tilemap, skinned). experimental 셰이더(forward, instanced, bilateral_blur, temporal_accum)는 src/experimental/ 에 있다

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
- Blinn-Phong 메시 셰이더는 mesh_shader.wgsl(`create_mesh_pipeline`, model_viewer 등)과 mesh_colored.wgsl(BasicRenderer)이 있다. PBR/Deferred path는 별도. triangle 예제는 position_color.wgsl

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_renderer --lib`
- 현재 테스트 개수: 180개 (skeleton +6). 스킨드 GPU 시각 검증은 voltex_capture::scene::skinned (#[ignore] GPU-gated)
