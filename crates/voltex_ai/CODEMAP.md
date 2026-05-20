# voltex_ai

## 책임 (한 줄)
AI 기초 — NavMesh (수동 + 자동 voxelization), A* + Funnel, steering, 동적 장애물 회피.

## 공개 API 진입점
- `NavMesh` — 폴리곤 기반 내비메시 (src/navmesh.rs)
- `NavMeshBuilder` — voxelization 기반 자동 생성 (src/navmesh_builder/)
- `find_path` — A* + String Pulling (Funnel) (src/pathfinding.rs)
- Steering — `seek`, `flee`, `arrive`, `wander` (src/steering.rs)
- `NavAgent` — ECS 컴포넌트 (src/nav_agent.rs)
- `Obstacle`, velocity obstacle 회피 (src/obstacle.rs)

## 모듈 구조
- `navmesh` — NavMesh 데이터 구조 + 직렬화 (src/navmesh.rs)
- `navmesh_builder` — voxelization (src/navmesh_builder/: mod, voxelize, walkable, regions, triangulate)
- `pathfinding` — A* + Funnel (src/pathfinding.rs)
- `steering` — steering behaviors (src/steering.rs)
- `nav_agent` — ECS 컴포넌트 (src/nav_agent.rs)
- `obstacle` — 동적 장애물 회피 (src/obstacle.rs)

## 의존성
- voltex crate: voltex_math, voltex_ecs
- 외부 crate: 없음

## 변경 시 주의점
- navmesh_builder는 src/navmesh_builder/ 디렉토리로 분리됨 (M3 완료)
- navmesh.rs는 497줄
- VNAV 바이너리 포맷 변경 시 저장된 내비메시 호환 깨짐
- nav_agent_system은 ECS 미구현 (DEFERRED)

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_ai --lib`
- 현재 테스트 개수: 34개
