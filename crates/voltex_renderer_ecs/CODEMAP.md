# voltex_renderer_ecs

ECS World ↔ voltex_renderer 브릿지. C0a 의 `MeshHandle` / `MaterialHandle` 컴포넌트를 실제 GPU 자원과 연결한다.

## 책임

- `voltex_renderer::Mesh` 와 `MaterialUniform` 을 `RenderAssets` 에 저장
- C0a 의 phantom-typed `Handle<MeshAsset>` / `Handle<MaterialAsset>` 과 내부 `Handle<Mesh>` / `Handle<MaterialUniform>` 의 id/generation lockstep 유지
- `BasicRenderer` (mesh_shader.wgsl, Blinn-Phong) 로 한 메서드 호출로 World 를 그림
- `collect_render_items` 로 자체 pipeline 호출자(survivor_game PBR loop) 를 위한 RenderItem 리스트 생성

## 공개 API

| 항목 | 파일 | 메모 |
|---|---|---|
| `RenderAssets::new()` / `add_mesh(Mesh) -> MeshHandle` / `mesh(MeshHandle) -> Option<&Mesh>` | `assets.rs` | 머티리얼도 동일 패턴 (`add_material` / `material`) |
| `RenderItem { entity, model, mesh, material, texture }` | `items.rs` | per-frame 데이터 단위 (texture: Option<TextureHandle>) |
| `collect_render_items(&mut World, &RenderAssets) -> Vec<RenderItem>` | `items.rs` | `propagate_transforms` 자동 호출 |
| `BasicRenderer::new(device, queue, target_format, max_entities)` | `basic_renderer.rs` | mesh_shader.wgsl pipeline 소유 |
| `BasicRenderer::render(queue, encoder, target_view, depth_view, clear_color, &mut World, &RenderAssets, view_proj, cam_pos, light)` | `basic_renderer.rs` | 메시 패스(colored Blinn-Phong 또는 `set_pbr_enabled` 시 PBR) + 스킨드 패스(colored 또는 PBR) + 스프라이트/파티클/디버그 라인 패스. PBR 시 메시·스킨드 모두 Cook-Torrance |
| `BasicRenderer::set_point_lights(&queue, &[PointLight])` / `set_shadows(bool)` / `set_debug_lines(&queue, &[LineVertex])` | `basic_renderer.rs` | 매 프레임 호출(`&self`). 디버그 라인은 `LineList` depth-tested 마지막 패스 (MAX 8192 verts, 초과 drop) |
| `Sprite { texture, size, color, uv_rect }` + `Sprite::solid` / `textured` / `with_uv_rect(rect)` / `set_sheet_frame(cols,rows,index)` | `sprite.rs` | 카메라-facing 빌보드 컴포넌트. `WorldTransform`+`Sprite` 엔티티를 mesh 패스 후 alpha-blended·depth-tested(no write) 빌보드로 렌더. `uv_rect`로 스프라이트시트/아틀라스 프레임 선택. 카메라 right/up은 inverse VP에서 복원. 카메라 거리 기준 back-to-front 정렬(투명도 합성 순서 무관). MAX 256/frame |
| `Particle { velocity, acceleration, age, lifetime, start/end_size, start/end_alpha }` + `Particle::new(vel,lifetime,size)` / `spawn_particle(world,pos,color,particle)` / `update_particles(&mut World, dt)` | `particle.rs` | CPU 파티클(=Transform+Sprite+Particle 엔티티). update가 이동·크기/알파 보간·수명 만료 despawn. RuntimeApp tick이 `update_particles` 자동 호출 |
| `SpriteAnimation { cols, rows, frame_count, fps, time, playing, looping }` + `SpriteAnimation::new(cols,rows,frames,fps)` / `update_sprite_animations(&mut World, dt)` | `sprite.rs` | 스프라이트시트 애니메이션. Sprite+SpriteAnimation 엔티티의 `uv_rect`를 시간에 따라 프레임 진행(loop=wrap, else clamp+stop). RuntimeApp tick 자동 호출 |
| `PrimitiveMesh` (`Cube`/`Plane`/`Quad`/`Sphere`) + `cube`/`plane`/`quad`/`sphere`/`sphere_default` / `aabb()` / `geometry()` | `renderable.rs` | 값-타입 모양 선언(GPU 무관). `Component` → 씬 직렬화. 빌트인 generator 위임 |
| `MaterialDesc(MaterialUniform)` + `color(rgba)` / `pbr(rgba,metallic,roughness)` | `renderable.rs` | 값-타입 머티리얼 선언. `Component` → 씬 직렬화 |
| `materialize_renderables(&mut World, &mut RenderAssets, &Device)` | `renderable.rs` | `PrimitiveMesh`/`MaterialDesc` → GPU 메시/머티리얼 + `MeshHandle`/`MaterialHandle`/`Bounds` 부착. **dedup**(동일 프리미티브·머티리얼 공유) + **멱등**(resolve 된 엔티티 skip). RuntimeApp 이 gpu_init 직후 자동 호출 |
| `register_renderable_components(&mut ComponentRegistry)` / `plan_primitive_meshes(&World)` / `plan_materials(&World)` | `renderable.rs` | 등록 헬퍼 + 순수(GPU 무관) plan 헬퍼(멱등 계약 테스트용) |
| `RenderAssets::add_skinned_mesh(SkinnedMesh) -> SkinnedMeshHandle` / `skinned_mesh(h)` | `assets.rs` | 스킨드 메시 저장 |
| `RenderAssets::add_texture(GpuTexture)` / `add_texture_png(device,queue,bytes) -> Result` / `add_texture_rgba(device,queue,w,h,px)` / `texture(h)`, `TextureHandle` 컴포넌트 | `assets.rs` | per-entity 디퓨즈 텍스처 (없으면 흰색 기본). `_png`/`_rgba`는 레이아웃 생성+업로드를 한 번에 (런타임 스프라이트/메시 텍스처 로딩 편의) |
| `SkinnedMeshHandle(Handle<SkinnedMesh>)`, `SkinnedPose{skeleton,animation,player}` | `skinned.rs` | 스킨드 ECS 컴포넌트 |
| `update_skinned_poses(&mut World, dt)`, `animation_duration(&GltfAnimation)` | `skinned.rs` | 포즈 진행 시스템 (RuntimeApp tick 호출) |
| `load_skinned_gltf(device, &mut assets, &GltfData) -> Option<(SkinnedMeshHandle, SkinnedPose)>` | `skinned.rs` | glTF(첫 mesh+skin+anim) → 스킨드 엔티티 컴포넌트 |
| `RendererEcsError`, `Result<T>` | `error.rs` | M4 표준 패턴 |

## 의존성

- voltex_math — `Mat4`, `Vec3`
- voltex_ecs — `World`, `Entity`, `MeshHandle`, `MaterialHandle`, `WorldTransform`, `propagate_transforms`
- voltex_asset — `Handle`, `Assets`
- voltex_renderer — `Mesh`, `MaterialUniform`, `CameraUniform`, `LightUniform`, `GpuTexture`, `pipeline::create_mesh_pipeline`
- wgpu, bytemuck, thiserror

## 모듈 구조

```
src/
├── lib.rs              # module decls + re-exports
├── error.rs            # RendererEcsError + Result<T>
├── assets.rs           # RenderAssets
├── items.rs            # RenderItem + collect_render_items
├── basic_renderer.rs   # BasicRenderer (비스킨 + 스킨드 패스)
├── renderable.rs       # PrimitiveMesh / MaterialDesc / materialize_renderables (선언적 렌더러블)
├── particle.rs         # Particle / spawn_particle / update_particles
├── sprite.rs           # Sprite / SpriteAnimation / update_sprite_animations
└── skinned.rs          # SkinnedMeshHandle / SkinnedPose / update_skinned_poses
```

각 파일 600줄 미만. lib.rs 는 module/use 만.

## 변경 시 주의점

- `RenderAssets::add_mesh` / `add_material` 외 경로로 핸들을 만들면 id 불일치 위험. `Handle::from_raw_parts` 의 직접 사용은 scene deserialize 가 유일한 정당 사용처.
- `BasicRenderer::new(max_entities)` 보다 많은 엔티티가 World 에 있으면 초과분은 silently drop. v1 한도 (수십~수백) 에서는 무시 가능하지만 외부 사용 시 `max_entities` 적절히 설정.
- `BasicRenderer` 의 pipeline 은 mesh_shader.wgsl (Blinn-Phong). PBR / 멀티라이트 / 그림자 사용 시 자체 pipeline + `collect_render_items` 패턴 사용 (survivor_game 참조).
- `propagate_transforms` 가 매 `collect_render_items` 호출마다 실행. 한 프레임에 여러 번 부르면 비용 누적 → 매 프레임 한 번만 호출 권장.
- max_bind_groups=4 제약: BasicRenderer 비스킨 패스는 2개 (cam+light, texture), 스킨드 패스는 2개 (skinned cam, bone palette). 외부 추가 bind group 시점 점검.
- 스킨드 패스: `SkinnedMeshHandle`+`SkinnedPose`+`WorldTransform` 엔티티를 `LoadOp::Load` 2차 패스로. per-entity palette(8KB)/cam dynamic-offset UBO, `MAX_SKINNED=64` 캡(VRAM 바운드). 포즈 진행은 `update_skinned_poses`(런타임 tick)가 담당 — 렌더는 `palette()` 만 읽음. PBR 스키닝은 미구현.
- headless 단위 테스트는 GPU device 가 필요해서 `add_mesh` 라운드트립 / `collect_render_items` 의 emit 케이스 직접 검증 불가 → editor_demo 시각 검증으로 위임.
