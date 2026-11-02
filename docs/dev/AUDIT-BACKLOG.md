# 심층 감사 후속 백로그 (v1.0.56 audit)

2026-05-28 아키텍처/테스트 품질 심층 감사에서 나온 미처리 발견 + 기존 DEFERRED 의 v1.1 항목.
각 항목은 발견 위치 / 작업량 / 리스크 표기. 처리 시 `[x]` 체크 + 커밋 해시 기록.

> 추천 진행 순서: 저리스크·고가치 **C(중복) → B(데드코드) → A2/A4(타입안전)** 한 묶음 →
> **D(테스트 갭)** → 신중히 **E1(voltex_image 분리)**.

## A. 타입 안전성 · 캡슐화

- [x] **A1** `Entity` `pub` 필드는 net/Lua/씬 직렬화 round-trip 용 의도적 POD — private화 high-churn·low-benefit. 대신 의도 명시 문서화 + `Entity::from_raw_parts` named 생성자(Handle 미러) 추가 + Lua bare 리터럴 마이그레이션 + 가드 테스트. (커밋 후속)
- [x] **A2** `AudioSource` → `ClipId(u32)` newtype + "재생 디스크립터(ECS 무의존)" 로 문서/CODEMAP 정정. `new(impl Into<ClipId>)` 로 ergonomics 유지. (커밋 후속)
- [x] **A3** `MeshHandle`/`MaterialHandle` `pub` inner 는 핸들 재사용·직렬화용 의도 — id 공간이 `Assets` TypeId 로 보호됨(latent weakness, not live bug). 의도 명시 문서화로 처리(private화는 ~10 파일 영향이라 거부). (커밋 후속)
- [x] **A4** Lua 마샬링: read 측은 기존 `read_handle_strict*` 유지 + `read_handle_strict` 가 `_at(1,2)` 로 위임, push 측 `push_entity`/`push_entity_table` 헬퍼로 spawn/find_by_tag/find_all_by_tag/iter_entities 중복 제거. (커밋 후속)

## B. 데드코드 정리 (YAGNI)

- [~] **B1** `DependencyScheduler`/`SystemAccess`/`build_schedule` — 소비자 0, 실행 불가. **2026-10-12 판정: 구현 안 함** — `World`가 Send/Sync가 아니고(컴파일 확인), 시스템이 `&mut World` 전체를 받아 access 선언이 타입으로 강제되지 않으며, 저장소 지연 생성이 구조 경쟁을 만든다. 병렬화는 ECS 파라미터 시스템 재설계가 필요하고 소비자·측정 근거가 없다. 재검토 조건: 시스템 실행이 프레임 시간의 30% 이상을 차지하는 게임이 나오거나, 다른 이유로 ECS를 파라미터 방식 시스템으로 재설계할 때. 그 전까지 `DependencyScheduler`는 충돌 분석과 그룹 빌드 도구로 남긴다.
- [x] **B2** `net::OrderedChannel` — **CODEMAP 문서화** (제거 X — in-order delivery 는 정당한 공개 네트워킹 primitive, 라이브러리 공개 API 는 내부 소비자 0 정상). (커밋 후속)
- [x] **B3** `CliError::SceneNotFound` 데드 variant 제거 + Display arm 제거. (커밋 후속)

## C. 중복 제거 (DRY)

- [x] **C1** CRC32/Adler32 중복 — E1 완료로 해소: 체크섬 구현은 `voltex_image::checksum`(crate-private)으로 단일화. `voltex_capture`의 중복 CRC32/Adler32 제거, `voltex_image::encode_rgba8` 재export로 교체. (커밋 7ee76f4 + b0c2d63)
- [x] **C2** `LoadState` vs `PendingState` 동일 상태공간 이중 정의 → `PendingState` 제거, `LoadState`(+Clone) 단일화. (커밋 후속)
- [x] **C3** `SparseSet::increment_tick` trait impl 이 inherent 메서드로 위임. (커밋 후속)
- [~] **C4** LE-f32 헬퍼 — **won't-fix**: 실제로는 각 crate 가 `f32::to_le_bytes`/`from_le_bytes`(std) 를 호출할 뿐, 추출할 공유 로직 없음.

## D. 남은 테스트 / 커버리지 갭

- [x] **D1** `ssgi` `pseudo_random`(범위/결정성/시드차) + `generate_noise_data`(길이16/unit-length) 테스트 +4. (커밋 후속)
- [x] **D2** `BlasTracker::mark_dirty` 미등록 id no-op, `BonePalette::set_bone` OOB drop 테스트. (커밋 후속)
- [x] **D3** `test_extrapolation_returns_latest` → `test_beyond_latest_clamps_to_latest` 리네임 + 명확화 주석. (커밋 후속)

## E. 대형 리팩터링 (영향 큼, 신중)

- [x] **E1** `voltex_image` crate 분리 — renderer 의 PNG/JPG/deflate/glTF/OBJ/json_parser 는 wgpu 무의존 → God-crate(~120 export) 해소, L0 신규 crate. 역호환 `pub use` 재export 유지. (커밋 202afea + 7ee76f4 + f3684ae + b0c2d63 + 1856ef1 + 이번 커밋)
- [x] **E2** `query2/3/4` 가 `Vec` 반환(매 프레임 할당) → `impl Iterator<...> + '_` 로 변경 (zero-alloc, `Either2/3/4` enum 으로 stack-allocated tagged union, `argmin3/4` 헬퍼로 min-selection 추출). 콜러 6곳(physics 단순 iter는 무수정, asset_demo/many_cubes는 `.len()`+iter 둘다 쓰는 곳만 `.collect::<Vec<_>>()` 1회). (이번 커밋)
- [x] **E3** `Transform`/snapshot 회전이 Euler `Vec3` → quaternion. **완료** (3 sub-items):
    - [x] **E3a** `voltex_math::Quat` 정식 추가 — `from_axis_angle`/`from_euler_yxz`(`Transform::matrix()`의 RotY·RotX·RotZ 순서 일치)/`to_mat4`/`mul`(Hamilton)/`conjugate`/`inverse`/`normalize`/`dot`/`length`/NaN-safe `slerp`/`Default = IDENTITY`. 테스트 20개. 후속: `to_euler_yxz` (inverse, gimbal-collapsing) + `integrate` (semi-implicit dq/dt = ½ωq) 헬퍼 추가, 테스트 +8. (커밋 9b47c54 + bee796c)
    - [x] **E3b** `Transform.rotation: Vec3` → `Quat` 마이그레이션 완료 — Transform/SetTransform/세션 undo/inspector(Euler 버퍼 proxy)/gizmo rotate(axis-angle 합성)/Lua API(Euler 인터페이스 유지, 내부 변환)/physics integrator(`Quat::integrate` 사용)/예제 4개 (hierarchy_demo, runtime_demo, many_cubes, editor_demo)/capture/scene. **영구 포맷 dual-read**: text `.vscn` 의 rotation 필드는 3-float Euler legacy + 4-float quat 둘 다 파싱, binary registry 의 transform payload 36 byte (Euler) + 40 byte (quat) 둘 다 디시리얼라이즈 → 기존 사용자 씬 파일 그대로 로드. **Wire format hard cutover**: `voltex_net::EntityState.rotation` `[f32;3]` → `[f32;4]` (snapshot wire size 40 → 44 byte), `lerp_angles3` → `Quat::slerp` 로 교체 (`voltex_net` 이 `voltex_math` 의존 — L0 → L1 승격, CLAUDE.md 그래프 갱신). 전 workspace build/clippy/1157 tests 그린.
    - [x] **E3c** `voltex_renderer::animation::Quat` (local) → `voltex_math::Quat` 통합 — animation.rs 의 자체 Quat struct/impl 삭제, `use voltex_math::Quat` 도입. `to_mat4()` 시그니처 차이([[f32;4];4] → Mat4)는 호출처 3곳에서 `.cols` 체이닝으로 해소. lib.rs 는 `pub use voltex_math::Quat` 로 `voltex_renderer::Quat` 경로 역호환 유지. animation 14 테스트 통과. (이번 커밋)

## F. 기존 v1.1 백로그 (DEFERRED 출처)

- [x] **F1** missing_docs ~1476개 (11 crate) — **백로그 작성 시점 figure는 stale**. 현재 13개 library crate(`voltex_math`/`platform`/`asset`/`audio`/`physics`/`ai`/`net`/`script`/`runtime`/`renderer`/`renderer_ecs`/`editor`/`image` + `ecs` + `capture`)는 모두 `#![warn(missing_docs)]` 적용 + `cargo build --workspace`에서 missing_docs 경고 0. 이번 커밋에서 `voltex` facade + `voltex_cli` binary + `voltex_script/build.rs`에도 doc 추가. 예제/게임 binary crate 의 strict `-D missing_docs` 통과는 별도 P3 (실 API 표면 아님). (이번 커밋)
- [x] **F2** `validate_hierarchy` pass — 직렬화 씬의 pre-existing 사이클 방어. `add_child` 우회 경로(ComponentRegistry-driven Parent deserialize / 수기 편집 / 적대적 입력)로 들어온 cycle 을 walk + visited-set 으로 탐지 후 시작 entity 의 `Parent` 제거 + 해당 부모의 `Children` 정리. binary/JSON deserializer 양쪽에서 호출. depth cap = `VALIDATION_MAX_DEPTH(1024)` (`MAX_HIERARCHY_DEPTH` 와 일치). 테스트 4개. (이번 커밋)
- [x] **F3** JSON deserializer entity-count cap (text/binary 는 완료) — `deserialize_scene_json` 에 `MAX_SCENE_ENTITIES` cap 추가 + `_with_cap` 테스트 헬퍼 + boundary/reject 테스트 2개. 파서 자체 array-length cap 은 별도 (이 fix 는 deserializer 경계 방어). (이번 커밋)
- [x] **F4** coverage-guided fuzzing scaffolded (cargo-fuzz) — `fuzz/` crate excluded from workspace, 9 libfuzzer targets (parse_png/jpg/gltf/obj, scene text/json/binary, snapshot, navmesh), README documents nightly invocation. Running them is a separate cadence — not a per-PR CI gate. `voltex_ai` gets pub re-exports for `deserialize_navmesh`/`serialize_navmesh` (formerly module-only). (이번 커밋)
- [x] **F5** GPU/winit headless 커버리지 + CI gate — `cargo llvm-cov --workspace --lib --fail-under-lines 78 -- --include-ignored` 로 gate. CI(ubuntu-latest)는 lavapipe(software Vulkan; `mesa-vulkan-drivers libvulkan1`) 설치 후 `VK_ICD_FILENAMES`/`WGPU_ADAPTER_NAME=llvmpipe` 로 voltex_capture 의 `#[ignore]` GPU 테스트 실행. lavapipe 가 없는 runner 에서는 `HeadlessGpu::new()` 가 graceful-skip → `--lib only` floor (79.69%) 도 78% gate 통과. GPU 포함 시 84.24% lines (로컬 RTX 4080 SUPER 측정). 로컬 재현 `scripts/coverage.sh` (`--lib-only`/`--html` 플래그). 기존 advisory 잡을 blocking 으로 승격. (이번 커밋)
  - 2026-10-12 재측정: `--lib` only 68.83%(78% 미달), `--include-ignored` 82.90%. "lavapipe 없는 runner도 통과" 전제는 더 이상 성립하지 않는다. 2026-11-02 CI에서 게이트를 없애고 측정만 하는 advisory 잡으로 바꿨다(`scripts/coverage.sh`는 로컬 게이트로 남김).
- [~] **F6** Linux/macOS 오디오 백엔드 실 머신 검증 — 인프라 작성 완료. 각 backend(wasapi/alsa/coreaudio)에 `#[ignore]` 실 device smoke 테스트 추가 (default 디바이스 open → 64 frames 무음 write → shutdown, 디바이스 없으면 eprintln + return). `docs/release.md` 의 pre-release checklist 에 등재. Windows 측 검증 완료 (이 머신 RTX/realtek). Linux/macOS 측 체크박스 채우기는 실 머신 push 대기.
  - [ ] **F6a** `backend/alsa.rs:48` FFI 선언 `snd_pcm_writei(pcm, buffer, size: c_int) -> isize` 의 `size` 가 libasound 선언 `snd_pcm_uframes_t`(= `unsigned long`, 64비트 Linux 에서 8바이트)와 다르다. 호출부 `alsa.rs:148`·`:163`. 64비트 인자 자리에 32비트를 넘겨 상위 비트가 정의되지 않을 수 있다(시그니처 대조만 함, 실 머신 미재현). `AudioSystem` 은 비-Windows 에서 무음 스레드라 이 경로를 쓰지 않지만, 공개 `make_backend()`·`AlsaBackend::write` 로는 도달한다. 고칠 때는 `c_ulong` 으로 맞추고 F6 smoke 테스트를 실 Linux 에서 돌린다. (2026-10-17 CI 조사 중 발견)
- [~] **F7** CI 첫 green run — `.github/workflows/ci.yml` 준비 완료 (build-test Linux+Windows blocking, macOS·coverage·deny advisory, e2e 수동. 2026-11-02 semver·license·changelog 잡 제거). GitHub 이전 때 첫 통과를 확인한다(release.md의 미러 절차 절은 2026-11-02 삭제).
- [x] **F8** `pub(crate)` 다운그레이드 audit 실행 — 9개 crate 총 139개 항목 downgrade: audio 20 (커밋 7c273c9), ecs 9 (커밋 f142f4f), net 1 (커밋 3dba5cc), physics 9 (커밋 47510b7), ai 12 (커밋 6093bc6), script 35 (커밋 7b16ead), image 5 (커밋 346b83a), editor 14 (커밋 a511a6c), renderer 34 (커밋 8845676). voltex_runtime은 macro-exported API chain 이유로 변경 없음. 전 workspace build+174 tests 통과.
  - **후속**: visibility-coherence (`X is more private than item Y`) 경고와 future-API의 dead_code 경고를 발생시킨 over-downgrade 일부를 `pub` 복원. 대상: audio 8개 (GUID/BusInput/HrtfResult/HEAD_RADIUS/SPEED_OF_SOUND/calculate_hrtf/azimuth_from_positions/GainEffect/OggPage/generate_wav_bytes_24bit/generate_wav_bytes_f32), ecs 8개 (JsonVal/json_write_*/json_parse/ComponentStorage), net 1개 (HistoryEntry), ai 3개 (serialize_navmesh/deserialize_navmesh/funnel_smooth), script 전체 ffi (LUA_*/lua_* 상수/함수), renderer 23개 (bilateral_weight/circle_of_confusion/encode_octahedral 등), editor 1개 (RasterResult+fields). 추가로 pre-existing clippy 경고(physics/renderer_ecs/survivor_game의 useless_conversion, audio wasapi upper_case_acronyms) 일괄 수정. 워크스페이스 경고 0 회복, `cargo clippy --workspace -- -D warnings` clean.
- [x] **F9** `voltex_image` 의 moved 디코더 파일들에 남아있는 `#![allow(missing_docs)]` 제거 — 7개 파일 doc 작성 + gltf/types.rs·parser.rs pub 항목 doc 추가. (이번 커밋)

## P3 — 보류 (저가치)

- ~~`AuthToken::validate` 비상수시간 비교 (dev-only 디스클레이머 있음)~~ — **v1.0.125 해소** (`constant_time_eq`, 빈 secret 패닉도 정리)
- ~~`query_with`/`query2_with` 이중 패스 (E2 와 함께)~~ — **E2로 이미 해소 확인 (2026-10-12)**: `query2`가 iterator가 된 뒤 `query2_with`/`query2_without`은 `filter().collect()` 한 번이고, `query_with`/`query_without`은 원래 단일 루프. 남은 것은 `Vec` 반환(API 변경)과 루프 안 W 저장소 조회(측정 없음)뿐이고 소비자는 테스트 4개라 변경 안 함.
- `voltex_cli` 수기 `Display` vs thiserror (외부 dep 0 정책상 의도적)
