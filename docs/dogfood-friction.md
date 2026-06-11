# 도그푸딩 마찰 로그 — voltex_depths

게임 제작 중 발견한 엔진 워크플로 마찰. 항목: 하려던 것 / 없거나 어색했던 것 / 우회 / 제안(P0~P2).
엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 v1.1 로드맵 입력이 된다.

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
| 1 | 레벨 바닥 지오메트리로 NavMesh 자동 생성 (`NavMeshBuilder`) | `navmesh_builder/`가 M3 분리 이후 lib.rs에 미선언 — 모듈·테스트 7개가 한 번도 컴파일된 적 없는 죽은 코드였고 CODEMAP은 공개 API로 등재 (obstacle.rs도 동일 상태로 남아 있음) | lib.rs 배선 + 첫 컴파일에서 드러난 bitrot(visibility/dead_code/clippy) 수정 후 사용 — 배선 후 7개 테스트·게임 detour 테스트 전부 그린, 파라미터(cell_size 0.5, agent_radius 0.4)는 기본 플랜 그대로 동작 | 모듈 분리 리팩토링 시 "선언 누락 = 침묵하는 죽은 코드" 감지 수단 필요 (CI에서 src/ 하위 미배선 .rs 검출 스크립트 등). obstacle.rs 배선 여부 결정 필요 | P1 |
