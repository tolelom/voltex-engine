# voltex_ai

## 책임 (한 줄)
AI 기초 — NavMesh (수동 + 자동 voxelization), A* + Funnel, steering, 동적 장애물 회피.

## 공개 API 진입점
- `NavMesh` — 폴리곤 기반 내비메시 (src/navmesh/mod.rs)
- 내비메시 바이너리 (src/navmesh/format.rs) — 두 포맷이 같은 본문을 쓴다. VNAV 파일: `NavMesh::to_vnav_bytes`/`from_vnav_bytes`(옛 이름 `serialize`/`deserialize`, deprecated). 헤더 없는 포맷: `serialize_navmesh_headerless`/`deserialize_navmesh_headerless`(옛 이름 `serialize_navmesh`/`deserialize_navmesh`, deprecated)
- `NavMeshBuilder` — voxelization 기반 자동 생성 (src/navmesh_builder/). `max_slope`는 도(degree) 단위. `agent_height`/`cell_height`는 아직 쓰이지 않는 설정
- `find_path` — A* + String Pulling (Funnel) (src/pathfinding/mod.rs)
- Steering — `seek`, `flee`, `arrive`, `wander`, `follow_path` (src/steering.rs). `follow_path` 는 평면(XZ) — waypoint 를 에이전트 y 평면에 투영해 도달 판정/조향 (게임플레이 y 유지, navmesh 평면으로 침하 없음)
- Flocking (boid) — `separation`, `cohesion`, `alignment` 순수 함수 + `Boid` 컴포넌트 + `BoidParams` + `flocking_system(world, &params, dt)` ECS 시스템 (src/flocking.rs). 시스템이 `SpatialGrid` 로 이웃 수집(~O(n)) 후 3법칙 가중합 → 속도/위치 적분(max_speed clamp). 스냅샷 후 일괄 기록(순서 무관)
- `NavAgent` — ECS 컴포넌트 (src/nav_agent.rs). `target` 직접 대입도 동작 — 시스템이 `planned_target`(crate 내부)과 비교해 변경 감지 후 스스로 repath
- `nav_agent_system(world, navmesh, dt)` — NavAgent 엔티티 경로 추적 ECS 시스템 (src/nav_agent_system.rs). target 변경 시 자동 repath; 새 target 이 기존 path 끝점과 같은 삼각형이면 A* 없이 끝점만 교체(retarget). 도달 판정 XZ, goal snap 은 agent y 보존
- `DynamicObstacle` + `avoid_obstacles(pos, vel, radius, &[obstacle], look_ahead)` — velocity obstacle 회피 조향 (src/obstacle.rs). M3 분리 후 미배선이었다가 module_wiring 가드 도입 시 배선 (+6 테스트 부활)
- `AiError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `navmesh` — NavMesh 데이터 구조 + XZ 무게중심 좌표 헬퍼 `barycentric_xz`(crate 내부) (src/navmesh/mod.rs) + 직렬화 (src/navmesh/format.rs)
- `navmesh_builder` — voxelization (src/navmesh_builder/: mod, voxelize, walkable, regions, triangulate). 세 단계 결과(Heightfield/WalkableMap/RegionMap)가 같은 `GridLayout`(셀 격자 + 4-이웃 범위 검사)을 공유
- `pathfinding` — A* + Funnel (src/pathfinding/mod.rs)
- `steering` — steering behaviors (src/steering.rs)
- `flocking` — boid separation/cohesion/alignment (SpatialGrid 로 이웃 수집) (src/flocking.rs)
- `nav_agent` — ECS 컴포넌트 (src/nav_agent.rs)
- `nav_agent_system` — NavAgent 경로 추적 시스템 (src/nav_agent_system.rs)
- `obstacle` — 동적 장애물 회피 (velocity obstacle) (src/obstacle.rs)

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- navmesh_builder는 src/navmesh_builder/ 디렉토리로 분리됨 (M3 완료). 분리 후 lib.rs 미배선으로 죽어 있다가 depths 투입 시점에 배선 (`pub use NavMeshBuilder`) — 파이프라인 단계 fn(voxelize 등)은 pub(crate)
- voxelize는 삼각형 winding 무관 (normal 미사용, 높이차로 slope 판정). agent_radius erosion은 `ceil(radius/cell_size)` 보크셀 단위
- VNAV 바이너리 포맷 변경 시 저장된 내비메시 호환 깨짐
- nav_agent_system은 kinematic(위치 적분) — RigidBody velocity 커플링은 호출자/후속 사이클. NavMesh는 호출자 소유로 매 프레임 주입

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ai --lib`
- 현재 테스트 개수: 75개 (planar follow_path +3, target 변경 감지 repath/retarget +3, obstacle 배선 +6)
