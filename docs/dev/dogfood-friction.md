# 도그푸딩 마찰 로그 — voltex_depths

게임 제작 중 발견한 엔진 워크플로 마찰. 항목: 하려던 것 / 없거나 어색했던 것 / 우회 / 제안(P0~P2).
엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 v1.1 로드맵 입력이 된다.

## 현재 상태 요약

**2026-07-07: 전 항목 해소 완료 (9/9).** 게임 완성 시점(2026-06-12) 기준 총 9건 중 3건은 게임 제작 중 해소, 잔여 6건(P1 4 + P2 2)은 2026-07-07 엔진 세션에서 일괄 해소. 이 로그는 v1.1 로드맵 입력으로서 소임을 다했고, 다음 도그푸딩에서 새 로그를 시작한다.

- ~~#1 NavMeshBuilder 미배선 (침묵하는 죽은 코드)~~ — **해소** (lib.rs 배선 + bitrot 수정). 후속이던 미배선 .rs 검출도 **해소** — `crates/voltex/tests/module_wiring.rs` 워크스페이스 가드 신설, 첫 실행에서 obstacle.rs / coroutine.rs / sandbox.rs 3건 추가 적발·배선 (+23 테스트 부활)
- ~~#2 `NavAgent.target` 직접 대입 footgun~~ — **해소** (`planned_target` 추적으로 시스템이 target 변경 감지 후 자동 repath; 새 target이 기존 path 끝점과 같은 삼각형이면 A* 없이 끝점만 교체하는 retarget)
- ~~#3 `follow_path` 도달 판정 3D 거리~~ — **해소** (waypoint를 에이전트 y 평면에 투영 — 도달 판정 XZ + 조향 force 평면 유지, goal snap이 agent y 보존. 가라앉음 자체가 사라짐)
- ~~#4 action system에서 창 크기 접근 불가~~ — **해소** (`InputState::window_size()` 물리 픽셀 노출, runtime이 resumed/Resized에서 주입)
- ~~#5 오디오 경로 이원화~~ — **해소** (`AudioClipCache::seed` — RuntimeApp 부트가 with_audio_files 매핑을 시드, 같은 파일 AudioPath는 기존 클립 재사용 + 핫리로드 공유)
- ~~#6 게임 에셋 루트 개념 부재~~ — **해소** (`RuntimeConfig::with_asset_root`; 상대 루트는 exe 디렉토리 기준 해석. read_asset 순서: Vfs → asset_root → cwd)
- ~~#7 게임 crate의 임의 World 픽셀 검증 진입점 부재~~ — **해소** (`voltex_capture::render_world` 공개)
- ~~#8 `voltex build`가 Rust 로직 게임 패키징 불가~~ — **해소** (`[build] kind = "rust"` — 게임 crate cargo build + dist에 exe/에셋/라이선스 레이아웃; depths가 실제 사용, 외부 cwd 실행 검증 통과)
- ~~#9 루핑 AudioSource despawn 시 voice 잔류~~ — **해소** (voltex_audio에 `VoiceId`/`stop_voice` 신설, audio_sync가 entity별 voice 추적 + entity 소멸 시 orphan voice 자동 stop — 수동 stop 우회 불필요)

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
| 1 | 레벨 바닥 지오메트리로 NavMesh 자동 생성 (`NavMeshBuilder`) | `navmesh_builder/`가 M3 분리 이후 lib.rs에 미선언 — 모듈·테스트 7개가 한 번도 컴파일된 적 없는 죽은 코드였고 CODEMAP은 공개 API로 등재 (obstacle.rs도 동일 상태로 남아 있음) | lib.rs 배선 + 첫 컴파일에서 드러난 bitrot(visibility/dead_code/clippy) 수정 후 사용 — 배선 후 7개 테스트·게임 detour 테스트 전부 그린, 파라미터(cell_size 0.5, agent_radius 0.4)는 기본 플랜 그대로 동작 | 모듈 분리 리팩토링 시 "선언 누락 = 침묵하는 죽은 코드" 감지 수단 필요 (CI에서 src/ 하위 미배선 .rs 검출 스크립트 등). obstacle.rs 배선 여부 결정 필요 | P1 |
| 2 | 추적자가 움직이는 플레이어를 따라가도록 매 repath 틱마다 `NavAgent` 목표 갱신 | `agent.target = Some(p)` 직접 대입은 캐시된 `path`/`reached`를 무효화하지 않음 — `nav_agent_system`은 `path.is_empty() && !reached`일 때만 재탐색하므로 직접 대입은 조용히 무시되고, 도달 후엔 영구 정지하는 footgun. 필드가 `pub`이라 컴파일러가 못 막음 | `set_target()` 사용 (path/reached 리셋 → 재탐색). 단 set_target은 목표가 거의 안 움직였어도 전체 A* 재계산 | NavAgent 필드 캡슐화(target을 직접 대입 불가로) 또는 nav_agent_system이 target 변경을 감지해 스스로 repath. 보너스: 목표 이동량이 작으면 기존 path 재사용하는 retarget API | P1 |
| 3 | 적이 NavMesh 경로(waypoint y=0)를 따라가되 게임플레이 y(=0.5) 유지 | `follow_path`의 waypoint 도달 판정이 3D 거리 — 호출자가 y를 고정하면 dist≥0.5 > 도달 반경이라 중간 waypoint에 영원히 못 도달. 보정 안 하면 에이전트가 navmesh 평면(y=0)으로 가라앉음 | y 보정 없이 가라앉게 둠 (최종 goal이 y=0.5라 도달 시 복귀; 렌더 높이는 후속 태스크에서 처리 예정) | 경로 추종/도달 판정을 XZ 거리로 (A* 휴리스틱은 이미 `distance_xz` 사용 — follow_path만 3D) | P1 |
| 4 | 마우스 조준: 화면 픽셀 → NDC 변환에 현재 창 크기 필요 | action system(`add_action_system`)에서 현재 창/서피스 크기를 읽을 경로가 없음 — `InputState`는 마우스 좌표만 주고 창 크기는 RuntimeState 내부에만 있음. 추가로 상수 우회는 물리/논리 픽셀 불일치(mouse_position은 물리, 창 설정은 논리)로 DPI 배율 125%+에서 조준이 어긋남 | **해소됨**: `InputState::window_size()`/`set_window_size()` 추가 (물리 픽셀 — mouse_position과 동일 단위), runtime handler가 창 생성(resumed) + Resized에서 주입. 게임은 SCREEN 상수 대신 실제 창 크기로 NDC 계산 + 조준점 디버그 마커(DebugLines cross)로 가시화 | — | P1 |
| 5 | 효과음/BGM 재생: `with_audio_files`로 프리로드한 클립 사용 | 오디오 경로가 이원화 — `with_audio_files`(인덱스=ClipId, 핫리로드 워치)와 `AudioPath`(경로→clip 런타임 resolve, resolver가 clip_id 소유). 둘을 같이 쓰면 같은 파일을 이중 디코드해 별도 슬롯에 올리고, AudioPath가 resolve한 클립은 핫리로드 대상이 아님 | `AudioPath`를 쓰지 않고 `with_audio_files` 인덱스를 CLIP_* 상수로 직접 사용 | with_audio_files가 등록한 path→ClipId 매핑을 AudioPath resolver(AudioClipCache)에 시드해 두 경로를 합치기 | P2 |
| 6 | 에셋 경로를 cwd 무관하게 지정 | 런타임에 "게임 에셋 루트" 개념이 없음 — 상대 경로는 cwd 의존, Vfs는 씬/스크립트 로드에만 적용 | `concat!(env!("CARGO_MANIFEST_DIR"), "/assets")` — 빌드 머신 절대 경로라 배포 exe에선 깨짐 | RuntimeConfig에 asset root 설정 + exe 기준 상대 해석 (voltex_cli 빌드 파이프라인과 연계) | P2 |
| 7 | 게임이 자기 던전 씬을 GPU 픽셀로 회귀 검증 | voltex_capture의 `render_*`가 전부 자기 씬을 하드코딩 — 게임 crate가 임의 World를 오프스크린 렌더할 공개 진입점 부재 | **해소됨(이 커밋)**: `render_declarative_colored_scene`에서 렌더 경로를 기계적 추출해 `voltex_capture::render_world(gpu, w, h, &mut world, cam_pos, yaw, pitch)` 공개 | — | P1 |
| 8 | 완성된 게임을 `voltex build`로 릴리스 패키징 | `voltex build`의 wrapper 코드젠이 데이터 주도 게임(scene.vscn + Lua) 전제 — `build.scene` 부재 시 즉시 실패하고, 더미 씬을 넣어도 wrapper exe엔 runtime+vpkg만 담겨 게임 crate의 Rust 시스템이 통째로 빠짐. Rust 로직 게임은 CLI 빌드 파이프라인(에셋 번들·라이선스 동봉 포함)을 아예 못 탐 | `cargo build --release -p voltex_depths`로 대체 — 단 에셋 번들링 없음 (#6 절대경로 문제와 결합돼 배포 exe 불가) | wrapper 코드젠에 "게임 crate를 dependency로 추가 + 사용자 `fn configure(RuntimeConfig)->RuntimeConfig` 훅 호출" 모드, 또는 manifest에 `[build] kind = "rust"` 추가해 cargo build + 에셋 번들만 수행하는 경로 신설 | P1 |
| 9 | 게임오버/클리어 후 R로 재시작할 때 BGM 중지 | 루핑 AudioSource 엔티티를 `world.clear()`로 despawn하면 playing=false 엣지가 발생하지 않아 mixer의 루핑 voice가 해제되지 않음 — 재시작마다 BGM voice가 누적 중첩됨. audio_sync는 엔티티 소멸을 감지하지 않아 Stop 커맨드를 발행하지 못하는 엔진 갭 | `world.clear()` 전에 BGM 엔티티를 찾아 `src.stop()` 호출(playing=false). audio_sync가 같은 프레임 내(action_systems 직후)에 Stop을 발행하므로 world.clear()는 다음 스텝(Space→start_game)으로 위임 | audio_sync가 자기가 발행한 voice를 entity별 `voice_id`로 추적해 entity 소멸(despawn/clear) 시 orphan voice를 자동 stop — 사용자가 stop()을 수동 호출할 필요 없어짐 | P1 |
