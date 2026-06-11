# Voltex Depths — 도그푸딩 게임 1편 (설계)

- 날짜: 2026-06-11
- 상태: 승인됨 (자율 구현 진행)
- 목적: 엔진 기능 검증 + **저작 워크플로 마찰 발견**. 게임을 타이틀→클리어 루프→사운드→빌드 배포까지 "끝까지" 완성한다.

## 배경 / 왜 게임 먼저인가

엔진의 진짜 격차는 기능 폭이 아니라 워크플로 (메모리/회고 합의). v1.0.62~85의
"기초 워크플로 격차" 사이클이 전부 실사용에서 나왔듯, 다음 마찰 목록은 실제 게임을
만들어야 발견된다. 특히 **NavMesh/A*/steering/NavAgent 는 데모조차 게임에서 안 써본
미실전 영역** — 이번 게임의 핵심 도그푸딩 대상.

## 게임 컨셉

**탑다운 3D 트윈스틱 던전 클리어 액션** (가칭 Voltex Depths).

- WASD 이동 + 마우스 조준·좌클릭 발사 (`InputMap` 액션/축)
- 고정 레이아웃 던전: 일반 방 5 + 보스방 1, 복도 연결
- 방 입장 → 문 잠김 → 적 전멸 → 문 열림 (RoomCleared)
- 적 2종:
  - **추적자**(근접): NavMesh A* 로 벽·기둥을 우회해 플레이어 추격, 접촉 데미지
  - **사수**(원거리): 사거리 유지 + 투사체 발사, 시야 차단 시 NavMesh 우회
- 보스: 패턴 2개 (돌진 + 방사 탄막)
- 플레이어 HP 5. 사망 → 게임오버 → 타이틀. 보스 격파 → 클리어 화면
- 완성 기준 (DoD): 타이틀 → 플레이 → 클리어/게임오버 전체 루프 + 효과음/BGM +
  `voltex build` 단독 exe 구동

## 기술 구성 (전부 기존 API — 신규 엔진 기능 0 으로 시작)

| 영역 | 사용 API | 도그푸딩 노트 |
|---|---|---|
| 부트스트랩 | `RuntimeApp` + `RuntimeConfig` | flagship 패턴 계승 |
| 카메라 | `with_camera` + `set_camera_fn` (플레이어 추적 탑다운) | perspective 유지 (직교 아님 — 그림자/깊이감) |
| 렌더 | `PrimitiveMesh`/`MaterialDesc` (colored Blinn-Phong), `with_shadows(true)` | cube 벽·sphere 유닛·plane 바닥, 역할별 색. PBR 비범위 |
| 레벨 | Rust 상수 ASCII 그리드 → `scene_builder` 에서 엔티티 생성 | Rust 올인 결정에 따라 씬 파일 저작 비범위 |
| NavMesh | `NavMeshBuilder`(voxelization) 로 던전 전체 1개 자동 생성, 호출자 소유 | **미실전 영역 1순위.** `nav_agent_system(world, navmesh, dt)` 매 프레임 주입 |
| 적 AI | `NavAgent` + 재경로 `Timer`(0.5s), 사수는 거리 유지 steering | kinematic — RigidBody 커플링 없음 (CODEMAP 명시 한계) |
| 전투 | 투사체 = `spawn_bundle`(PrimitiveMesh + 자체 `Velocity` + `Lifetime`), 피격 = `Collider`(Sphere) + `set_collision_fn` 트리거 | 물리 트리거 경로 실전 첫 투입 |
| 스테이트 | `World` 리소스: `GamePhase`(Title/Playing/GameOver/Victory), `PlayerStats` | |
| 이벤트 | `Events<T>`: EnemyKilled / PlayerHit / RoomCleared | |
| UI | `with_hud(true)` + `set_hud_fn` — HP/방 표시, 타이틀·게임오버 화면도 HUD 텍스트+버튼 | HUD 로 풀스크린 메뉴 만들기 — 마찰 발견 예상 지점 |
| 오디오 | `with_audio(clips)` + `AudioSource` — 발사/피격/클리어 블립 + BGM 루프 | 에셋 부재 → 절차 생성 WAV 를 스크립트로 만들어 `assets/` 커밋 |
| 빌드 | `voltex.toml` + `voltex build` → exe | CLI E2E 재검증 |

## 구조 (`games/voltex_depths/`, flagship 레이아웃 계승)

```
src/main.rs        — RuntimeConfig 조립만 (~50줄 목표)
src/level.rs       — ASCII 그리드 → 벽/문/스폰포인트 엔티티 + NavMesh 빌드 (순수 함수)
src/combat.rs      — 발사/피격/HP/투사체 시스템 (순수 로직 분리)
src/enemy.rs       — AI 시스템 (NavAgent 재경로, 사수 거리 유지, 보스 패턴)
src/game_state.rs  — GamePhase 전이, 방 클리어/문 개폐, HUD 콜백
assets/            — 절차 생성 WAV (생성 스크립트 포함)
voltex.toml
```

게임 코드도 엔진과 같은 규율: 600줄 한도, 순수 로직과 World 배선 분리.

## 검증 (TDD)

- **단위**: 그리드 파싱→엔티티 좌표, 데미지/HP, 문 열림 조건, GamePhase 전이,
  사수 거리 유지 로직 — 전부 GPU 무관
- **헤드리스 통합**: `tick_systems_only` 로 "적 스폰 → 추적 → 피격 → 방 클리어"
  시나리오 (gameplay_slice.rs 패턴)
- **캡처 회귀 1개**: 던전 씬 non-blank + 역할색 픽셀 (RTX 4080, `voltex_capture`)
- **스모크 런**: 8초 실행 0 에러
- 수동 플레이 검증은 사용자 배치 패스로 일괄

## 마찰 로그 (이 프로젝트의 진짜 산출물)

작업 중 발견하는 엔진 마찰을 `docs/dogfood-friction.md` 에 누적 기록:
- 항목: 무엇을 하려다 / 무엇이 없거나 어색했나 / 임시 우회 / 제안 (P0~P2)
- 엔진 기능 즉석 구현은 **게임 진행을 실제로 막을 때만** (그 외는 기록 후 우회)
- 게임 완성 후 마찰 로그가 v1.1 로드맵 입력이 된다

## 비범위 (YAGNI)

절차 생성 던전, 세이브/로드, 업그레이드·메타 진행, 적 3종 이상, 씬 파일 레벨 저작,
PBR/스킨드 메시, 멀티플레이, 패드 입력.

## 리스크

- `NavMeshBuilder` voxelization / `nav_agent_system` 실전 첫 투입 — 버그 가능성 높음.
  발견 시 엔진 회귀 테스트와 함께 수정 (도그푸딩의 수확으로 취급)
- HUD 기반 풀스크린 메뉴는 IMGUI 위젯 표현력 한계에 부딪힐 수 있음 — 마찰 로그 후보
- kinematic NavAgent 와 Collider 트리거의 상호작용(겹침 해소 없음)은 게임 감각 튜닝 필요
