# 도그푸딩 마찰 로그 — voltex_depths

게임 제작 중 발견한 엔진 워크플로 마찰. 항목: 하려던 것 / 없거나 어색했던 것 / 우회 / 제안(P0~P2).
엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 v1.1 로드맵 입력이 된다.

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
| 1 | 레벨 바닥 지오메트리로 NavMesh 자동 생성 (`NavMeshBuilder`) | `navmesh_builder/`가 M3 분리 이후 lib.rs에 미선언 — 모듈·테스트 7개가 한 번도 컴파일된 적 없는 죽은 코드였고 CODEMAP은 공개 API로 등재 (obstacle.rs도 동일 상태로 남아 있음) | lib.rs 배선 + 첫 컴파일에서 드러난 bitrot(visibility/dead_code/clippy) 수정 후 사용 — 배선 후 7개 테스트·게임 detour 테스트 전부 그린, 파라미터(cell_size 0.5, agent_radius 0.4)는 기본 플랜 그대로 동작 | 모듈 분리 리팩토링 시 "선언 누락 = 침묵하는 죽은 코드" 감지 수단 필요 (CI에서 src/ 하위 미배선 .rs 검출 스크립트 등). obstacle.rs 배선 여부 결정 필요 | P1 |
| 2 | 추적자가 움직이는 플레이어를 따라가도록 매 repath 틱마다 `NavAgent` 목표 갱신 | `agent.target = Some(p)` 직접 대입은 캐시된 `path`/`reached`를 무효화하지 않음 — `nav_agent_system`은 `path.is_empty() && !reached`일 때만 재탐색하므로 직접 대입은 조용히 무시되고, 도달 후엔 영구 정지하는 footgun. 필드가 `pub`이라 컴파일러가 못 막음 | `set_target()` 사용 (path/reached 리셋 → 재탐색). 단 set_target은 목표가 거의 안 움직였어도 전체 A* 재계산 | NavAgent 필드 캡슐화(target을 직접 대입 불가로) 또는 nav_agent_system이 target 변경을 감지해 스스로 repath. 보너스: 목표 이동량이 작으면 기존 path 재사용하는 retarget API | P1 |
| 3 | 적이 NavMesh 경로(waypoint y=0)를 따라가되 게임플레이 y(=0.5) 유지 | `follow_path`의 waypoint 도달 판정이 3D 거리 — 호출자가 y를 고정하면 dist≥0.5 > 도달 반경이라 중간 waypoint에 영원히 못 도달. 보정 안 하면 에이전트가 navmesh 평면(y=0)으로 가라앉음 | y 보정 없이 가라앉게 둠 (최종 goal이 y=0.5라 도달 시 복귀; 렌더 높이는 후속 태스크에서 처리 예정) | 경로 추종/도달 판정을 XZ 거리로 (A* 휴리스틱은 이미 `distance_xz` 사용 — follow_path만 3D) | P1 |
| 4 | 마우스 조준: 화면 픽셀 → NDC 변환에 현재 창 크기 필요 | action system(`add_action_system`)에서 현재 창/서피스 크기를 읽을 경로가 없음 — `InputState`는 마우스 좌표만 주고 창 크기는 RuntimeState 내부에만 있음 | 창 크기를 상수(`SCREEN`)로 고정 — 리사이즈하면 조준이 틀어짐 | InputState 또는 콜백 인자에 화면 크기 노출 (UiContext는 이미 screen_width를 가짐 — 입력 쪽에도 동일하게) | P1 |
| 5 | 효과음/BGM 재생: `with_audio_files`로 프리로드한 클립 사용 | 오디오 경로가 이원화 — `with_audio_files`(인덱스=ClipId, 핫리로드 워치)와 `AudioPath`(경로→clip 런타임 resolve, resolver가 clip_id 소유). 둘을 같이 쓰면 같은 파일을 이중 디코드해 별도 슬롯에 올리고, AudioPath가 resolve한 클립은 핫리로드 대상이 아님 | `AudioPath`를 쓰지 않고 `with_audio_files` 인덱스를 CLIP_* 상수로 직접 사용 | with_audio_files가 등록한 path→ClipId 매핑을 AudioPath resolver(AudioClipCache)에 시드해 두 경로를 합치기 | P2 |
| 6 | 에셋 경로를 cwd 무관하게 지정 | 런타임에 "게임 에셋 루트" 개념이 없음 — 상대 경로는 cwd 의존, Vfs는 씬/스크립트 로드에만 적용 | `concat!(env!("CARGO_MANIFEST_DIR"), "/assets")` — 빌드 머신 절대 경로라 배포 exe에선 깨짐 | RuntimeConfig에 asset root 설정 + exe 기준 상대 해석 (voltex_cli 빌드 파이프라인과 연계) | P2 |
