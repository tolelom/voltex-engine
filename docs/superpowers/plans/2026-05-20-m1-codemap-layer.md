# M1: CODEMAP 레이어 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 11개 crate에 `CODEMAP.md` 추가하고 루트 `CLAUDE.md`를 보강하여, AI 어시스턴트가 코드베이스 위치/책임/의존성을 1회 조회로 파악 가능하게 한다.

**Architecture:** Spec(`docs/superpowers/specs/2026-05-20-engine-restructure-design.md`) Layer 1을 구현. 코드 변경 없음. 각 crate별 고정 템플릿 CODEMAP.md + 루트 CLAUDE.md에 의존성 그래프/용어집/공통 패턴 인덱스 추가.

**Tech Stack:** Markdown (구현 언어 없음). 검증은 `grep`, `find`, 그리고 spec의 검증 게이트.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 1, 성공기준 #1, 마일스톤 M1.

**의존성 그래프 (확정)**:
```
L0 (no voltex deps): voltex_math, voltex_asset, voltex_net
L1 (uses math only): voltex_platform, voltex_ecs, voltex_audio
L2: voltex_ai (math+ecs), voltex_physics (math+ecs), voltex_script (math+ecs),
    voltex_renderer (math+platform)
L3: voltex_editor (math+ecs+renderer)
```

---

## CODEMAP.md 고정 템플릿 (모든 crate에 동일하게 사용)

```markdown
# voltex_<crate>

## 책임 (한 줄)
<이 crate가 해결하는 문제>

## 공개 API 진입점
- `Type` — 한 줄 설명 (src/file.rs)
- `function()` — 한 줄 설명 (src/file.rs)

## 모듈 구조
- `mod_a` (src/mod_a.rs) — 책임
- `mod_b` (src/mod_b.rs) — 책임

## 의존성
- voltex crate: <목록 또는 "없음">
- 외부 crate: <목록 또는 "없음">

## 변경 시 주의점
- <이 crate를 만지면 깨질 수 있는 다른 crate/example>
- <불변식, 제약>

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_<crate>`
- 현재 테스트 개수: N개
```

작성 시 원칙:
- "공개 API 진입점"은 `lib.rs`의 `pub use` + `pub mod` 항목 기준
- "모듈 구조"는 `src/` 안의 `.rs` 파일/디렉토리 기준
- "변경 시 주의점"은 grep으로 사용처 찾아 명시
- 모든 항목을 채울 것 (빈 항목 금지). 해당 없으면 "없음" 명시.

---

## Task 1: L0 CODEMAP (math, asset, net)

**Files:**
- Create: `crates/voltex_math/CODEMAP.md`
- Create: `crates/voltex_asset/CODEMAP.md`
- Create: `crates/voltex_net/CODEMAP.md`

- [ ] **Step 1: voltex_math CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_math/src/
cat crates/voltex_math/src/lib.rs
cat crates/voltex_math/Cargo.toml
cargo test -p voltex_math --lib 2>&1 | grep "test result"
```

`crates/voltex_math/CODEMAP.md`:

```markdown
# voltex_math

## 책임 (한 줄)
선형대수 원시 타입 — Vec2/3/4, Mat4, AABB, Ray.

## 공개 API 진입점
- `Vec2`, `Vec3`, `Vec4` — 2D/3D/4D 벡터 (src/vec2.rs, src/vec3.rs, src/vec4.rs)
- `Mat4` — 4x4 변환 행렬, transform/lookAt/perspective/orthographic/inverse (src/mat4.rs)
- `AABB` — 축 정렬 바운딩 박스 (src/aabb.rs)
- `Ray` — 광선 (origin + direction) (src/ray.rs)

## 모듈 구조
- `vec2`, `vec3`, `vec4` — 벡터 (src/vec2.rs, vec3.rs, vec4.rs)
- `mat4` — 4x4 행렬 + 변환 함수 (src/mat4.rs)
- `aabb` — AABB (src/aabb.rs)
- `ray` — Ray (src/ray.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std만 사용)

## 변경 시 주의점
- 거의 모든 crate가 의존 (ai, audio, ecs, editor, physics, platform, renderer, script)
- 시그니처/메모리 레이아웃 변경 시 GPU 셰이더(WGSL)와 정렬 깨질 수 있음 (vec3는 16바이트 padding 필요)
- Mat4 row-major vs column-major 선택은 셰이더와 일치해야 함

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_math --lib`
- 현재 테스트 개수: 37개
```

- [ ] **Step 2: voltex_asset CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_asset/src/
cat crates/voltex_asset/src/lib.rs
cat crates/voltex_asset/Cargo.toml
```

`crates/voltex_asset/CODEMAP.md`:

```markdown
# voltex_asset

## 책임 (한 줄)
에셋 핸들 + 저장소 + 비동기 로더 + 핫 리로드.

## 공개 API 진입점
- `Handle<T>` — generation 기반 에셋 핸들 (src/handle.rs)
- `AssetStorage<T>` — 타입별 저장소 (ref counting) (src/storage.rs)
- `Assets` — type-erased 다중 저장소 (src/assets.rs)
- `AssetLoader` — 워커 스레드 비동기 로딩 (src/loader.rs)
- `FileWatcher` — mtime 폴링 변경 감지 (src/watcher.rs)

## 모듈 구조
- `handle` — Handle<T> 정의 (src/handle.rs)
- `storage` — 단일 타입 저장소 (src/storage.rs)
- `assets` — 다중 타입 저장소 (src/assets.rs)
- `loader` — 비동기 로더 (src/loader.rs)
- `watcher` — 파일 변경 감지 (src/watcher.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std만 사용)

## 변경 시 주의점
- Handle<T>의 generation 필드 시맨틱 변경 시 모든 자원 추적 깨짐
- replace_in_place는 핫 리로드용 — 호출 시점에 모든 핸들이 새 버전을 보게 됨

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_asset --lib`
- 현재 테스트 개수: 22개
```

- [ ] **Step 3: voltex_net CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_net/src/
cat crates/voltex_net/src/lib.rs
```

`crates/voltex_net/CODEMAP.md`:

```markdown
# voltex_net

## 책임 (한 줄)
UDP 기반 게임 네트워킹 — 신뢰성/순서/암호화/스냅샷/예측 보간.

## 공개 API 진입점
- `NetServer` — UDP 서버 (src/server.rs)
- `NetClient` — UDP 클라이언트 (src/client.rs)
- `Packet` — 패킷 프로토콜 (src/packet.rs)
- `ReliableChannel` — sequence + ACK + 재전송 + RTT (src/reliable.rs)
- `Snapshot` — 상태 직렬화 + delta 압축 (src/snapshot.rs)
- `InterpolationBuffer` — 클라이언트 보간 (src/interpolation.rs)
- `PacketCipher`, `AuthToken` — XOR 암호화 + anti-replay + 토큰 (src/encryption.rs)
- `LagCompensation` — 히스토리 rewind (src/lag_compensation.rs)

## 모듈 구조
- `packet` — 패킷 직렬화/역직렬화 (src/packet.rs)
- `socket` — UDP 소켓 래퍼 (src/socket.rs)
- `server`, `client` — 서버/클라이언트 (src/server.rs, src/client.rs)
- `reliable` — 신뢰성 채널 (src/reliable.rs)
- `snapshot` — 스냅샷 + delta (src/snapshot.rs)
- `interpolation` — 보간 버퍼 (src/interpolation.rs)
- `encryption` — 암호화/인증 (src/encryption.rs)
- `lag_compensation` — 지연 보상 (src/lag_compensation.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std::net 사용)

## 변경 시 주의점
- Packet 와이어 포맷 변경 시 클라이언트/서버 모두 재컴파일 필요 (버전 호환 없음)
- ReliableChannel의 sequence 32-bit wraparound 고려

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_net --lib`
- 현재 테스트 개수: 47개
```

- [ ] **Step 4: 검증**

```bash
ls crates/voltex_math/CODEMAP.md crates/voltex_asset/CODEMAP.md crates/voltex_net/CODEMAP.md
```

Expected: 3개 파일 모두 존재.

각 파일에 모든 템플릿 항목이 있는지 점검:
```bash
for f in crates/voltex_math/CODEMAP.md crates/voltex_asset/CODEMAP.md crates/voltex_net/CODEMAP.md; do
  echo "=== $f ==="
  grep -E "^## " "$f"
done
```

Expected: 각 파일에 `책임`, `공개 API 진입점`, `모듈 구조`, `의존성`, `변경 시 주의점`, `테스트` 모두 출현.

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_math/CODEMAP.md crates/voltex_asset/CODEMAP.md crates/voltex_net/CODEMAP.md
git commit -m "docs(codemap): add L0 crate CODEMAPs (math, asset, net)"
```

---

## Task 2: L1 CODEMAP (platform, ecs, audio)

**Files:**
- Create: `crates/voltex_platform/CODEMAP.md`
- Create: `crates/voltex_ecs/CODEMAP.md`
- Create: `crates/voltex_audio/CODEMAP.md`

- [ ] **Step 1: voltex_platform CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_platform/src/
cat crates/voltex_platform/src/lib.rs
cat crates/voltex_platform/Cargo.toml
```

`crates/voltex_platform/CODEMAP.md`:

```markdown
# voltex_platform

## 책임 (한 줄)
윈도우/입력/게임 루프 — winit 0.30 위 얇은 래퍼.

## 공개 API 진입점
- `VoltexWindow` — winit 윈도우 래퍼 (src/window.rs)
- `InputState` — 키보드/마우스 입력 추적 (src/input.rs)
- `GameTimer` — frame timing, delta time (src/game_loop.rs)

## 모듈 구조
- `window` — VoltexWindow + winit 통합 (src/window.rs)
- `input` — InputState (src/input.rs)
- `game_loop` — GameTimer (src/game_loop.rs)

## 의존성
- voltex crate: voltex_math (cursor position 등에 Vec2 사용)
- 외부 crate: winit 0.30

## 변경 시 주의점
- winit 버전 업그레이드 시 ApplicationHandler API 변경 점검 필요
- InputState 키 코드는 winit의 PhysicalKey 기준
- 모든 GPU 컨텍스트(voltex_renderer)는 VoltexWindow의 raw window handle 필요

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_platform --lib`
- 현재 테스트 개수: 3개
```

- [ ] **Step 2: voltex_ecs CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_ecs/src/
cat crates/voltex_ecs/src/lib.rs
```

`crates/voltex_ecs/CODEMAP.md`:

```markdown
# voltex_ecs

## 책임 (한 줄)
ECS 코어 — Entity/Component/World/Query + Hierarchy + Scene 직렬화 + System Scheduler.

## 공개 API 진입점
- `World` — type-erased 저장소 + query (src/world.rs)
- `Entity` — id + generation (src/entity.rs)
- `SparseSet<T>` — 컴포넌트 저장소 (src/sparse_set.rs)
- `Transform` — 위치/회전/스케일 컴포넌트 (src/transform.rs)
- `Parent`, `Children`, `add_child`, `despawn_recursive` — 계층 (src/hierarchy.rs)
- `WorldTransform` — 부모 변환 propagation (src/world_transform.rs)
- `serialize_scene_json`/`deserialize_scene_json` — JSON 씬 (src/json.rs, src/scene.rs)
- VSCN binary format — `serialize_scene_binary` (src/binary_scene.rs)
- `ComponentRegistry` — 임의 컴포넌트 등록 기반 직렬화 (src/component_registry.rs)
- `Scheduler`, `DependencyScheduler` — 시스템 실행 (src/scheduler.rs, src/dependency_scheduler.rs)
- `Archetype` — signature 기반 그룹 저장소 (src/archetype.rs)

## 모듈 구조
- `entity` — Entity (src/entity.rs)
- `sparse_set` — SparseSet<T> (src/sparse_set.rs)
- `world` — World + query<T>/query2/3/4, With/Without 필터 (src/world.rs)
- `transform` — Transform component (src/transform.rs)
- `hierarchy` — Parent/Children + 트리 조작 (src/hierarchy.rs)
- `world_transform` — 변환 propagation (src/world_transform.rs)
- `scene`, `json`, `binary_scene` — 씬 직렬화 (src/scene.rs, src/json.rs, src/binary_scene.rs)
- `component_registry` — 등록 기반 다형 직렬화 (src/component_registry.rs)
- `scheduler`, `dependency_scheduler` — 시스템 스케줄링 (src/scheduler.rs, src/dependency_scheduler.rs)
- `archetype` — Archetype 저장 (src/archetype.rs)

## 의존성
- voltex crate: voltex_math (Vec3, Mat4 등)
- 외부 crate: 없음

## 변경 시 주의점
- ECS 인터페이스 변경은 모든 example, voltex_physics, voltex_ai, voltex_script, voltex_editor에 영향
- ComponentRegistry에 등록된 타입의 직렬화 포맷 변경 시 기존 .vscn 파일 호환 깨짐
- World의 query 시그니처 변경은 game 코드에 광범위 영향

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ecs --lib`
- 현재 테스트 개수: 104개
```

- [ ] **Step 3: voltex_audio CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_audio/src/
cat crates/voltex_audio/src/lib.rs
```

`crates/voltex_audio/CODEMAP.md`:

```markdown
# voltex_audio

## 책임 (한 줄)
오디오 — WAV/OGG 디코더, WASAPI 백엔드, 3D 사운드, 믹서, 이펙트.

## 공개 API 진입점
- `AudioSystem` — 메인 오디오 시스템 (src/audio_system.rs)
- `AudioClip` — PCM 데이터 (src/audio_clip.rs)
- `AudioSource` — ECS 컴포넌트 (src/audio_source.rs)
- `AsyncAudioLoader` — 비동기 로딩 (src/async_loader.rs)
- WAV/OGG 디코더 — `parse_wav`, `decode_ogg` (src/wav.rs, src/ogg.rs, src/vorbis.rs)
- `MixGroup`, `MixGroupManager`, `AudioBus` — 믹싱 (src/mix_group.rs, src/dynamic_groups.rs, src/audio_bus.rs)
- `EffectChain`, `Reverb`, `EchoDelay` — 이펙트 (src/effect_chain.rs, src/reverb.rs)
- spatial — `distance_attenuation`, `stereo_pan`, `doppler_shift`, HRTF (src/spatial.rs, src/hrtf.rs)
- `Occlusion` — 레이 기반 차폐 (src/occlusion.rs)

## 모듈 구조
- `audio_system` — 메인 시스템 (src/audio_system.rs)
- `audio_clip`, `audio_source` — 데이터 + 컴포넌트 (src/audio_clip.rs, src/audio_source.rs)
- `async_loader` — 비동기 로더 (src/async_loader.rs)
- `wav`, `ogg`, `vorbis` — 디코더 (src/wav.rs, src/ogg.rs, src/vorbis.rs)
- `mixing`, `mix_group`, `dynamic_groups`, `audio_bus` — 믹서 (src/mixing.rs 등)
- `effect_chain`, `reverb`, `fade_curves` — 이펙트 (src/effect_chain.rs 등)
- `spatial`, `hrtf`, `occlusion` — 3D 오디오 (src/spatial.rs 등)
- `wasapi` — Windows 백엔드 (src/wasapi.rs)

## 의존성
- voltex crate: voltex_math (3D 사운드 좌표)
- 외부 crate: 없음 (WASAPI는 Windows API 직접 사용)

## 변경 시 주의점
- WASAPI는 Windows 전용 — macOS/Linux 백엔드 미구현
- vorbis.rs는 1493줄로 매우 큼 (M2에서 분리 예정)
- AudioSource 컴포넌트는 ECS와 함께 사용

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_audio --lib`
- 현재 테스트 개수: 84개
```

- [ ] **Step 4: 검증**

```bash
for f in crates/voltex_platform/CODEMAP.md crates/voltex_ecs/CODEMAP.md crates/voltex_audio/CODEMAP.md; do
  test -f "$f" && echo "OK $f" || echo "MISSING $f"
done
```

Expected: 3개 OK.

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_platform/CODEMAP.md crates/voltex_ecs/CODEMAP.md crates/voltex_audio/CODEMAP.md
git commit -m "docs(codemap): add L1 crate CODEMAPs (platform, ecs, audio)"
```

---

## Task 3: L2 CODEMAP (ai, physics, script, renderer)

**Files:**
- Create: `crates/voltex_ai/CODEMAP.md`
- Create: `crates/voltex_physics/CODEMAP.md`
- Create: `crates/voltex_script/CODEMAP.md`
- Create: `crates/voltex_renderer/CODEMAP.md`

- [ ] **Step 1: voltex_ai CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_ai/src/
cat crates/voltex_ai/src/lib.rs
```

`crates/voltex_ai/CODEMAP.md`:

```markdown
# voltex_ai

## 책임 (한 줄)
AI 기초 — NavMesh (수동 + 자동 voxelization), A* + Funnel, steering, 동적 장애물 회피.

## 공개 API 진입점
- `NavMesh` — 폴리곤 기반 내비메시 (src/navmesh.rs)
- `NavMeshBuilder` — voxelization 기반 자동 생성 (src/navmesh_builder.rs)
- `find_path` — A* + String Pulling (Funnel) (src/pathfinding.rs)
- Steering — `seek`, `flee`, `arrive`, `wander` (src/steering.rs)
- `NavAgent` — ECS 컴포넌트 (src/nav_agent.rs)
- `Obstacle`, velocity obstacle 회피 (src/obstacle.rs)

## 모듈 구조
- `navmesh` — NavMesh 데이터 구조 + 직렬화 (src/navmesh.rs)
- `navmesh_builder` — voxelization (src/navmesh_builder.rs)
- `pathfinding` — A* + Funnel (src/pathfinding.rs)
- `steering` — steering behaviors (src/steering.rs)
- `nav_agent` — ECS 컴포넌트 (src/nav_agent.rs)
- `obstacle` — 동적 장애물 회피 (src/obstacle.rs)

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- navmesh_builder.rs는 651줄 (M3 분리 예정)
- navmesh.rs는 497줄
- VNAV 바이너리 포맷 변경 시 저장된 내비메시 호환 깨짐
- nav_agent_system은 ECS 미구현 (DEFERRED)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ai --lib`
- 현재 테스트 개수: 34개
```

- [ ] **Step 2: voltex_physics CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_physics/src/
cat crates/voltex_physics/src/lib.rs
```

`crates/voltex_physics/CODEMAP.md`:

```markdown
# voltex_physics

## 책임 (한 줄)
충돌 감지 (GJK/EPA + BVH) + 강체 동역학 (sequential impulse) + 레이캐스트.

## 공개 API 진입점
- `Collider` — Sphere/Box/Capsule/ConvexHull (src/collider.rs)
- `MeshCollider` — triangle mesh (src/mesh_collider.rs)
- `ContactPoint`, `gjk_epa` — 좁은 단계 충돌 (src/gjk.rs, src/contact.rs)
- `BVH` — 넓은 단계 + refit + query_ray (src/bvh.rs)
- `RigidBody` — 질량/속도/각속도/마찰/sleep (src/rigid_body.rs)
- `Integrator`, `Solver` — Semi-implicit Euler + Sequential Impulse (src/integrator.rs, src/solver.rs)
- raycast — `raycast`, `raycast_all`, ray_vs_aabb/sphere/box/capsule/triangle/mesh (src/raycast.rs, src/ray.rs)
- `detect_collisions` — ECS 통합 (src/collision.rs)
- CCD — `swept_sphere_vs_aabb` (src/ccd.rs)

## 모듈 구조
- `collider`, `mesh_collider` — 콜라이더 타입 (src/collider.rs, src/mesh_collider.rs)
- `gjk`, `narrow`, `contact` — 좁은 단계 (src/gjk.rs 등)
- `bvh` — 넓은 단계 (src/bvh.rs)
- `rigid_body`, `integrator`, `solver` — 강체 동역학 (src/rigid_body.rs 등)
- `ray`, `raycast` — 레이 인터섹션 (src/ray.rs, src/raycast.rs)
- `collision` — ECS 통합 (src/collision.rs)
- `ccd` — Continuous Collision Detection (src/ccd.rs)

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- solver.rs(574줄), gjk.rs(524줄)는 M3 분리 예정
- detect_collisions(world)는 ECS와 강하게 결합 — World 구조 변경 시 영향
- CCD는 sphere vs AABB만 구현 (다른 형상은 DEFERRED)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_physics --lib`
- 현재 테스트 개수: 106개
```

- [ ] **Step 3: voltex_script CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_script/src/
cat crates/voltex_script/src/lib.rs
```

`crates/voltex_script/CODEMAP.md`:

```markdown
# voltex_script

## 책임 (한 줄)
Lua 5.4 내장 + 엔진 API 바인딩 + 샌드박싱 + 핫 리로드.

## 공개 API 진입점
- `LuaState` — Lua 인터프리터 (src/state.rs)
- `LuaCoroutine` — 코루틴 (src/coroutine.rs)
- `Sandbox` — os/io 차단 (src/sandbox.rs)
- engine_api — spawn/position/velocity/rotation/scale/destroy/find_by_tag (src/engine_api.rs)
- bindings — voltex_print 등 (src/bindings.rs)
- ffi — Lua C API FFI (src/ffi.rs)

## 모듈 구조
- `state` — LuaState + push/read table/vec3, register_fn, exec (src/state.rs, 505줄 — M3)
- `coroutine` — LuaCoroutine (src/coroutine.rs)
- `sandbox` — 샌드박싱 (src/sandbox.rs)
- `engine_api` — Rust ↔ Lua 엔진 API (src/engine_api.rs)
- `bindings` — 표준 라이브러리 바인딩 (src/bindings.rs)
- `ffi` — Lua C FFI 선언 (src/ffi.rs)

## 의존성
- voltex crate: voltex_math (Vec3), voltex_ecs (Entity, World)
- 외부 crate: cc 1 (빌드 타임 Lua 컴파일)

## 변경 시 주의점
- engine_api는 현재 4개 함수만 노출 (DEFERRED 확장)
- Entity.generation은 하드코딩 0 버그 있음 (DEFERRED)
- state.rs는 505줄 (M3 분리 예정)
- Lua 5.4는 cc로 내장 빌드 — Windows MSVC 환경 의존

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_script --lib`
- 현재 테스트 개수: 18개
```

- [ ] **Step 4: voltex_renderer CODEMAP 작성**

수집 정보 (큰 crate이므로 lib.rs 전체):
```bash
ls crates/voltex_renderer/src/
wc -l crates/voltex_renderer/src/lib.rs
cat crates/voltex_renderer/src/lib.rs
```

`crates/voltex_renderer/CODEMAP.md` — 다른 CODEMAP보다 큰 crate라 그룹화하여 작성:

```markdown
# voltex_renderer

## 책임 (한 줄)
wgpu 기반 렌더링 — 메시/PBR/그림자/IBL/디퍼드/RT/post-process + 이미지 디코더(PNG/JPG) + glTF 파서.

## 공개 API 진입점

### Core GPU
- `GpuContext` — wgpu 디바이스/큐/surface (src/gpu_context.rs)
- `Vertex`, `MeshVertex`, `SkinnedMeshVertex` — 정점 포맷 (src/mesh.rs, src/skinned.rs)
- `Mesh` — 메시 데이터 (src/mesh.rs)
- `Camera`, `FpsController` — 카메라 (src/camera.rs)
- `GpuTexture` — GPU 텍스처 + BMP/PNG/JPG 로딩 (src/texture.rs)

### Image / Model
- PNG, JPG 디코더 (src/png.rs, src/jpg.rs)
- `parse_obj` — OBJ 파서 (src/obj.rs)
- `parse_gltf` — glTF 2.0/GLB (src/gltf.rs)
- `deflate` — zlib/deflate (src/deflate.rs)

### Materials / Lighting / Shadows
- `MaterialUniform`, PBR 텍스처 레이아웃 (src/material.rs)
- `LightData`, `LightsUniform` — Directional/Point/Spot, MAX_LIGHTS=16 (src/lights.rs)
- `ShadowMap`, `CascadedShadowMap`, `PointShadowMap`, `SpotShadowMap` (src/shadow.rs, src/csm.rs 등)
- `Frustum` — 라이트 컬링 (src/frustum.rs)

### IBL
- SH irradiance (L2, 9 coeffs) (src/sh.rs)
- BRDF LUT compute (src/brdf_lut.rs + brdf_lut_compute.wgsl)
- Sky (Hosek-Wilkie) (src/sky.rs)

### Animation / Skinning
- `BonePalette`, `Animation`, keyframe sampling, SLERP, bone matrix (src/animation.rs)

### Deferred / RT / Post-process
- `DeferredPipeline` (src/deferred_pipeline.rs, 905줄 — M2)
- `BlasTracker`, RT shadows/reflections/AO (src/blas_update.rs 등)
- Post: bloom, TAA, SSR, motion blur, DOF, auto exposure, bilateral blur (각 모듈)

## 모듈 구조

대분류 (자세한 항목은 ls 결과 참조):
- 코어: gpu_context, mesh, camera, texture
- 이미지/모델: png, jpg, deflate, obj, gltf
- 머티리얼/조명/그림자: material, lights, shadow, csm, frustum
- IBL: sh, brdf_lut, sky
- 애니메이션: animation
- 디퍼드/RT: deferred_pipeline, deferred_lighting, blas_update, rt_*
- 포스트: bloom, taa, ssr, motion_blur, dof, auto_exposure, bilateral_*
- 셰이더: 각종 .wgsl 파일

## 의존성
- voltex crate: voltex_math, voltex_platform
- 외부 crate: wgpu 28.0, winit 0.30 (간접), bytemuck

## 변경 시 주의점
- max_bind_groups=4 — 리소스를 합쳐 4개 이내로 유지
- WGSL vec3 alignment 16바이트 (Rust struct에 padding 필요)
- wgpu 28.0 전용 필드: immediate_size, multiview_mask, TexelCopyTextureInfo
- gltf.rs (982줄), jpg.rs (959줄), deferred_pipeline.rs (905줄), texture.rs (514줄)는 M2/M3 분리 예정
- 이미지 디코더/glTF 파서는 향후 voltex_image 같은 별도 crate로 분리 고려 (DEFERRED)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_renderer --lib`
- 현재 테스트 개수: 200개
```

- [ ] **Step 5: 검증**

```bash
for f in crates/voltex_ai/CODEMAP.md crates/voltex_physics/CODEMAP.md crates/voltex_script/CODEMAP.md crates/voltex_renderer/CODEMAP.md; do
  test -f "$f" && echo "OK $f" || echo "MISSING $f"
done
```

Expected: 4개 OK.

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_ai/CODEMAP.md crates/voltex_physics/CODEMAP.md crates/voltex_script/CODEMAP.md crates/voltex_renderer/CODEMAP.md
git commit -m "docs(codemap): add L2 crate CODEMAPs (ai, physics, script, renderer)"
```

---

## Task 4: L3 CODEMAP (editor)

**Files:**
- Create: `crates/voltex_editor/CODEMAP.md`

- [ ] **Step 1: voltex_editor CODEMAP 작성**

수집 정보:
```bash
ls crates/voltex_editor/src/
cat crates/voltex_editor/src/lib.rs
```

`crates/voltex_editor/CODEMAP.md`:

```markdown
# voltex_editor

## 책임 (한 줄)
Immediate-mode UI 에디터 — 도킹/플로팅/뷰포트/인스펙터/에셋 브라우저 + TTF 렌더링.

## 공개 API 진입점

### 코어 UI
- `UiContext` — IMGUI 컨텍스트 (src/ui_context.rs)
- `UiRenderer` — wgpu UI 렌더러 (src/renderer.rs)
- `DrawList`, `DrawVertex`, `DrawCommand` (src/draw_list.rs)
- `FontAtlas` — 8x12 비트맵 ASCII (src/font.rs)
- `LayoutState` — 레이아웃 (src/layout.rs)
- `TtfFont`, TTF 파서/래스터라이저/캐시 (src/ttf_font.rs, src/ttf_parser.rs, src/rasterizer.rs, src/glyph_cache.rs)

### Widgets
- text, button, slider, checkbox, panel, text_input, scroll_panel, drag/drop (src/widgets.rs, 745줄 — M3)

### Layout
- `DockTree`, `DockNode`, `Axis`, `Rect`, `LeafLayout` — 도킹 (src/dock.rs, 558줄 — M3)
- `FloatingPanel`, `FloatingPanelManager` (src/floating_panel.rs)

### 3D 뷰포트
- `OrbitCamera` (src/orbit_camera.rs)
- `ViewportTexture`, `ViewportRenderer` (src/viewport_texture.rs, src/viewport_renderer.rs)

### Panels
- `hierarchy_panel`, `inspector_panel` (Rect/slider 기반, World 직접 변형) (src/inspector.rs)
- `panels::hierarchy`, `panels::inspector` (text_input 기반, EditorSession dispatch) (src/panels/)
- `AssetBrowser`, `asset_browser_panel` (src/asset_browser.rs)

### Editor Command
- `EditorCommand`, `CommandError` — Spawn/Despawn/SetTransform/SetTag/SaveScene/LoadScene (src/command.rs)
- `EditorSession` — World + ComponentRegistry + 자동저장 + jsonl 로그 (src/session.rs)

## 모듈 구조
- 코어: ui_context, renderer, draw_list, font, layout, ttf_*
- 위젯: widgets
- 레이아웃: dock, floating_panel
- 뷰포트: orbit_camera, viewport_texture, viewport_renderer
- 패널: inspector (구), panels/hierarchy, panels/inspector (신), asset_browser
- 커맨드: command, session

## 의존성
- voltex crate: voltex_math, voltex_ecs, voltex_renderer
- 외부 crate: wgpu, bytemuck

## 변경 시 주의점
- 두 인스펙터 공존 중: `inspector::*` (구, slider 기반 — World 직접 변형) vs `panels::inspector::*` (신, text_input 기반 — EditorSession dispatch). 통합 미결정 (STATUS.md 참고)
- examples/editor_demo는 두 가지 인스펙터를 모두 사용
- widgets.rs (745줄), dock.rs (558줄), ttf_parser.rs (518줄)는 M3 분리 예정
- 탭 드래그 이동, 커스텀 컴포넌트 인스펙터 리플렉션, 에셋 프리뷰는 DEFERRED

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_editor --lib`
- 현재 테스트 개수: 105개
```

- [ ] **Step 2: 검증**

```bash
test -f crates/voltex_editor/CODEMAP.md && echo "OK" || echo "MISSING"
grep -c "^## " crates/voltex_editor/CODEMAP.md
```

Expected: OK, `## ` 헤더 6개 이상.

- [ ] **Step 3: Commit**

```bash
git add crates/voltex_editor/CODEMAP.md
git commit -m "docs(codemap): add L3 crate CODEMAP (editor)"
```

---

## Task 5: 루트 CLAUDE.md 보강

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: 현재 CLAUDE.md 확인**

```bash
cat CLAUDE.md
```

현재 내용 (참고):
- 프로젝트 소개 + wgpu/winit 버전
- 작업 현황 (STATUS.md, DEFERRED.md 링크)
- 스펙, 구현 계획 위치
- 빌드/테스트 명령
- wgpu/WGSL 관련 주의사항

- [ ] **Step 2: CLAUDE.md 보강**

기존 내용을 보존하면서 아래 4개 섹션 추가:

`CLAUDE.md` 전체 내용:

```markdown
# Voltex Engine

Rust 게임 엔진 프로젝트. wgpu 28.0 + winit 0.30 기반.

## 작업 현황
- `docs/STATUS.md` — 완료된 Phase, crate 구조, 테스트 현황, 다음 작업
- `docs/DEFERRED.md` — 간소화/미뤄진 항목 목록

## 스펙
- `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` — 전체 엔진 설계
- `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` — AI 친화 재구조화 설계

## 구현 계획
- `docs/superpowers/plans/` — 각 Phase별 상세 구현 계획

## 빌드/테스트
```bash
cargo build --workspace
cargo test --workspace
```

## 규칙
- Cargo path: `export PATH="$HOME/.cargo/bin:$PATH"` (Windows bash)
- wgpu 28.0 API: `immediate_size`, `multiview_mask`, `TexelCopyTextureInfo` 등 28.0 전용 필드 사용
- WGSL vec3 alignment: Rust struct에서 vec3 뒤에 padding 필요 (16바이트 정렬)
- max_bind_groups=4 (group 0~3). 리소스를 합쳐서 4개 이내로 유지
- Dynamic UBO 패턴: per-entity uniform은 aligned staging buffer + dynamic offset 사용
- 기존 예제(triangle, model_viewer 등)는 mesh_shader.wgsl(Blinn-Phong) 사용 — PBR 변경에 영향 없음

## Crate 의존성 그래프

각 crate의 자세한 책임/API는 `crates/<crate>/CODEMAP.md` 참고.

```
voltex_math       (L0, no voltex deps)
voltex_asset      (L0, no voltex deps)
voltex_net        (L0, no voltex deps)

voltex_platform   (L1) → voltex_math
voltex_ecs        (L1) → voltex_math
voltex_audio      (L1) → voltex_math

voltex_ai         (L2) → voltex_math, voltex_ecs
voltex_physics    (L2) → voltex_math, voltex_ecs
voltex_script     (L2) → voltex_math, voltex_ecs
voltex_renderer   (L2) → voltex_math, voltex_platform

voltex_editor     (L3) → voltex_math, voltex_ecs, voltex_renderer
```

**변경 영향도**: L0 변경은 거의 모든 crate에 영향. L3 변경은 영향 적음. 변경 전 의존 그래프를 따라 영향 범위 확인.

## 도메인 용어집

- **Entity**: ECS의 식별자. `(id: u32, generation: u32)` 쌍. 재사용 안전.
- **Component**: 데이터 단위. Transform, Tag, Parent 등. SparseSet<T>에 저장.
- **System**: 게임 로직 단위. `Scheduler`가 순차 실행. (의존성 기반 병렬은 `DependencyScheduler`)
- **World**: 모든 엔티티/컴포넌트를 담는 컨테이너 (`voltex_ecs::World`).
- **Handle<T>**: 에셋 핸들. generation 기반 안전 참조 (`voltex_asset`).
- **Collider**: 충돌 형상. Sphere/Box/Capsule/ConvexHull/Mesh.
- **RigidBody**: 강체. 질량 + 속도 + 각속도 + 마찰 + sleep 상태.
- **NavMesh**: 폴리곤 기반 내비메시 (`voltex_ai`).
- **DockTree**: 에디터 도킹 트리 (binary split layout) (`voltex_editor`).
- **EditorCommand**: 에디터 작업 단위. Spawn/Despawn/SetTransform 등. EditorSession에 dispatch.
- **CODEMAP.md**: 각 crate 루트의 메타데이터 파일. 책임/API/의존성/주의점.

## 공통 패턴 인덱스

### 에러 처리 (M4 표준화 예정)
현재 crate마다 다름. M4 이후: `voltex_<crate>::<Crate>Error` (thiserror 기반).

### 컴포넌트 등록 (M5 표준화 예정)
현재: `ComponentRegistry::register::<T>()`. 자세한 사용은 `crates/voltex_ecs/CODEMAP.md`.

### 시스템 정의 (M5 표준화 예정)
현재: `System` trait + `Scheduler`. 의존성 기반은 `DependencyScheduler`.

### 직렬화
- JSON: `voltex_ecs::serialize_scene_json` / `deserialize_scene_json`
- VSCN binary: `voltex_ecs::serialize_scene_binary` / `deserialize_scene_binary`
- VNAV (navmesh): `voltex_ai`의 navmesh 직렬화

### 에디터 작업
- 사용자 액션은 `EditorCommand`로 표현
- `EditorSession::dispatch(command)` 호출
- World 직접 변형은 가급적 피하고 command 경유 (jsonl 로그 + 자동 저장 혜택)

## 변경 워크플로 (AI 작업 기준)

1. **위치 파악**: 수정할 기능이 어느 crate에 있는지 `CLAUDE.md` 의존성 그래프 → 해당 `CODEMAP.md` → 모듈 구조에서 찾기
2. **변경 영향 점검**: `CODEMAP.md`의 "변경 시 주의점" 확인
3. **수정**: 단일 책임 원칙, 600줄 한도, 일관된 패턴
4. **빌드 확인**: `cargo build -p <crate>` (해당 crate만) 후 `cargo build --workspace`
5. **테스트 확인**: `cargo test -p <crate> --lib` 후 `cargo test --workspace --lib`
6. **CODEMAP 갱신**: 공개 API/모듈 구조가 바뀌었으면 해당 `CODEMAP.md` 갱신
7. **Commit**: 단일 책임 단위로 commit (모듈 분리/에러 마이그레이션은 atomic)
```

- [ ] **Step 3: 검증**

```bash
grep -E "^## (Crate 의존성|도메인 용어집|공통 패턴|변경 워크플로)" CLAUDE.md
```

Expected: 4개 헤더 모두 출현.

```bash
wc -l CLAUDE.md
```

Expected: 100줄 이상.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): add dependency graph, glossary, patterns, workflow"
```

---

## Task 6: 최종 검증

**Files:** (없음 — 검증만)

- [ ] **Step 1: 11개 CODEMAP 존재 확인**

```bash
for c in math asset net platform ecs audio ai physics script renderer editor; do
  f="crates/voltex_$c/CODEMAP.md"
  test -f "$f" && echo "OK $f" || echo "MISSING $f"
done
```

Expected: 11개 모두 OK.

- [ ] **Step 2: 각 CODEMAP의 모든 템플릿 항목 존재 확인**

```bash
for c in math asset net platform ecs audio ai physics script renderer editor; do
  f="crates/voltex_$c/CODEMAP.md"
  echo "=== $f ==="
  for section in "책임" "공개 API 진입점" "모듈 구조" "의존성" "변경 시 주의점" "테스트"; do
    grep -q "## $section" "$f" && echo "  OK $section" || echo "  MISSING $section"
  done
done
```

Expected: 11 crate × 6 섹션 = 66 항목 모두 OK.

- [ ] **Step 3: 루트 CLAUDE.md 보강 확인**

```bash
for section in "Crate 의존성 그래프" "도메인 용어집" "공통 패턴 인덱스" "변경 워크플로"; do
  grep -q "## $section" CLAUDE.md && echo "OK $section" || echo "MISSING $section"
done
```

Expected: 4개 모두 OK.

- [ ] **Step 4: 빌드/테스트 게이트 (regression 없음)**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --workspace 2>&1 | tail -3
cargo test --workspace --lib 2>&1 | grep -E "^test result" | grep -v "0 passed"
```

Expected:
- 빌드 성공 (`Finished` 메시지)
- `test result: ok. N passed` 형태 11줄 (각 crate 1줄, 0 passed가 아닌 줄만)
- 실패(`FAILED`) 없음

전체 카운트 합계 확인:
```bash
cargo test --workspace --lib 2>&1 | grep -E "^test result" | awk '{sum += $4} END {print sum}'
```
Expected: 760

- [ ] **Step 5: 푸시**

```bash
git log --oneline -10
git push origin master
```

Expected: 8개 정도의 커밋이 푸시됨 (각 CODEMAP 그룹 + CLAUDE.md).

---

## 완료 기준 (Definition of Done)

- [ ] 11개 `crates/<name>/CODEMAP.md` 존재 + 6개 섹션 모두 채워짐
- [ ] 루트 `CLAUDE.md`에 4개 새 섹션 추가 (의존성 그래프, 용어집, 공통 패턴, 워크플로)
- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace --lib` 760개 통과 (regression 0)
- [ ] M2~M7는 다음 plan에서 진행
