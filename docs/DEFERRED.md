# 미뤄진/간소화된 구현 항목

## Phase 2

- ~~**PNG 디코더 자체 구현**~~ ✅ 완료.
- ~~**JPG 디코더 자체 구현**~~ ✅ Baseline JPEG 완료.
- ~~**glTF 파서**~~ ✅ glTF 2.0 / GLB 완료.
- ~~**JPG Progressive**~~ ✅ SOF2 감지 + 스캔 파라미터 파싱 + DCT 계수 병합 완료.
- ~~**glTF 애니메이션/스킨 파싱**~~ ✅ nodes/skins/animations/JOINTS_0/WEIGHTS_0 파싱 완료. **스킨드 렌더링 + 애니메이션 시스템** 미구현.

## Phase 3a

- **Archetype 기반 스토리지** → SparseSet 사용 중. 대규모 씬에서 성능 이슈 시 전환.
- ~~**시스템 스케줄러**~~ ✅ 순서 기반 Scheduler 완료.
- ~~**쿼리 필터**~~ ✅ With/Without 완료.
- ~~**query3+**~~ ✅ query3, query4 완료.
- ~~**Changed 필터**~~ ✅ tick 기반 변경 감지, query_changed, clear_changed 완료.
- **의존성 기반 스케줄러** — 읽기/쓰기 의존성 자동 정렬/병렬 실행 미구현.

## Phase 3b

- ~~**JSON 직렬화**~~ ✅ serialize_scene_json/deserialize_scene_json 완료.
- ~~**바이너리 씬 포맷**~~ ✅ VSCN 바이너리 포맷 완료.
- ~~**임의 컴포넌트 직렬화**~~ ✅ ComponentRegistry 등록 기반 완료.

## Phase 3c

- ~~**비동기 로딩**~~ ✅ AssetLoader (워커 스레드) 완료.
- ~~**핫 리로드**~~ ✅ FileWatcher (mtime 폴링) + replace_in_place 완료.

## Phase 4a

- ~~**Metallic/Roughness/AO 텍스처 맵**~~ ✅ ORM 텍스처 샘플링 완료.
- ~~**Emissive 맵**~~ ✅ Emissive 텍스처 + 셰이더 완료.

## Phase 4b

- ~~**CSM (Cascaded Shadow Maps)**~~ ✅ 2-캐스케이드 CSM 완료.
- ~~**Point Light Shadow (큐브맵)**~~ ✅ 큐브맵 depth + 6면 렌더링 완료.
- ~~**Spot Light Shadow**~~ ✅ Perspective shadow map 완료.
- ~~**라이트 컬링**~~ ✅ CPU 프러스텀 컬링 완료.

## Phase 4c

- ~~**HDR 큐브맵 환경맵**~~ → 프로시저럴 Hosek-Wilkie sky로 대체 완료.
- ~~**Irradiance/Prefiltered Map 컨볼루션**~~ ✅ SH Irradiance (L2, 9계수) 완료.
- ~~**GPU 컴퓨트 BRDF LUT**~~ ✅ 컴퓨트 셰이더 Rg16Float 완료.

## Phase 5-1

- ~~**Capsule, Convex Hull 콜라이더**~~ ✅ Capsule + ConvexHull + GJK/EPA 완료.
- ~~**OBB (회전된 박스) 충돌**~~ ✅ GJK/EPA로 대체 완료.
- ~~**Incremental BVH 업데이트**~~ ✅ refit() 완료.
- ~~**연속 충돌 감지 (CCD)**~~ ✅ swept_sphere_vs_aabb 완료.

## Phase 5-2

- ~~**각속도/회전 물리**~~ ✅ 관성 텐서 + angular velocity 적분 완료.
- ~~**마찰 (Coulomb)**~~ ✅ 완료.
- ~~**Sequential Impulse 솔버**~~ ✅ N회 반복 솔버 완료.
- ~~**Sleep/Island 시스템**~~ ✅ velocity threshold + timer 완료.

## Phase 5-3

- ~~**Ray vs Triangle**~~ ✅ Möller–Trumbore 완료.
- ~~**Ray vs Mesh**~~ ✅ MeshCollider + ray_vs_mesh + ray_vs_mesh_all 완료. ~~**Ray vs Plane**~~ ✅ 완료.
- ~~**raycast_all (다중 hit)**~~ ✅ 거리순 정렬 완료.
- ~~**BVH 조기 종료 최적화**~~ ✅ 재귀 트리 순회 + query_ray 완료.

## Phase 6-1

- ~~**macOS/Linux 백엔드**~~ ✅ Cycle 10 (ALSA) + Cycle 11 (CoreAudio) 코드 완료. 실 머신 runtime 검증 v1.1.
- ~~**OGG/Vorbis 디코더**~~ ✅ OGG 컨테이너 + Vorbis 디코더 완료.
- ~~**24-bit/32-bit WAV**~~ ✅ 완료.
- ~~**ECS 통합**~~ ✅ AudioSource 컴포넌트 완료. audio_sync_system 미구현.
- ~~**비동기 로딩**~~ ✅ AsyncAudioLoader (스레드 기반) 완료.

## Phase 6-2

- ~~**도플러 효과**~~ ✅ doppler_shift 완료.
- ~~**HRTF**~~ ✅ ITD/ILD 기반 간소화 HRTF 완료.
- ~~**Reverb/Echo**~~ ✅ Schroeder reverb (4 comb + 2 allpass) + Echo delay line 완료.
- ~~**Occlusion**~~ ✅ 레이 기반 차폐 + 로우패스 필터 완료.

## Phase 6-3

- ~~**동적 그룹 생성**~~ ✅ MixGroupManager 동적 생성/삭제 완료.
- ~~**그룹 간 라우팅/버스**~~ ✅ AudioBus 믹싱 완료.
- ~~**이펙트 체인**~~ ✅ EffectChain (trait 기반 파이프라인) 완료.
- ~~**비선형 페이드 커브**~~ ✅ Exponential/Logarithmic/SCurve 완료.

## Phase 7-1

- ~~**투명 오브젝트**~~ ✅ ForwardPass (알파 블렌딩, HDR 타겟, depth read-only) + back-to-front 정렬 완료. deferred_demo 통합 미완료.
- ~~**G-Buffer 압축**~~ ✅ 옥타헤드럴 노멀 인코딩 + depth 기반 위치 복원 완료.
- ~~**Light Volumes**~~ ✅ Sphere/Cone/Fullscreen 볼륨 + contains_point 완료.
- ~~**Stencil 최적화**~~ ✅ 라이트 볼륨 스텐실 mark/test 완료.

## Phase 7-2

- ~~**Bilateral Blur**~~ ✅ depth/normal edge-aware 컴퓨트 셰이더 완료.
- ~~**반해상도 렌더링**~~ ✅ half_resolution + depth-aware 업스케일 완료.
- ~~**Temporal Accumulation**~~ ✅ 히스토리 블렌딩 컴퓨트 셰이더 완료.
- ~~**Light Probes**~~ ✅ SH 평가 + LightProbeGrid nearest 완료.

## Phase 7-3

- ~~**RT Reflections**~~ ✅ 컴퓨트 셰이더 (G-Buffer 기반 반사 레이 마칭) 완료.
- ~~**RT AO**~~ ✅ 컴퓨트 셰이더 (코사인 가중 반구 샘플링) 완료.
- ~~**Point/Spot Light RT shadows**~~ ✅ 인프라 + 컴퓨트 셰이더 완료.
- ~~**Soft RT shadows**~~ ✅ 멀티 레이 + penumbra 크기 계산 완료.
- ~~**BLAS 업데이트**~~ ✅ BlasTracker (dirty mesh 추적) 완료.
- ~~**Fallback**~~ ✅ RtCapabilities + RenderMode 선택 완료.

## Phase 7-4

- ~~**TAA**~~ ✅ Halton jitter + neighborhood clamping + history blending 컴퓨트 셰이더 완료.
- ~~**SSR**~~ ✅ 스크린 스페이스 레이 마칭 + edge fade 컴퓨트 셰이더 완료.
- ~~**Motion Blur**~~ ✅ 카메라 속도 기반 컴퓨트 셰이더 완료.
- ~~**DOF**~~ ✅ Circle-of-confusion 기반 컴퓨트 셰이더 완료.
- ~~**Auto Exposure**~~ ✅ 컴퓨트 셰이더 luminance + 적응형 노출 계산 완료. deferred_demo 통합 미완료.
- ~~**Bilateral Bloom Blur**~~ ✅ luminance 기반 bilateral bloom weight 완료.

## Phase 8-1

- ~~**자동 내비메시 생성**~~ ✅ 복셀화 기반 NavMeshBuilder 완료.
- ~~**String Pulling (Funnel)**~~ ✅ SSF 알고리즘 완료.
- ~~**동적 장애물 회피**~~ ✅ velocity obstacle 기반 완료.
- ~~**ECS 통합**~~ ✅ NavAgent 컴포넌트 완료. nav_agent_system 미구현.
- ~~**내비메시 직렬화**~~ ✅ VNAV 바이너리 포맷 serialize/deserialize 완료.

## Phase 8-2

- ~~**상태 동기화 (스냅샷)**~~ ✅ Snapshot + delta 압축 완료.
- ~~**보간 / 예측**~~ ✅ InterpolationBuffer 완료.
- ~~**지연 보상**~~ ✅ LagCompensation (history rewind + interpolation) 완료.
- ~~**신뢰성 계층**~~ ✅ ReliableChannel + OrderedChannel 완료.
- ~~**암호화 / 인증**~~ ✅ PacketCipher (XOR + anti-replay) + AuthToken 완료.

## Phase 8-3

- ~~**핫 리로드**~~ ✅ reload_file 완료.
- ~~**엔진 API 노출**~~ ✅ 기본 API (spawn, position, entity_count, velocity, rotation, scale, destroy, find_by_tag) 완료.
- ~~**Lua 테이블 ↔ Rust 구조체**~~ ✅ push_table/read_table/push_vec3/read_vec3 완료.
- ~~**코루틴**~~ ✅ LuaCoroutine (create/resume/is_finished) 완료.
- ~~**샌드박싱**~~ ✅ os/io/loadfile/dofile/require 차단 완료.

## Phase 8-4

- ~~**씬 뷰포트**~~ ✅ 오프스크린 렌더링 + 오빗 카메라 완료. Blinn-Phong forward만 (디퍼드/PBR 미지원).
- ~~**엔티티 인스펙터**~~ ✅ Transform/Tag/Parent 편집 완료. 커스텀 컴포넌트 리플렉션 미구현.
- ~~**에셋 브라우저**~~ ✅ 파일 목록 + 디렉토리 탐색 완료. 에셋 로딩/프리뷰 미구현.
- ~~**텍스트 입력**~~ ✅ text_input 위젯 완료.
- ~~**스크롤, 드래그앤드롭**~~ ✅ scroll_panel + drag/drop 완료.
- ~~**도킹, 탭**~~ ✅ DockTree + ~~**플로팅 윈도우**~~ ✅ FloatingPanelManager 완료. 탭 드래그 이동 미구현.
- ~~**TTF 폰트**~~ ✅ 자체 TTF 파서 + 래스터라이저 + 온디맨드 캐시 + 렌더러 아틀라스 교체 완료.

## 렌더링 한계

- ~~**per-entity dynamic UBO**~~ ✅ GPU 인스턴싱 (InstanceData, InstanceBuffer, instanced pipeline) 완료.
- **max_bind_groups=4** — 리소스 합치기로 해결 중.

## Phase 8-7 (Picking + Editor 후속)

- ~~**viewport raycast picking**~~ ✅ sphere 기반 (`picking::pick_entity`). 좌클릭 → `session.selected`.
- ~~**Lua hierarchy API**~~ ✅ add_child / remove_child / find_all_by_tag 추가.
- ~~**editor_demo 시드의 command 경로 통합**~~ ✅ Save/Load round-trip 일관성 확보.
- ~~**메시 AABB picking**~~ ✅ Cycle 3 (E1) — `Bounds` 컴포넌트 있는 entity 는 8-corner AABB world transform 후 ray vs AABB. 없으면 sphere fallback.
- ~~**gizmo (translate/rotate/scale)**~~ ✅ Cycle 3 (E1) — translate/rotate/scale 3축 핸들 + LINE wgpu 파이프라인. End-drag 시 SetTransform 단일 undo 엔트리.
- ~~**Despawn 정확한 undo**~~ ✅ Cycle 12 (E3) — `EntityAllocator::restore_at_generation` + 등록된 모든 컴포넌트 snapshot/restore. 슬롯 재사용 시 silent skip.

## v1.0 / v1.0.1 / v1.0.2 / v1.0.3 / v1.0.4 / v1.0.5 / v1.0.6 / v1.0.7 / v1.0.8 추가 한계

- **3 OS exe artifact 검증** — Windows host 빌드 + runtime 검증만. Linux + macOS 실 머신 검증 v1.1 사용자 PR.
- **Lua input API** — 입력은 Rust `add_input_system` 만. Lua 측 `input.is_key_pressed(...)` v1.1 후속.
- **voltex_cli editor 모드** — `voltex new/build/run` 만. `voltex editor` 서브커맨드 v1.1.
- **HUD 통합** — 런타임 UiContext 정식 API 없음. 점수 등 표시는 콘솔 print 만.
- **1573 missing_docs** — facade prelude + tier-1 API 만 보장. crate 별 점진 작성 v1.1.
- **`pub(crate)` 다운그레이드** — 후보 노트 (`docs/v1.0-pub-audit.md`) 만 작성. 실 변경 v1.1.
- **600줄 한도 위반 4건** ✅ v1.0.2 에서 재분리.
- **macOS/Linux 백엔드 runtime** — Cycle 10 (P1) ALSA + Cycle 11 (B3) CoreAudio 코드 작성 완료. 실 머신 audio 출력 v1.1.
- **`voltex build` Windows P0** ✅ v1.0.3 fix (canonicalize \\?\ verbatim prefix strip).
- **clippy approx_constant 9 errors** ✅ v1.0.4 fix.
- **clippy warnings 84 → 0** ✅ v1.0.5 (43 auto-fix + 24 manual + 2 dead code 삭제).
- **`cargo audit` 0 vulnerabilities** ✅ v1.0.5 검증. 1 advisory (paste 1.0.15 unmaintained, transitive via wgpu/metal) — 통제 불가, 모니터링만.
- **CI 워크플로 (.github/workflows/ci.yml)** ✅ v1.0.5 추가. 단, **실제 GitHub runner 첫 green run 은 mirror 필요** — git.tolelom.xyz 가 GitHub Actions 실행 환경 미보유 시 inert. self-hosted gitea/forgejo runner 또는 GitHub mirror push 가 v1.1 후속.
- **`cargo audit` CI 통합** — cargo-audit binary install step 추가 필요. v1.1 후속.
- **GUI 시각 검증** — viewport / gizmo / editor / 게임 화면 실제 동작 0건 확인. 6초 smoke-test 는 panic 없음만 보장.
- **coverage 측정** ✅ v1.0.6 — baseline 82.06% functions / 82.74% lines (cargo llvm-cov --workspace --lib). 80% 목표 평균 달성.
- **CLI 친화 메시지 (engine-path)** ✅ v1.0.6 — 1-line → 7-line help block (bash + PowerShell 양쪽 안내).
- **GPU/winit 의존 4 crate (renderer 66 / runtime 61 / renderer_ecs 54 / platform 46%) 의 coverage 보완** — headless GPU adapter + event-loop stub 필요. v1.1+.
- **`--fail-under-lines 80` CI gate** — 4 low-coverage crate 통합 테스트 도입 후 활성. v1.1+.
- **HTML coverage 리포트 (codecov/coveralls upload)** — CI 가 실제 runner 에서 동작한 후. v1.1+.
- **외부 의존 라이선스 고지** ✅ v1.0.7 — THIRD-PARTY-LICENSES.md (4,795 라인) + about.toml/hbs + CI license-check job. 284 transitive 모두 permissive, 0 copyleft.
- **license-check advisory → blocking** — CI 첫 green run 후 승격. v1.1+.
- **THIRD-PARTY-LICENSES.md 를 `voltex build` artifact 에 동봉** — 현재 `dist/<name>.exe` 옆에 안 들어감. v1.1+.
- **`cargo-deny` 통합** — license + advisory + duplicate 통합 스캔. 현재 cargo-audit + cargo-about 으로 대체 가능. v1.1+.
- **parser panic-resistance fuzz** ✅ v1.0.8 — 21 신규 test (PNG/JPG/WAV/GLTF/OBJ), panic 0건. 외부 dep 없는 inline xorshift32.
- **OGG/Vorbis lib.rs 등록** ✅ v1.0.9 — `pub mod ogg; pub mod vorbis;` 등록 + clippy 27→0 (자동 5 + #![allow] 6 파일 + 진짜 fix 4) + `decode_ogg_vorbis_bytes` 진입점. 숨겨진 테스트 회귀 2건 (ogg invalid_capture_pattern, vorbis bitreader_basics) fix. voltex_audio 89→119 tests.
- **coverage-guided fuzzing (cargo-fuzz / honggfuzz)** — 현재 deterministic xorshift32 만. nightly toolchain + libfuzzer 인프라는 v1.1+.
- **실제 `.ogg` 파일 디코드 검증** — v1.0.9 는 unit test (≤100 byte 합성 입력) 만. Vorbis decoder 자체가 simplified (scalar codebook only, no coupled channels). 실 파일 + audio_demo 통합은 v1.0.x+.
- **crash-log panic hook** ✅ v1.0.10 — `voltex_runtime::crash` 모듈 + `RuntimeConfig::{enable_crash_log, crash_log_dir}` + 자동 `<app>-crash-UTC.txt` 작성. 외부 dep 0 (Howard Hinnant 알고리즘 inline). +14 tests. 한계: live panic 통합 테스트 미실시 (global hook state 격리 까다로움), 크래시 리포트에 GPU 어댑터/FPS/마지막 씬 정보 없음 — v1.1+.
- **voltex.toml strict schema + path validation** ✅ v1.0.11 — UnknownField/UnknownSection/ManifestPathMissing 에러 + Levenshtein ≤3 suggestion + `validate_paths` + `voltex build --no-validate-paths`. +13 tests. 한계: ManifestParse line-only (column 미지원), Levenshtein 동률시 알파벳 순 — v1.1+.
