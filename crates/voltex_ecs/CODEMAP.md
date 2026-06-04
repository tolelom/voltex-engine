# voltex_ecs

## 책임 (한 줄)
ECS 코어 — Entity/Component/World/Query + Hierarchy + Scene 직렬화 + System Scheduler.

## 공개 API 진입점
- `World` — type-erased 저장소 + query, `clear()` 로 전체 리셋 (src/world/mod.rs)
- `World::insert_resource::<T>` / `resource::<T>()` / `resource_mut` / `remove_resource` / `resource_or_insert_with` / `has_resource` — 엔티티 무관 싱글턴 공유 상태(게임 점수/설정/이벤트 큐/RNG). 시스템이 이미 `&mut World` 를 받아 추가 배선 없이 접근. `clear()` 시 함께 제거
- `Events<T>` + `World::send_event::<T>` / `drain_events::<T>() -> Vec<T>` — 시스템 간 디커플 메시징 (src/events.rs). 리소스 기반(`Events<T>` 가 리소스). single-buffer drain — producer 를 consumer 보다 먼저 등록
- `SpatialGrid` + `from_transforms(&World, cell_size)` / `insert` / `query_radius(center, r)` / `query_radius_into` / `nearest(point, exclude)` — 게임플레이 근접 쿼리용 균일 그리드 공간 해시 (src/spatial.rs). AoE/어그로/이웃 탐색. physics BVH(콜라이더)와 별개로 Transform 위치 기반. 프레임당 재구축
- `Bundle` + `World::spawn_bundle((A,B,..))` / `insert_bundle` / `spawn_child_bundle(parent, ..)` — 컴포넌트 튜플(1~12)을 한 번에 첨부 (src/bundle.rs). spawn() + N add() 대체. `spawn_child_bundle` 은 add_child 로 부모 하위 연결
- `World::find_by_tag(&str) -> Option<Entity>` / `iter_by_tag(&str)` — Tag 로 엔티티 조회 (src/scene.rs). `query::<Tag>().find(..)` 보일러플레이트 대체
- `Prefabs` + `register(name, \|&mut World\|->Entity)` / `spawn(&mut World, name)` / `spawn_child(&mut World, name, parent)` / `contains` — 명명된 엔티티 템플릿 (src/prefab.rs). spawn_bundle 위에 "한 번 정의, 이름으로 N개 인스턴스화"(Unity Prefab). 팩토리 컴포넌트-무관. `spawn_child` 는 부모 하위로 인스턴스화
- `Entity` — id + generation (src/entity.rs)
- `SparseSet<T>` — 컴포넌트 저장소 (src/sparse_set.rs)
- `Transform` — 위치/회전/스케일 컴포넌트 (src/transform.rs)
- `Parent`, `Children`, `add_child`, `despawn_recursive` — 계층 (src/hierarchy.rs)
- `WorldTransform` — 부모 변환 propagation (src/world_transform.rs)
- `serialize_scene_json`/`deserialize_scene_json` — JSON 씬 (src/json.rs, src/scene.rs)
- VSCN binary format — `serialize_scene_binary` (src/binary_scene.rs)
- `Component` — 직렬화 가능 컴포넌트 trait (`NAME` + `serialize`/`deserialize`) (src/component_registry.rs)
- `MeshHandle`, `MaterialHandle`, `Bounds` — 렌더링 데이터 컴포넌트 (src/render_components.rs)
- `MeshAsset`, `MaterialAsset` — Handle<T> 의 phantom marker (src/render_components.rs)
- `ScriptComponent` — per-entity Lua 스크립트 첨부. `script_path` + `instance_id` (UNBOUND sentinel) (src/script_component.rs)
- `World::live_entities()` — 살아있는 모든 entity iterator (src/world.rs, voltex_script::iter_entities 가 사용)
- `ComponentRegistry::register::<T>()` — Component trait 기반 표준 등록 (src/component_registry.rs)
- `ComponentRegistry::register_raw` — trait 구현 불가 타입용 escape hatch (src/component_registry.rs)
- `path_component!(Ty, "name")` — `struct Ty(pub String)` 경로 컴포넌트의 Component impl 생성 매크로 (UTF-8 round-trip). TexturePath/MeshPath(renderer_ecs)/AudioPath(runtime) 가 사용 (src/component_registry.rs)
- `Scheduler`, `DependencyScheduler` — 시스템 실행 (src/scheduler.rs, src/dependency_scheduler.rs)
- `Archetype` — signature 기반 그룹 저장소 (src/archetype.rs)
- `EcsError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `entity` — Entity (src/entity.rs)
- `sparse_set` — SparseSet<T> (src/sparse_set.rs)
- `world` — World + query<T>/query2/3/4, With/Without 필터 (src/world.rs)
- `bundle` — Bundle trait (튜플 1~12) + World::spawn_bundle/insert_bundle/spawn_child_bundle (src/bundle.rs)
- `prefab` — Prefabs 레지스트리 (명명된 엔티티 템플릿) (src/prefab.rs)
- `events` — Events<T> + World::send_event/drain_events (시스템 간 메시징) (src/events.rs)
- `spatial` — SpatialGrid 근접 쿼리 (균일 그리드 공간 해시) (src/spatial.rs)
- `transform` — Transform component (src/transform.rs)
- `hierarchy` — Parent/Children + 트리 조작 (src/hierarchy.rs)
- `world_transform` — 변환 propagation (src/world_transform.rs)
- `scene`, `json`, `binary_scene` — 씬 직렬화 (src/scene.rs, src/json.rs, src/binary_scene.rs)
- `component_registry` — Component trait + 제네릭 register::<T>() + raw escape hatch (src/component_registry.rs)
- `scheduler`, `dependency_scheduler` — 시스템 스케줄링 (src/scheduler.rs, src/dependency_scheduler.rs)
- `archetype` — Archetype 저장 (src/archetype.rs)
- `render_components` — Mesh/Material handle + Bounds + phantom marker (src/render_components.rs)
- `script_component` — ScriptComponent (script_path + instance_id) + Component impl (src/script_component.rs)

## 의존성
- voltex crate: voltex_math (Vec3, Mat4, AABB), voltex_asset (Handle<T>)
- 외부 crate: 없음

## 변경 시 주의점
- ECS 인터페이스 변경은 모든 example, voltex_physics, voltex_ai, voltex_script, voltex_editor에 영향
- ComponentRegistry에 등록된 타입의 직렬화 포맷 변경 시 기존 .vscn 파일 호환 깨짐
- World의 query 시그니처 변경은 game 코드에 광범위 영향
- world.rs는 589줄로 큰 편이나 spec 만족 (600줄 이하), 분리 대상 아님

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ecs --lib`
- 현재 테스트 개수: 121개 (S1 +5: ScriptComponent ×4 + register_defaults_includes_script ×1)
