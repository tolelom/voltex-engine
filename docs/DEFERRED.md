# 미뤄진/간소화된 구현 항목

## Phase 2

- ~~**PNG 디코더 자체 구현**~~ ✅ 완료.
- ~~**JPG 디코더 자체 구현**~~ ✅ Baseline JPEG 완료.
- ~~**glTF 파서**~~ ✅ glTF 2.0 / GLB 완료.
- ~~**JPG Progressive**~~ ✅ SOF2 감지 + 스캔 파라미터 파싱 + DCT 계수 병합 완료.
- ~~**glTF 애니메이션/스킨 파싱**~~ ✅ nodes/skins/animations/JOINTS_0/WEIGHTS_0 파싱 완료. ~~**스킨드 렌더링 + 애니메이션 시스템** 미구현~~ 🟡 2026-06-03 Blinn-Phong 경로 완료 — `voltex_renderer::Skeleton`(from_skin/apply_animation/compute_palette) + `create_skinned_mesh_pipeline`/`mesh_skinned.wgsl`(group0 camera, group1 bone palette, GPU vertex skinning). `voltex_capture::scene_skinned` 가 RTX 4080(Vulkan)에서 rest(직선)↔bent(굽힘) 2프레임 렌더 + 픽셀 차이 검증 (`docs/visual/skinned_{rest,bent}.png`). ~~ECS 통합~~ ✅ 2026-06-03 — `SkinnedMesh` 에셋 + `SkinnedMeshHandle`/`SkinnedPose` 컴포넌트 + `update_skinned_poses` 시스템(RuntimeApp tick 자동 호출) + `BasicRenderer` 2차 스킨드 패스(per-entity dynamic-offset palette/cam UBO, MAX_SKINNED=64). `game_ready_demo` 에 흔드는 totem 추가. `runtime_basic_renderer_skins_and_animates` GPU 회귀. ~~**PBR 경로 스키닝(bind group 재배치)** 은 여전히 미구현 — v1.1.~~ ✅ 2026-06-03 — `pbr_skinned.wgsl`(bone palette를 group 0에 camera+lights와 합쳐 max_bind_groups=4 유지) + `create_pbr_skinned_pipeline`; `BasicRenderer` skinned 패스가 `pbr_enabled` 시 PBR 스킨드 파이프라인 사용(멀티라이트 Cook-Torrance + IBL + 그림자). 스킨드 엔티티도 `MaterialHandle`/`TextureHandle`/`PbrTextures` 반영(정적 메시와 동일, per-entity 머티리얼 + 4-맵). 캡처 회귀 `runtime_skinned_pbr_path_renders_and_animates` + `docs/visual/skinned_pbr.png`.

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

- ~~**비동기 로딩**~~ ✅ AssetLoader (워커 스레드) 완료. **v1.0.55 버그 fix**: `load()` 가 반환하는 handle 은 전역 load 카운터 기반 *ticket* 인데 `Assets::insert` 의 per-type 슬롯과 달라, real≠ticket (다른 타입 로드 / 기존 에셋 존재) 시 `process_loaded` 의 remap 이 ticket 키를 지워 `state(ticket)` 이 Loading 오보 + caller 의 handle 이 엉뚱한 슬롯을 가리킴 (fresh 단일-로드에서만 우연히 동작). remap 제거 (handle_to_load 를 안정적 ticket 키로 유지) + `resolved_handle(ticket)` 추가 — Ready 후 실제 `Handle<T>` 반환. +1 회귀 테스트.
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
- ~~**ECS 통합**~~ ✅ AudioSource 컴포넌트 완료. ~~audio_sync_system 미구현~~ ✅ 2026-06-03 — `voltex_runtime::audio_sync_system` 가 AudioSource→AudioSystem 브릿지 (playing 엣지에서 play/play_3d/stop, listener Transform 동기화). 한계: SpatialParams min/max 고정. ~~씬 직렬화 미등록 — v1.1.~~ ✅ 2026-06-03 (v1.0.61) — `voltex_runtime::audio_sync::register_audio_source` 가 `register_raw` escape hatch 로 AudioSource 직렬화 등록 (clip_id+volume+spatial/looping/playing 11바이트; transient played_once/active 는 미저장). RuntimeApp 이 `register_defaults()` 직후 자동 등록. JSON/binary 씬 round-trip.
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
- ~~**String Pulling (Funnel)**~~ ✅ SSF 알고리즘 완료. **apex-restart 버그 fix (확인됨, 수정 완료)**: apex 갱신 시 현 구현이 현재 인덱스에서 `continue` 만 해 정석 SSF (Mononen) 의 `i = apex_idx` 재시작을 누락 → 지그재그 corridor 에서 내부 코너 절단. randomized corridor + max-penetration oracle 로 재현 확인 (3-band 지그재그에서 경로가 corridor 밖 ~0.41 단위 이탈). `funnel_smooth` 를 `for` → `while` 루프로 바꾸고 apex 갱신 시 `i = apex_idx` 재시작으로 fix (공개 시그니처 불변, `find_path` 영향 없음). 회귀 테스트 `test_funnel_zigzag_stays_inside_corridor` 추가 (max-penetration < 1e-3 검증, fix 전 코드에서 fail 확인). 기존 funnel/path 테스트도 정확한 삼각형 시퀀스·waypoint·corner 검증으로 강화 (endpoint/len 만 검증하던 한계 해소).
- ~~**동적 장애물 회피**~~ ✅ velocity obstacle 기반 완료.
- ~~**ECS 통합**~~ ✅ NavAgent 컴포넌트 완료. ~~nav_agent_system 미구현~~ ✅ 2026-06-03 — `voltex_ai::nav_agent_system(world, navmesh, dt)` 가 find_path + follow_path 로 NavAgent 엔티티 이동 (kinematic, reached 처리). NavMesh 호출자 주입.
- ~~**내비메시 직렬화**~~ ✅ VNAV 바이너리 포맷 serialize/deserialize 완료.

## Phase 8-2

- ~~**상태 동기화 (스냅샷)**~~ ✅ Snapshot + delta 압축 완료. **v1.0.55 버그 fix**: `diff_snapshots`/`apply_diff` 가 삭제된 엔티티(old 엔 있고 new 엔 없는)를 전파하지 않아 클라이언트에 despawn 된 엔티티가 영구 잔존하던 버그 — 0x40 "removed" 플래그 추가로 fix. +2 회귀 테스트.
- ~~**보간 / 예측**~~ ✅ InterpolationBuffer 완료.
- ~~**지연 보상**~~ ✅ LagCompensation (history rewind + interpolation) 완료.
- ~~**신뢰성 계층**~~ ✅ ReliableChannel + OrderedChannel 완료. **v1.0.55 버그 fix**: 수신 dedup 이 `HashSet<u16>` 에 모든 시퀀스를 영구 보관 → u16 시퀀스 wrap(~65536패킷, 60/s 면 ~18분) 후 새 패킷이 옛 seq 를 재사용하면 "중복"으로 거부되어 데이터 손실 + OrderedChannel 영구 stall. `DEDUP_WINDOW=1024` sliding-window (slot = seq % WINDOW) 로 교체 — window 밖 시퀀스는 새 패킷 취급해 wrap 안전. 한계: 1024 시퀀스 넘게 지연된 재전송 중복은 재전달될 수 있음 (재전송은 2×RTT 내 도착하므로 비현실적). +3 회귀 테스트.
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

- ~~**런타임 그림자 없음**~~ ✅ 2026-06-03 — BasicRenderer directional shadow mapping (`RuntimeConfig::with_shadows(true)`): shadow depth pre-pass + mesh_colored 3x3 PCF (group 3). 기본 off. 한계: ortho 고정(30), 바닥 접촉 오브젝트 self-occlusion. 캡처 검증 docs/visual/shadow_on.png. ~~스킨드 메시 caster 아님~~ ✅ 2026-06-03 (v1.0.61) — `create_skinned_shadow_pipeline` + `skinned_shadow_shader.wgsl`(depth-only GPU 스키닝, 기존 skinned camera/palette 레이아웃 재사용, light-space cam UBO만 추가). BasicRenderer shadow pre-pass가 정적 메시 후 스킨드 메시도 깊이에 렌더. 캡처 회귀 `skinned_mesh_casts_shadow`(strip이 바닥에 그림자 투사, Hidden 대조군 대비 압도) + docs/visual/skinned_shadow.png.

- ~~**per-entity dynamic UBO**~~ ✅ GPU 인스턴싱 (InstanceData, InstanceBuffer, instanced pipeline) 완료.
- **max_bind_groups=4** — 리소스 합치기로 해결 중.
- ~~**BasicRenderer 가 머티리얼 색상 무시**~~ ✅ 2026-06-03 — 런타임 Blinn-Phong 경로(BasicRenderer)가 `MaterialUniform.base_color` 를 무시하고 흰색 기본 텍스처만 써 모든 오브젝트가 같은 회색 → 같은 색 바닥에 묻힘("박스 안 보임" 2차 원인). `ColoredCameraUniform`(per-entity base_color) + `mesh_colored.wgsl` + `create_colored_mesh_pipeline` 로 BasicRenderer 만 격리 수정 (shared CameraUniform/mesh_shader/PBR/4개 레거시 예제 무영향). voltex_capture `basic_renderer_applies_material_base_color` 회귀. 한계: 텍스처/노멀맵 등 풀 PBR 머티리얼은 여전히 PBR 파이프라인 전용 — 런타임 PBR 통합은 후속.

## Phase 8-7 (Picking + Editor 후속)

- ~~**viewport raycast picking**~~ ✅ sphere 기반 (`picking::pick_entity`). 좌클릭 → `session.selected`.
- ~~**Lua hierarchy API**~~ ✅ add_child / remove_child / find_all_by_tag 추가.
- ~~**editor_demo 시드의 command 경로 통합**~~ ✅ Save/Load round-trip 일관성 확보.
- ~~**메시 AABB picking**~~ ✅ Cycle 3 (E1) — `Bounds` 컴포넌트 있는 entity 는 8-corner AABB world transform 후 ray vs AABB. 없으면 sphere fallback.
- ~~**gizmo (translate/rotate/scale)**~~ ✅ Cycle 3 (E1) — translate/rotate/scale 3축 핸들 + LINE wgpu 파이프라인. End-drag 시 SetTransform 단일 undo 엔트리.
- ~~**Despawn 정확한 undo**~~ ✅ Cycle 12 (E3) — `EntityAllocator::restore_at_generation` + 등록된 모든 컴포넌트 snapshot/restore. 슬롯 재사용 시 silent skip.

## v1.0 / v1.0.1 / v1.0.2 / v1.0.3 / v1.0.4 / v1.0.5 / v1.0.6 / v1.0.7 / v1.0.8 추가 한계

- **3 OS exe artifact 검증** — Windows host 빌드 + runtime 검증 완료. v1.0.55 에서 **CLI 빌드 파이프라인 end-to-end 풀 실행 검증**: `voltex new` → `voltex build` (release, ~30s) → 산출 `dist/<name>.exe` (+ scene/assets/scripts + LICENSE×3 번들) 가 실제 RTX 4080 에서 panic/validation 0 으로 게임 구동. 즉 "게임 빌드 → 단일 exe 배포" DoD 헤드라인 워크플로가 실 하드웨어에서 작동 확인. Linux + macOS 실 머신 검증은 v1.1 사용자 PR.
- ~~**Lua input API**~~ ✅ 2026-06-03 (v1.0.59) — `voltex_script::engine_input` 가 Lua 글로벌 `input` 테이블 노출 (`is_key_pressed`/`is_key_just_pressed`/`mouse_position`/`mouse_delta`). 런타임이 매 프레임 `InputState` → 문자열-키 `ScriptInput` 스냅샷으로 브릿지 (`set_current_input`, thread-local 포인터, `__voltex_world` 와 동일 규약).
- **voltex_cli editor 모드** — `voltex new/build/run` 만. `voltex editor` 서브커맨드 v1.1.
- ~~**HUD 통합**~~ ✅ 2026-06-03 — `RuntimeApp::set_hud_fn` + `RuntimeConfig::with_hud` (v1.0.59).
- ~~**런타임 카메라 고정**~~ ✅ 2026-06-03 — `RuntimeConfig::with_camera(pos,yaw,pitch)`/`with_camera_fov` + `RuntimeApp::set_camera_fn(follow/orbit)`. 기존 하드코딩 `(0,10,20)` 가 기본값으로 유지되지만 이제 게임이 프레이밍 가능. (발견 계기: game_ready_demo에서 1-unit 큐브가 먼 기본 카메라 때문에 ~15px 점으로만 보여 "박스 안 보임" 리포트. 렌더는 정상이었음.)
- ~~**충돌/트리거 이벤트 런타임 노출**~~ ✅ 2026-06-03 — `RuntimeApp::set_collision_fn(\|contacts, &mut world, dt\|)`. 매 프레임 `detect_collisions` 결과(entity_a/entity_b 페어) 전달. `enable_physics` 무관 실행(트리거 존). game_ready_demo의 "잡힘"을 거리 계산 → 실제 Collider 충돌 이벤트로 교체. +1 테스트.
- ~~**런타임 조명 고정**~~ ✅ 2026-06-03 — `RuntimeConfig::with_light(dir,color,ambient)` + `RuntimeApp::set_light_fn(\|light,world,dt\|)` (day/night·flicker). 기본값은 기존 `LightUniform::new()` 와 동일(역호환). 단일 directional light — 런타임 멀티 라이트(PBR 통합)는 후속.
- **1573 missing_docs** — facade prelude + tier-1 API 만 보장. crate 별 점진 작성 v1.1. v1.0.19 부터 시작: voltex_math 59→0, v1.0.20 voltex_platform 38→0. 나머지 11 crate (~1476): asset / net / ai / physics / audio / ecs / script / renderer_ecs / editor / runtime / renderer.
- **`pub(crate)` 다운그레이드** — 후보 노트 (`docs/v1.0-pub-audit.md`) 만 작성. 실 변경 v1.1.
- **600줄 한도 위반 4건** ✅ v1.0.2 에서 재분리.
- **macOS/Linux 백엔드 runtime** — Cycle 10 (P1) ALSA + Cycle 11 (B3) CoreAudio 코드 작성 완료. 실 머신 audio 출력 v1.1.
- **`voltex build` Windows P0** ✅ v1.0.3 fix (canonicalize \\?\ verbatim prefix strip).
- **clippy approx_constant 9 errors** ✅ v1.0.4 fix.
- **clippy warnings 84 → 0** ✅ v1.0.5 (43 auto-fix + 24 manual + 2 dead code 삭제).
- **`cargo audit` 0 vulnerabilities** ✅ v1.0.5 검증. 1 advisory (paste 1.0.15 unmaintained, transitive via wgpu/metal) — 통제 불가, 모니터링만.
- **CI 워크플로 (.github/workflows/ci.yml)** ✅ v1.0.5 추가. 단, **실제 GitHub runner 첫 green run 은 mirror 필요** — git.tolelom.xyz 가 GitHub Actions 실행 환경 미보유 시 inert. self-hosted gitea/forgejo runner 또는 GitHub mirror push 가 v1.1 후속.
- **`cargo audit` CI 통합** — cargo-audit binary install step 추가 필요. v1.1 후속.
- ~~**GUI 시각 검증**~~ 🟡 부분 완료 v1.0.55 — `voltex_capture` 헤드리스 오프스크린 render-to-PNG 하니스로 **6개 경로**가 실제로 올바르게 렌더됨을 PNG 로 확인 (`docs/visual/`): BasicRenderer(Blinn-Phong) / PBR(Cook-Torrance) / 2-패스 directional shadow / multi-light(point·spot·directional, 거리감쇠) / gizmo(translate·rotate·scale) / editor UI(IMGUI+폰트). 이 과정에서 발견한 에디터 툴바 세로 적층 버그 fix (`UiContext::same_line`). **deferred + IBL + shadow demo** 는 실제 RTX 4080(Vulkan)에서 6초 run — RT 기능획득+BLAS+9-entry bind group 포함 panic/validation 0 (offscreen 픽셀 캡처는 9-entry bind group 의 미초기화 SSGI/RT 입력 때문에 v1.1; deferred BRDF 는 forward PBR 로 이미 픽셀 검증). **남은 미검증**: 게임 런타임 화면, viewport/gizmo 실제 마우스 드래그 인터랙션.
- **coverage 측정** ✅ v1.0.6 — baseline 82.06% functions / 82.74% lines (cargo llvm-cov --workspace --lib). 80% 목표 평균 달성.
- **CLI 친화 메시지 (engine-path)** ✅ v1.0.6 — 1-line → 7-line help block (bash + PowerShell 양쪽 안내).
- **GPU/winit 의존 4 crate (renderer 66 / runtime 61 / renderer_ecs 54 / platform 46%) 의 coverage 보완** — headless GPU adapter + event-loop stub 필요. v1.1+.
- **`--fail-under-lines 80` CI gate** — 4 low-coverage crate 통합 테스트 도입 후 활성. v1.1+.
- **HTML coverage 리포트 (codecov/coveralls upload)** — CI 가 실제 runner 에서 동작한 후. v1.1+.
- **외부 의존 라이선스 고지** ✅ v1.0.7 — THIRD-PARTY-LICENSES.md (4,795 라인) + about.toml/hbs + CI license-check job. 284 transitive 모두 permissive, 0 copyleft.
- **license-check advisory → blocking** — CI 첫 green run 후 승격. v1.1+.
- ~~**THIRD-PARTY-LICENSES.md 를 `voltex build` artifact 에 동봉**~~ ✅ v1.0.12 — 아래 "LICENSE-MIT/APACHE 파일 추가 + `voltex build` 라이선스 번들" 항목에서 함께 해소. `voltex_cli::build::copy_license_files` 가 엔진 루트의 THIRD-PARTY-LICENSES.md + LICENSE-MIT + LICENSE-APACHE 를 `dist/` 로 복사 (crates/voltex_cli/src/build.rs, +4 tests).
- **`cargo-deny` 통합** — license + advisory + duplicate 통합 스캔. 현재 cargo-audit + cargo-about 으로 대체 가능. v1.1+.
- **parser panic-resistance fuzz** ✅ v1.0.8 — 21 신규 test (PNG/JPG/WAV/GLTF/OBJ), panic 0건. 외부 dep 없는 inline xorshift32.
- **OGG/Vorbis lib.rs 등록** ✅ v1.0.9 — `pub mod ogg; pub mod vorbis;` 등록 + clippy 27→0 (자동 5 + #![allow] 6 파일 + 진짜 fix 4) + `decode_ogg_vorbis_bytes` 진입점. 숨겨진 테스트 회귀 2건 (ogg invalid_capture_pattern, vorbis bitreader_basics) fix. voltex_audio 89→119 tests.
- **coverage-guided fuzzing (cargo-fuzz / honggfuzz)** — 현재 deterministic xorshift32 만. nightly toolchain + libfuzzer 인프라는 v1.1+.
- **실제 `.ogg` 파일 디코드 검증** — v1.0.9 는 unit test (≤100 byte 합성 입력) 만. Vorbis decoder 자체가 simplified (scalar codebook only, no coupled channels). 실 파일 + audio_demo 통합은 v1.0.x+.
- **crash-log panic hook** ✅ v1.0.10 — `voltex_runtime::crash` 모듈 + `RuntimeConfig::{enable_crash_log, crash_log_dir}` + 자동 `<app>-crash-UTC.txt` 작성. 외부 dep 0 (Howard Hinnant 알고리즘 inline). +14 tests. 한계: live panic 통합 테스트 미실시 (global hook state 격리 까다로움), 크래시 리포트에 GPU 어댑터/FPS/마지막 씬 정보 없음 — v1.1+.
- **voltex.toml strict schema + path validation** ✅ v1.0.11 — UnknownField/UnknownSection/ManifestPathMissing 에러 + Levenshtein ≤3 suggestion + `validate_paths` + `voltex build --no-validate-paths`. +13 tests. 한계: ManifestParse line-only (column 미지원), Levenshtein 동률시 알파벳 순 — v1.1+.
- **LICENSE-MIT/APACHE 파일 추가 + `voltex build` 라이선스 번들** ✅ v1.0.12 — repo 의 dual license 약속을 실 파일로 backing + 모든 `voltex build` 산출물의 `dist/` 에 THIRD-PARTY-LICENSES.md + LICENSE-MIT + LICENSE-APACHE 자동 복사. +4 tests. 한계: THIRD-PARTY-LICENSES.md 자동 재생성 (현재 수동 cargo about) — v1.1+.
- **voltex CLI `--version` + per-subcommand `--help`** ✅ v1.0.13 — help.rs 모듈 (TOP/NEW/BUILD/RUN), top-level `-V`/`--version`, `voltex <cmd> --help` 동작. +7 gating tests. ~~한계: --version 이 Cargo.toml package version (0.1.0) 출력 — workspace package version 일괄 bump 는 별 cycle.~~ ✅ 아래 항목에서 해소.
- ~~**워크스페이스 crate Cargo.toml `version` v1.0.x 일괄 sync**~~ ✅ v1.0.61 (9cdfaac) — `[workspace.package] version` 도입 + 17개 라이브러리 crate 전부 `version.workspace = true` 로 상속 (기존 `license.workspace = true` 와 동일한 opt-in 방식). `voltex --version` 이 릴리스 버전을 출력 — v1.0.110 에서 `voltex 1.0.110` 실행 확인. crate 간 의존은 전부 path/workspace 선언이라 박힌 version 없음. examples 17 + games 2 는 **의도적으로 0.1.0 유지** (엔진 릴리스 라인이 아님), `fuzz/` 는 워크스페이스 비멤버(nightly 전용)로 0.0.0.
- **CONTRIBUTING.md + git pre-commit hook** ✅ v1.0.14 — 4 파일 (.githooks/pre-commit, install.sh, install.ps1, CONTRIBUTING.md). bash syntax 검증. 한계: install.ps1 실 실행 검증 미실시 (개발 호스트 pwsh 부재), pre-commit fast (fmt) / pre-push slow (clippy+test) 분리 — v1.1+.
- **NaN-safe sort comparators (6 callsites)** ✅ v1.0.40 — physics BVH/raycast/mesh_collider, editor rasterizer, renderer light_probes 의 `partial_cmp(...).unwrap()` 을 `unwrap_or(Ordering::Equal)` 로 교체. +5 회귀 테스트.
- **Image parser size hardening (BMP/PNG/JPG)** ✅ v1.0.40 — zero-dim 거부 + 16384 max-dim cap + `checked_mul` 통일. +5 회귀 테스트. 한계: fuzz-driven coverage 는 v1.1+.
- **Vec/Ray/Mat4 normalize 문서 거짓 + safe variant** ✅ v1.0.41 — `normalize_or_zero` (Vec2/Vec3), `Ray::try_new`. 기존 docstring 의 panic 주장 제거. ~~한계: 기존 30+ `.normalize()` 호출자 일괄 audit/교체는 v1.1+.~~ 🟡 v1.0.61 audit 완료 — 게임플레이 핵심 경로(`voltex_ai` steering seek/flee/arrive/wander + nav_agent_system)는 **이미 전부 length/length_squared 가드**가 있어 NaN-safe 확인. 유저 라이트 설정이 zero direction 일 때 NaN 이 전파되던 `csm::compute_cascade_matrices`(`-light_dir`)와 `spot_shadow_matrix`(`direction`) 두 곳을 `normalize_or_zero` + straight-down fallback 으로 수정 (+2 회귀 테스트, `directional_light_vp` 와 일관). 잔여 미가드 호출자는 모두 degenerate-geometry/coincident-camera 엣지케이스(physics 삼각형 normal, editor gizmo cam_dir, orbit_camera) — 입력 유효성 도메인이라 호출자 가드 유지, v1.1+ 검토. math 프리미티브(`Ray::new`/`look_at`)는 문서화된 의도적 NaN(안전 변형 제공).
- **Snapshot deserialiser 적대적 count 거부** ✅ v1.0.41 — `MAX_SNAPSHOT_ENTITIES = 1_000_000` + `checked_mul`/`checked_add`. +2 회귀 테스트.
- **Hierarchy cycle / self-parent / stale reparent** ✅ v1.0.42 — `add_child` 가 self-parent, cycle 생성, re-parent stale entry 3종을 silently 허용. 모두 silently 거부 + 옛 parent 의 Children 정리. +5 회귀 테스트. ~~한계: 직렬화된 씬 파일의 pre-existing 사이클은 `validate_hierarchy` 별 pass 필요 — v1.1+.~~ ✅ `voltex_ecs::validate_hierarchy(world)` 구현 — Parent 엣지를 순회해 사이클/과심도 체인을 끊고 repaired 엔티티 목록 반환. text/binary/JSON 3개 deserializer 모두 로드 후 호출 (raw `world.add(Parent)` 우회 경로 방어).
- **Scene deserializer 적대적 entity count / index** ✅ v1.0.43 — `.vscn` text + `VSCN` binary 양쪽에 `MAX_SCENE_ENTITIES = 1_000_000` cap. Out-of-range parent index 도 silently 무시. +5 회귀 테스트. ~~JSON deserializer 의 추가 cap 은 v1.1+.~~ ✅ `deserialize_scene_json` 도 동일 `MAX_SCENE_ENTITIES` cap 적용 (`deserialize_scene_json_with_cap` 내부 경로 + 경계 테스트 2개).
- **Navmesh deserializer 적대적 입력** ✅ v1.0.44 — VNAV 두 진입점에 `MAX_NAVMESH_VERTICES/TRIANGLES = 1_000_000` cap + triangle vertex index / neighbour index 범위 검사. +3 회귀 테스트.
- **glTF accessor count + attribute mismatch** ✅ v1.0.45 — 7개 read_accessor_* 가 공통 `validate_count` (MAX=4M) + POSITION/NORMAL/UV/TANGENT 길이 일치 강제. +2 회귀 테스트.
- **Editor redo-Spawn entity leak** ✅ v1.0.46 — `Spawn→Undo→Redo→Undo` 시 두 번째 undo 가 stale entity 를 가리켜 새로 생성된 entity 가 영구 생존하던 버그. redo 후 inverse 패치. +1 회귀.
- **Audio mixer clip_index OOB** ✅ v1.0.46 — `clips.get()` 으로 안전 접근. +1 회귀.
- **Lua FFI null-deref + type confusion** ✅ v1.0.47 — `get_world` 가 Option 반환, 비숫자 handle/좌표 arg silent no-op 으로 거부. +3 회귀 테스트.
- **Animation zero-duration / extract_keyframe OOB / quat slerp NaN** ✅ v1.0.48 — `AnimationPlayer::update` duration=0 early-out, `extract_keyframe` start>=values.len() 기본값 반환, `Quat::slerp` dot clamp(-1,1). +3 회귀 테스트.
- **propagate_transforms cycle stack-overflow** ✅ v1.0.49 — raw `world.add(Children/Parent)` 가 add_child 가드를 우회한 cycle 에 대비 `MAX_HIERARCHY_DEPTH = 1024` depth-limit 추가. +2 회귀 테스트.
- **PacketCipher u64::MAX seq wrap + AuthToken NaN-time bypass** ✅ v1.0.50 — `saturating_add` + `is_finite()` 가드. +2 회귀 테스트.
- **Input first-mouse-move 카메라 점프** ✅ v1.0.51 — `mouse_seen: bool` 도입, 첫 이벤트는 warp 취급. +2 회귀 테스트.
- **GameTimer hz=0 panic + alpha div-by-zero** ✅ v1.0.52 — `clamp(1, 100_000)` + alpha 가드. +3 회귀 테스트.
- **Camera::right NaN at pitch ±90°** ✅ v1.0.53 — `normalize_or_zero` + Vec3::X fallback. +2 회귀 테스트.
- **Sphere generator degenerate-input** ✅ v1.0.54 — sectors/stacks/radius clamp. +2 회귀 테스트.
