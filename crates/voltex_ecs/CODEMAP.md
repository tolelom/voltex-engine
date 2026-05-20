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
- world.rs는 589줄로 큰 편이나 spec 만족 (600줄 이하), 분리 대상 아님

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ecs --lib`
- 현재 테스트 개수: 104개
