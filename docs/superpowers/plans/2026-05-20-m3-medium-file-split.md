# M3: 600줄 초과 파일 분리 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** spec의 성공 기준 "모든 .rs 파일 600줄 이하"를 충족한다. 600줄 초과 파일을 모듈 디렉토리로 분리.

**Architecture:** M2와 동일한 분리 패턴. `foo.rs` → `foo/{mod.rs, sub_a.rs, sub_b.rs, ...}`. 공개 API 시그니처 무변경.

**Tech Stack:** Rust (cargo, rustfmt). 검증은 `cargo build/test` + 파일 줄 수 측정.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 2, 성공기준 #2 "모든 .rs 파일 600줄 이하".

---

## M3 대상 결정

원래 plan은 "500-800줄 파일 11개 분리"였으나, **spec의 성공 기준은 600줄 이하**임. 현재 상태에서 600줄을 초과하는 파일은 다음 2개만 해당:

| 파일 | 줄 수 |
|---|---|
| `crates/voltex_editor/src/widgets.rs` | 745 |
| `crates/voltex_ai/src/navmesh_builder.rs` | 651 |

500-600줄 사이 파일 9개(world 589, solver 574, dock 558, gjk 524, wasapi 521, ttf_parser 518, texture 514, state 505, navmesh 497)는 **이미 spec 만족**이므로 분리 대상 아님. M3에서는 600 초과 2개만 분리하고, 500-600 파일에 대한 "M3 분리 예정" 문구는 CODEMAP에서 정정.

---

## 분리 후 목표 줄 수

- 모든 `.rs` 파일 ≤ 600줄
- `crates/voltex_editor/src/widgets/` 내 모든 파일 ≤ 600줄 (목표 ≤ 300줄)
- `crates/voltex_ai/src/navmesh_builder/` 내 모든 파일 ≤ 600줄 (목표 ≤ 300줄)

---

## Task 1: widgets.rs 분리

**현재 위치**: `crates/voltex_editor/src/widgets.rs` (745줄)
**목표 구조**: `crates/voltex_editor/src/widgets/` 디렉토리

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod widgets;
```

widgets.rs 전체가 `impl UiContext { ... }` 단일 블록 + tests이다. Rust에서는 같은 type에 대한 impl을 같은 crate 내 여러 파일에 분산할 수 있으므로, 각 sub-module 파일에서 `use super::super::ui_context::UiContext;` 후 `impl UiContext { ... }`로 메서드 정의.

### 모듈 매핑

각 sub-module은 `impl UiContext { ... }` 블록 안에 해당 메서드들을 둔다.

| 새 파일 | 원본 line 범위 | 포함 메서드 |
|---|---|---|
| `widgets/mod.rs` | 1-22 (imports + impl 헤더) + tests (line ~466-end) | sub-module 선언, `use` 재정렬, tests |
| `widgets/controls.rs` | 24-213 (text, button, slider, checkbox) | `text`, `button`, `slider`, `checkbox` |
| `widgets/containers.rs` | 214-249, 359-409 | `begin_panel`, `end_panel`, `begin_scroll_panel`, `end_scroll_panel` |
| `widgets/text_input.rs` | 250-358 (text_input) | `text_input` |
| `widgets/drag.rs` | 410-465 (drag/drop) | `begin_drag`, `is_dragging`, `end_drag`, `drop_target` |

예상 줄 수: 각 파일 50~250줄.

각 sub-module 파일 상단:
```rust
use super::super::ui_context::UiContext;
// 필요시 다른 imports (UiId, DrawList, layout, font 등)

impl UiContext {
    pub fn <method_name>(...) -> ... {
        // 원본 코드 그대로
    }
    // 같은 카테고리의 다른 메서드들...
}
```

`mod.rs`:
```rust
mod controls;
mod containers;
mod text_input;
mod drag;
// 메서드들은 impl UiContext { ... } 형태로 sub-module에 있으므로,
// 별도의 pub use는 필요 없음 (impl 메서드는 자동 노출).
// 단, 만약 widgets.rs에 자유 함수나 type이 있으면 pub use로 재노출.

#[cfg(test)]
mod tests {
    // 원본의 #[cfg(test)] 블록을 그대로 옮긴다.
    use super::super::ui_context::UiContext;
    // ... 기존 use 그대로
    // 각 테스트의 method 호출은 UiContext의 impl로 자동 해결됨.
}
```

- [ ] **Step 1: 디렉토리 생성**

```bash
mkdir -p crates/voltex_editor/src/widgets
```

- [ ] **Step 2: 원본 widgets.rs의 imports + tests를 정확히 파악**

```bash
head -25 crates/voltex_editor/src/widgets.rs
sed -n '465,750p' crates/voltex_editor/src/widgets.rs
```

원본 widgets.rs line 1-23의 `use` 문 목록을 그대로 보존하여 각 sub-module의 필요에 맞게 분배.

- [ ] **Step 3: widgets/controls.rs 작성**

원본 line 22(`impl UiContext {`)부터 시작해 24-213(text/button/slider/checkbox)을 옮긴다.

```rust
use super::super::ui_context::UiContext;
// 원본 widgets.rs 상단의 use 중 controls(text/button/slider/checkbox)에 필요한 것:
// - DrawList, Color, font 관련, layout 관련 등

impl UiContext {
    pub fn text(&mut self, text: &str) {
        // line 24-42 그대로
    }

    pub fn button(&mut self, label: &str) -> bool {
        // line 43-99 그대로
    }

    pub fn slider(&mut self, label: &str, value: f32, min: f32, max: f32) -> f32 {
        // line 100-161 그대로
    }

    pub fn checkbox(&mut self, label: &str, checked: bool) -> bool {
        // line 162-213 그대로
    }
}
```

- [ ] **Step 4: widgets/containers.rs 작성**

원본 line 214-249 (begin_panel, end_panel) + 359-409 (begin/end_scroll_panel) 을 옮긴다.

```rust
use super::super::ui_context::UiContext;
// 필요한 use 추가

impl UiContext {
    pub fn begin_panel(&mut self, ...) { /* 214-239 */ }
    pub fn end_panel(&mut self) { /* 240-249 */ }
    pub fn begin_scroll_panel(&mut self, ...) { /* 359-401 */ }
    pub fn end_scroll_panel(&mut self) { /* 402-409 */ }
}
```

- [ ] **Step 5: widgets/text_input.rs 작성**

원본 line 250-358 (text_input) 을 옮긴다.

```rust
use super::super::ui_context::UiContext;
// 필요한 use 추가

impl UiContext {
    pub fn text_input(&mut self, id: u32, buffer: &mut String, x: f32, y: f32, width: f32) -> bool {
        // 250-358 그대로
    }
}
```

- [ ] **Step 6: widgets/drag.rs 작성**

원본 line 410-465 (drag/drop) 을 옮긴다.

```rust
use super::super::ui_context::UiContext;
// 필요한 use 추가

impl UiContext {
    pub fn begin_drag(&mut self, id: u32, payload: u64) { /* 410-418 */ }
    pub fn is_dragging(&self) -> bool { /* 419-428 */ }
    pub fn end_drag(&mut self) -> Option<(u32, u64)> { /* 429-452 */ }
    pub fn drop_target(&mut self, _id: u32, x: f32, y: f32, w: f32, h: f32) -> Option<u64> { /* 453-465 */ }
}
```

- [ ] **Step 7: widgets/mod.rs 작성**

```rust
mod controls;
mod containers;
mod text_input;
mod drag;

#[cfg(test)]
mod tests {
    // 원본 widgets.rs의 #[cfg(test)] mod tests { ... } 블록 (line ~466부터 끝까지)을 옮긴다.
    // use 문은 그대로 (super::* 또는 명시적).
}
```

- [ ] **Step 8: 원본 widgets.rs 삭제**

```bash
rm crates/voltex_editor/src/widgets.rs
```

- [ ] **Step 9: 빌드 + 테스트 + 줄 수 검증**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p voltex_editor 2>&1 | tail -5
cargo test -p voltex_editor --lib 2>&1 | grep "test result"
find crates/voltex_editor/src/widgets -name "*.rs" -exec wc -l {} \;
```

Expected:
- 빌드 성공
- 테스트 105개 통과
- 모든 sub-module 파일 ≤ 600줄

- [ ] **Step 10: Commit**

```bash
git add crates/voltex_editor/src/widgets/ crates/voltex_editor/src/widgets.rs
git commit -m "refactor(editor): split widgets.rs (745 lines) into module directory"
```

---

## Task 2: navmesh_builder.rs 분리

**현재 위치**: `crates/voltex_ai/src/navmesh_builder.rs` (651줄)
**목표 구조**: `crates/voltex_ai/src/navmesh_builder/`

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod navmesh_builder;
// 외부 사용은 voltex_ai::navmesh_builder::NavMeshBuilder 등
```

### 모듈 매핑

| 새 파일 | 원본 line 범위 | 포함 항목 |
|---|---|---|
| `navmesh_builder/mod.rs` | 1-78 (structs + Default + small impls) + 79-84, 436-444 (NavMeshBuilder::new, build) + 445-478 (free helpers) + tests (~479-end) | structs, simple methods, helper fn, tests |
| `navmesh_builder/voxelize.rs` | 85-168 (voxelize) | `impl NavMeshBuilder { fn voxelize ... }` |
| `navmesh_builder/walkable.rs` | 169-295 (mark_walkable + check_slope) | `impl NavMeshBuilder { fn mark_walkable, fn check_slope ... }` |
| `navmesh_builder/regions.rs` | 296-341 (build_regions) | `impl NavMeshBuilder { fn build_regions ... }` |
| `navmesh_builder/triangulate.rs` | 342-435 (triangulate) | `impl NavMeshBuilder { fn triangulate ... }` |

예상 줄 수: 각 파일 50~130줄.

각 sub-module 파일 상단:
```rust
use super::{NavMeshBuilder, Heightfield, WalkableMap, RegionMap};
// 필요한 추가 use (Vec3, NavMesh 등)
use voltex_math::Vec3;
use crate::navmesh::NavMesh;

impl NavMeshBuilder {
    // 메서드들 옮김
}
```

`mod.rs`:
```rust
mod voxelize;
mod walkable;
mod regions;
mod triangulate;

use voltex_math::Vec3;
use crate::navmesh::NavMesh;

// 원본 widgets.rs line 1-78 (structs + Default + small impls 그대로)
pub struct NavMeshBuilder { ... }
impl Default for NavMeshBuilder { ... }
pub struct Heightfield { ... }
impl Heightfield { ... }
pub struct WalkableMap { ... }
impl WalkableMap { ... }
pub struct RegionMap { ... }

impl NavMeshBuilder {
    pub fn new() -> Self { ... }     // 80-84
    pub fn build(&self, vertices: &[Vec3], indices: &[u32]) -> NavMesh { ... }  // 436-444
}

// 자유 헬퍼 함수 (445-478)
fn point_in_triangle_xz_loose(...) -> bool { ... }
fn interpolate_y(...) -> f32 { ... }

#[cfg(test)]
mod tests {
    // 원본 #[cfg(test)] mod tests 그대로
}
```

**중요**: `point_in_triangle_xz_loose`와 `interpolate_y`는 triangulate.rs에서 호출. 두 가지 방법:
1. `mod.rs`에 두고 `pub(super)`로 변경 → triangulate.rs에서 `use super::point_in_triangle_xz_loose;`
2. triangulate.rs로 옮기기 (가장 간단)

**채택: 방법 2** — triangulate.rs로 옮기고 private 유지.

`navmesh_builder/triangulate.rs`:
```rust
use voltex_math::Vec3;
use crate::navmesh::NavMesh;
use super::{NavMeshBuilder, RegionMap};

impl NavMeshBuilder {
    pub fn triangulate(&self, rm: &RegionMap) -> NavMesh {
        // line 342-435 그대로
    }
}

fn point_in_triangle_xz_loose(point: Vec3, a: Vec3, b: Vec3, c: Vec3, margin: f32) -> bool {
    // line 445-463 그대로
}

fn interpolate_y(a: Vec3, b: Vec3, c: Vec3, px: f32, pz: f32) -> f32 {
    // line 464-478 그대로
}
```

이로써 `mod.rs`는 자유 헬퍼 없이 깔끔.

- [ ] **Step 1: 디렉토리 생성**

```bash
mkdir -p crates/voltex_ai/src/navmesh_builder
```

- [ ] **Step 2: navmesh_builder/voxelize.rs 작성**

원본 line 85-168을 옮긴다.

```rust
use super::{NavMeshBuilder, Heightfield};
use voltex_math::Vec3;

impl NavMeshBuilder {
    pub fn voxelize(&self, vertices: &[Vec3], indices: &[u32]) -> Heightfield {
        // line 85-168 그대로
    }
}
```

- [ ] **Step 3: navmesh_builder/walkable.rs 작성**

원본 line 169-295를 옮긴다 (mark_walkable + check_slope).

```rust
use super::{NavMeshBuilder, Heightfield, WalkableMap};

impl NavMeshBuilder {
    pub fn mark_walkable(&self, hf: &Heightfield) -> WalkableMap {
        // line 169-264 그대로
    }

    fn check_slope(&self, hf: &Heightfield, x: usize, z: usize, max_slope_cos: f32) -> bool {
        // line 265-295 그대로
    }
}
```

- [ ] **Step 4: navmesh_builder/regions.rs 작성**

원본 line 296-341을 옮긴다.

```rust
use super::{NavMeshBuilder, WalkableMap, RegionMap};

impl NavMeshBuilder {
    pub fn build_regions(&self, wm: &WalkableMap) -> RegionMap {
        // line 296-341 그대로
    }
}
```

- [ ] **Step 5: navmesh_builder/triangulate.rs 작성**

원본 line 342-435 + 445-478 (자유 헬퍼)를 옮긴다.

```rust
use super::{NavMeshBuilder, RegionMap};
use voltex_math::Vec3;
use crate::navmesh::NavMesh;

impl NavMeshBuilder {
    pub fn triangulate(&self, rm: &RegionMap) -> NavMesh {
        // line 342-435 그대로
    }
}

fn point_in_triangle_xz_loose(point: Vec3, a: Vec3, b: Vec3, c: Vec3, margin: f32) -> bool {
    // line 445-463 그대로
}

fn interpolate_y(a: Vec3, b: Vec3, c: Vec3, px: f32, pz: f32) -> f32 {
    // line 464-478 그대로
}
```

- [ ] **Step 6: navmesh_builder/mod.rs 작성**

원본 line 1-78 (structs + Default + small impls) + line 79-84 (new) + line 436-444 (build) + tests (~479-end) 을 합친다.

```rust
mod voxelize;
mod walkable;
mod regions;
mod triangulate;

use voltex_math::Vec3;
use crate::navmesh::NavMesh;

// 원본 line 1-78의 structs + Default impl + Heightfield/WalkableMap impl 그대로 옮긴다.
pub struct NavMeshBuilder { /* ... */ }
impl Default for NavMeshBuilder { /* ... */ }
pub struct Heightfield { /* ... */ }
impl Heightfield {
    pub fn cell_index(&self, x: usize, z: usize) -> usize { /* ... */ }
}
pub struct WalkableMap { /* ... */ }
impl WalkableMap {
    pub fn cell_index(&self, x: usize, z: usize) -> usize { /* ... */ }
}
pub struct RegionMap { /* ... */ }

// new/build는 mod.rs에 둔다 (간단한 wrapper).
impl NavMeshBuilder {
    pub fn new() -> Self {
        // line 80-84 그대로
    }

    pub fn build(&self, vertices: &[Vec3], indices: &[u32]) -> NavMesh {
        // line 436-444 그대로
    }
}

#[cfg(test)]
mod tests {
    // 원본 #[cfg(test)] mod tests { ... } 그대로 옮긴다.
    use super::*;
    // ...
}
```

- [ ] **Step 7: 원본 navmesh_builder.rs 삭제**

```bash
rm crates/voltex_ai/src/navmesh_builder.rs
```

- [ ] **Step 8: 빌드 + 테스트 + 줄 수 검증**

```bash
cargo build -p voltex_ai 2>&1 | tail -5
cargo test -p voltex_ai --lib 2>&1 | grep "test result"
find crates/voltex_ai/src/navmesh_builder -name "*.rs" -exec wc -l {} \;
```

Expected:
- 빌드 성공
- 테스트 34개 통과
- 모든 sub-module 파일 ≤ 600줄

- [ ] **Step 9: Commit**

```bash
git add crates/voltex_ai/src/navmesh_builder/ crates/voltex_ai/src/navmesh_builder.rs
git commit -m "refactor(ai): split navmesh_builder.rs (651 lines) into module directory"
```

---

## Task 3: 최종 검증 + CODEMAP 갱신 + push

- [ ] **Step 1: 600줄 초과 파일 0개 확인**

```bash
find crates -name "*.rs" -type f -exec wc -l {} \; | awk '$1 > 600 {print}'
```

Expected: 출력 없음 (0개).

- [ ] **Step 2: 전체 빌드/테스트**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --workspace 2>&1 | tail -3
cargo test --workspace --lib 2>&1 | grep -E "^test result" | awk '{sum += $4} END {print sum}'
```

Expected:
- 빌드 성공
- 760개 테스트 통과

- [ ] **Step 3: CODEMAP 갱신**

영향받는 CODEMAP:
- `crates/voltex_editor/CODEMAP.md`: widgets.rs (745줄)를 `widgets/` 디렉토리로 갱신, "M3 분리 예정" 문구 제거
- `crates/voltex_ai/CODEMAP.md`: navmesh_builder.rs (651줄)를 `navmesh_builder/` 디렉토리로 갱신, "M3 분리 예정" 문구 제거

500-600 파일에 대한 "M3 분리 예정" 문구도 제거 (이미 spec 만족):
- `crates/voltex_editor/CODEMAP.md`: dock.rs (558줄), ttf_parser.rs (518줄)
- `crates/voltex_ecs/CODEMAP.md`: world.rs (589줄)
- `crates/voltex_physics/CODEMAP.md`: solver.rs (574줄), gjk.rs (524줄)
- `crates/voltex_audio/CODEMAP.md`: wasapi.rs (521줄)
- `crates/voltex_renderer/CODEMAP.md`: texture.rs (514줄)
- `crates/voltex_script/CODEMAP.md`: state.rs (505줄)

각각 "M3 분리 예정" 표기를 제거하거나 "현재 N줄 (spec 만족, 분리 대상 아님)" 식으로 정정.

- [ ] **Step 4: CODEMAP commit**

```bash
git add crates/*/CODEMAP.md
git commit -m "docs(codemap): update CODEMAPs for M3 splits and clarify line targets"
```

- [ ] **Step 5: Push**

```bash
git log --oneline -10
git push origin master
```

---

## 완료 기준 (Definition of Done)

- [ ] `crates/voltex_editor/src/widgets/` 디렉토리 존재, 원본 widgets.rs 없음
- [ ] `crates/voltex_ai/src/navmesh_builder/` 디렉토리 존재, 원본 navmesh_builder.rs 없음
- [ ] **600줄 초과 `.rs` 파일 0개** (spec 성공기준 #2 충족)
- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace --lib` 760개 통과
- [ ] 영향받은 CODEMAP 갱신
- [ ] Push 완료

## 리스크 및 대응

- **impl UiContext 분산 시 동일 메서드 중복 정의**: 분리 후 같은 메서드가 두 sub-module에 있으면 컴파일 에러. 매핑 표를 정확히 따를 것.
- **테스트가 sub-module의 private 메서드 참조**: tests를 mod.rs에 둘 때 메서드는 모두 `impl UiContext { pub fn ... }` 형태로 sub-module에 있으므로 자동 노출됨. 단, free helper(`point_in_triangle_xz_loose` 등)는 private이므로 같은 module 내에 있어야 함.
- **navmesh_builder의 free helper 위치**: triangulate.rs로 옮겨 private 유지. mod.rs로 옮기지 않음.

## 다음 마일스톤

M4: 에러 타입 표준화 (thiserror 기반).
