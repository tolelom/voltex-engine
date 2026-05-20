# Cycle 1 (C0a) — ECS rendering 컴포넌트 설계

**작성일**: 2026-05-20
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 1
**Sub-project**: C (Core bridge)
**세션 수**: 1

---

## 1. 목적

ECS World가 "이 엔티티는 어떤 메시/머티리얼로 그릴지" + "월드 공간 AABB 캐시" 를 표현할 수 있게 한다. **이번 cycle은 ECS 측 데이터 컴포넌트 정의 + 직렬화까지만**이며, 실제 렌더링 통합 (`voltex_renderer_ecs`) 은 cycle 2 (C0b) 에서 처리한다.

## 2. Scope / Out-of-scope

### 2.1 In-scope

- `voltex_ecs` 에 컴포넌트 3종 신규: `MeshHandle`, `MaterialHandle`, `Bounds`
- 위 3종의 `Component` trait 구현 + 직렬화 라운드트립 테스트
- `ComponentRegistry::register_defaults()` 에 3종 추가 (built-in 으로)
- `voltex_asset::Handle<T>` 에 public 접근자 (`id()`, `generation()`, `from_raw_parts()`) 추가 — 외부 직렬화 가능하게
- `voltex_ecs` 의존성 그래프 갱신 — `voltex_asset` 신규 의존 (L1 → L0, OK)
- `crates/voltex_ecs/CODEMAP.md`, `crates/voltex_asset/CODEMAP.md`, `CLAUDE.md` 의존 그래프 갱신

### 2.2 Out-of-scope (cycle 2+ 에서 처리)

- 실제 렌더링 호출, `voltex_renderer` 측 변경
- `voltex_renderer_ecs` 신규 crate
- `editor_demo` / `survivor_game` 의 평행 `Vec<Mesh>` 구조 마이그레이션
- `MeshHandle` 변경 시 `Bounds` 자동 재계산 시스템
- `voltex_renderer::Mesh` 와 marker 사이 ID 공간 매핑 (cycle 2 의 voltex_renderer_ecs brainstorm 에서 결정)

## 3. 아키텍처

### 3.1 의존성 변경

```
이전:  voltex_ecs → voltex_math
이후:  voltex_ecs → voltex_math
                  → voltex_asset  (신규)
```

`voltex_asset` 은 L0 (외부 의존 없음, std만). 따라서 cycle 형성되지 않고 L1 → L0 화살표만 추가. `CLAUDE.md` 의 의존성 그래프 텍스트도 갱신.

### 3.2 신규 marker 타입 (voltex_ecs)

`Handle<T>` 의 T 자리에 들어갈 phantom marker 를 voltex_ecs 측에 둔다.

```rust
// crates/voltex_ecs/src/render_components.rs (신규 모듈)

/// Mesh asset 마커. ID 공간을 분리하기 위한 phantom 타입.
/// 실제 GPU mesh 자원은 voltex_renderer::Mesh 이며,
/// cycle 2 의 voltex_renderer_ecs 가 이 marker 와 실제 자원을 매핑한다.
pub struct MeshAsset;

pub struct MaterialAsset;
```

**이름 선택 근거**: `Mesh` / `Material` 은 voltex_renderer 의 실제 GPU 타입과 충돌. `*Asset` 접미사로 "phantom marker (asset ID 공간)" 의도를 명확히 함.

### 3.3 신규 컴포넌트

```rust
// crates/voltex_ecs/src/render_components.rs

use voltex_asset::Handle;
use voltex_math::AABB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub Handle<MeshAsset>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialHandle(pub Handle<MaterialAsset>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds(pub AABB);
```

`Bounds` 는 `AABB` 가 `PartialEq` 만 지원하므로 `Eq` / `Hash` 미구현 (f32 비교).

### 3.4 Handle public 접근자 (voltex_asset)

현재 `Handle<T>::{id, generation}` 는 `pub(crate)` 이라 voltex_asset 외부에서 접근 불가. 직렬화를 위해 다음 추가:

```rust
// crates/voltex_asset/src/handle.rs

impl<T> Handle<T> {
    pub fn id(&self) -> u32 { self.id }
    pub fn generation(&self) -> u32 { self.generation }

    /// 직렬화에서 복원할 때 사용. **유효성 보장은 호출자 책임** —
    /// 임의의 (id, gen) 을 만들 수 있으므로 잘못된 핸들이 만들어질 수 있다.
    /// 정상 흐름은 AssetStorage::insert 의 반환값을 사용.
    pub fn from_raw_parts(id: u32, generation: u32) -> Self {
        Self {
            id,
            generation,
            _marker: std::marker::PhantomData,
        }
    }
}
```

## 4. 직렬화 포맷

`Component::NAME` 과 little-endian 바이트 레이아웃을 못 박는다 (`.vscn` 호환 잠금).

| 컴포넌트 | `NAME` | 크기 | 레이아웃 |
|---|---|---|---|
| `MeshHandle` | `"mesh_handle"` | 8 bytes | `id: u32 LE` + `generation: u32 LE` |
| `MaterialHandle` | `"material_handle"` | 8 bytes | `id: u32 LE` + `generation: u32 LE` |
| `Bounds` | `"bounds"` | 24 bytes | `min: 3 × f32 LE` + `max: 3 × f32 LE` |

deserialize 시 길이 부족이면 `EcsError::Generic("<name> data too short")` 반환 — Transform 패턴 그대로.

## 5. register_defaults() 변경

```rust
pub fn register_defaults(&mut self) {
    self.register::<crate::Transform>();
    self.register::<crate::scene::Tag>();
    self.register::<crate::MeshHandle>();      // 신규
    self.register::<crate::MaterialHandle>();  // 신규
    self.register::<crate::Bounds>();          // 신규
}
```

**근거**: editor / runtime 어느 워크플로로 들어와도 메시 컴포넌트는 항상 살아 있어야 한다 (특히 Play/Stop snapshot/restore — cycle 4 의 안전 조건). 컴포넌트 자체는 가벼운 핸들 (8 bytes) + AABB (24 bytes) 라 헤드리스/서버에서 등록 부담 없음.

## 6. 영향 범위

| 파일 | 변경 |
|---|---|
| `crates/voltex_ecs/Cargo.toml` | `voltex_asset.workspace = true` 의존 추가 |
| `crates/voltex_ecs/src/lib.rs` | `render_components` 모듈 + re-export (`MeshHandle`, `MaterialHandle`, `Bounds`, `MeshAsset`, `MaterialAsset`) |
| `crates/voltex_ecs/src/render_components.rs` (신규) | 마커 + 컴포넌트 + `Component` impl 3종 |
| `crates/voltex_ecs/src/component_registry.rs` | `register_defaults()` 에 3종 추가 |
| `crates/voltex_ecs/CODEMAP.md` | 신규 컴포넌트 + 의존성 갱신 + 테스트 개수 갱신 |
| `crates/voltex_asset/src/handle.rs` | `id()`, `generation()`, `from_raw_parts()` public 추가 |
| `crates/voltex_asset/CODEMAP.md` | 공개 API 섹션에 신규 메서드 |
| `CLAUDE.md` | 의존성 그래프 텍스트 `voltex_ecs → math` → `voltex_ecs → math, asset` 갱신 |

## 7. 테스트 계획 (≥ 6 신규)

`crates/voltex_ecs/src/render_components.rs` 내 `#[cfg(test)] mod tests`:

1. `test_mesh_handle_roundtrip` — `world.add(e, MeshHandle(Handle::from_raw_parts(42, 7)))` → serialize → deserialize → 핸들 id/gen 보존
2. `test_material_handle_roundtrip` — 동일 패턴
3. `test_bounds_roundtrip` — `Bounds(AABB::new(min, max))` → serialize → deserialize → min/max 정확히 보존 (각 좌표 epsilon 비교)
4. `test_bounds_deserialize_too_short` — 23 byte 데이터 → `Err`
5. `test_mesh_handle_deserialize_too_short` — 7 byte 데이터 → `Err`
6. `test_register_defaults_includes_render_components` — `registry.find("mesh_handle")`, `find("material_handle")`, `find("bounds")` 모두 `Some`
7. `test_register_defaults_total_count` — `entries().len() == 5` (Transform + Tag + 신규 3) — 기존 `test_entries_count` 갱신

`crates/voltex_asset/src/handle.rs`:

8. `test_handle_id_generation_accessors` — `Handle::from_raw_parts(3, 9).id() == 3 && .generation() == 9`

총 신규/갱신 ≥ 8 (로드맵 요구치 +6 충족).

## 8. 미해결 / cycle 2 로 이월

- **voltex_renderer 의 실제 `Mesh` / `Material` 과 marker 의 ID 공간 매핑**: cycle 2 의 voltex_renderer_ecs brainstorm 에서 결정. 후보:
  - (i) voltex_renderer 가 voltex_ecs 의 marker 를 import 해 `Assets<voltex_ecs::MeshAsset>` 형태 storage 보유 (id 공간 직접 공유)
  - (ii) voltex_renderer_ecs 가 `HashMap<Handle<MeshAsset>, &voltex_renderer::Mesh>` 매핑 테이블 유지
  - (iii) voltex_renderer 가 자체 GPU `Mesh` 를 `voltex_renderer::Mesh` 그대로 두되, voltex_renderer_ecs 가 marker → real 매핑을 단일 `Assets` 인스턴스로 통합
- **Bounds 자동 재계산**: 메시 핸들 변경 시 AABB 다시 계산 → cycle 2 의 voltex_renderer_ecs 가 시스템으로 제공
- **Bounds 가 로컬/월드 어느 공간인지**: 이번 cycle 은 "메시의 로컬 AABB 캐시" 의도로 둠. 월드 공간 변환은 cycle 2 의 시스템 책임. 컴포넌트 자체는 좌표계 불가지.

## 9. Done 기준 (로드맵 § Cycle 1 와 동기)

- 컴포넌트 3종 신규 + `Component` impl + 직렬화 라운드트립 테스트 통과
- voltex_ecs lib 테스트 +6 이상 (실제는 +7~8)
- `cargo build --workspace` 깨끗 (warning 허용 — strict 모드는 cycle 13 에서)
- `cargo test --workspace --lib` 깨끗
- `CODEMAP.md` (voltex_ecs, voltex_asset), `CLAUDE.md` 의존 그래프 갱신
- 로드맵 표의 Cycle 1 마커 `[ ]` → `[✅ 1/1]` + commit hash 기록

## 10. 변경 이력

- 2026-05-20: 초안 작성. brainstorming 과정에서 결정된 marker 위치/이름 (voltex_ecs::MeshAsset/MaterialAsset) 반영. 의존 그래프 변경 명시.
