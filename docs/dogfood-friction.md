# 도그푸딩 마찰 로그 — voltex_depths

게임 제작 중 발견한 엔진 워크플로 마찰. 항목: 하려던 것 / 없거나 어색했던 것 / 우회 / 제안(P0~P2).
엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 v1.1 로드맵 입력이 된다.

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
| 1 | 레벨 바닥 지오메트리로 NavMesh 자동 생성 (`NavMeshBuilder`) | `navmesh_builder/`가 M3 분리 이후 lib.rs에 미선언 — 모듈·테스트 7개가 한 번도 컴파일된 적 없는 죽은 코드였고 CODEMAP은 공개 API로 등재 (obstacle.rs도 동일 상태로 남아 있음) | lib.rs 배선 + 첫 컴파일에서 드러난 bitrot(visibility/dead_code/clippy) 수정 후 사용 — 배선 후 7개 테스트·게임 detour 테스트 전부 그린, 파라미터(cell_size 0.5, agent_radius 0.4)는 기본 플랜 그대로 동작 | 모듈 분리 리팩토링 시 "선언 누락 = 침묵하는 죽은 코드" 감지 수단 필요 (CI에서 src/ 하위 미배선 .rs 검출 스크립트 등). obstacle.rs 배선 여부 결정 필요 | P1 |
| 2 | 추적자가 움직이는 플레이어를 따라가도록 매 repath 틱마다 `NavAgent` 목표 갱신 | `agent.target = Some(p)` 직접 대입은 캐시된 `path`/`reached`를 무효화하지 않음 — `nav_agent_system`은 `path.is_empty() && !reached`일 때만 재탐색하므로 직접 대입은 조용히 무시되고, 도달 후엔 영구 정지하는 footgun. 필드가 `pub`이라 컴파일러가 못 막음 | `set_target()` 사용 (path/reached 리셋 → 재탐색). 단 set_target은 목표가 거의 안 움직였어도 전체 A* 재계산 | NavAgent 필드 캡슐화(target을 직접 대입 불가로) 또는 nav_agent_system이 target 변경을 감지해 스스로 repath. 보너스: 목표 이동량이 작으면 기존 path 재사용하는 retarget API | P1 |
| 3 | 적이 NavMesh 경로(waypoint y=0)를 따라가되 게임플레이 y(=0.5) 유지 | `follow_path`의 waypoint 도달 판정이 3D 거리 — 호출자가 y를 고정하면 dist≥0.5 > 도달 반경이라 중간 waypoint에 영원히 못 도달. 보정 안 하면 에이전트가 navmesh 평면(y=0)으로 가라앉음 | y 보정 없이 가라앉게 둠 (최종 goal이 y=0.5라 도달 시 복귀; 렌더 높이는 후속 태스크에서 처리 예정) | 경로 추종/도달 판정을 XZ 거리로 (A* 휴리스틱은 이미 `distance_xz` 사용 — follow_path만 3D) | P1 |
