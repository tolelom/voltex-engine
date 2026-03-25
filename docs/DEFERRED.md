# 미뤄진/간소화된 구현 항목

## Phase 2

- ~~**PNG 디코더 자체 구현**~~ ✅ 완료.
- ~~**JPG 디코더 자체 구현**~~ ✅ Baseline JPEG 완료.
- ~~**glTF 파서**~~ ✅ glTF 2.0 / GLB 완료.
- **JPG Progressive** — Baseline만 지원. Progressive JPEG 미구현.
- **glTF 애니메이션/스킨** — 메시+머티리얼만 지원.

## Phase 3a

- **Archetype 기반 스토리지** → SparseSet 사용 중. 대규모 씬에서 성능 이슈 시 전환.
- ~~**시스템 스케줄러**~~ ✅ 순서 기반 Scheduler 완료.
- ~~**쿼리 필터**~~ ✅ With/Without 완료.
- ~~**query3+**~~ ✅ query3, query4 완료.
- **Changed 필터** — 컴포넌트 변경 감지 미구현.
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

- ~~**Capsule, Convex Hull 콜라이더**~~ ✅ Capsule + GJK/EPA 완료. **Convex Hull 미구현.**
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
- **Ray vs Plane, Mesh** — 삼각형 단위만 지원. 전체 메시 순회 레이캐스트 미구현.
- ~~**raycast_all (다중 hit)**~~ ✅ 거리순 정렬 완료.
- ~~**BVH 조기 종료 최적화**~~ ✅ 재귀 트리 순회 + query_ray 완료.

## Phase 6-1

- **macOS/Linux 백엔드** — WASAPI(Windows)만 구현.
- ~~**OGG/Vorbis 디코더**~~ ✅ OGG 컨테이너 + Vorbis 디코더 완료.
- ~~**24-bit/32-bit WAV**~~ ✅ 완료.
- **ECS 통합** — AudioSource 컴포넌트 미구현.
- **비동기 로딩** — 동기 로딩만.

## Phase 6-2

- ~~**도플러 효과**~~ ✅ doppler_shift 완료.
- **HRTF** — 미구현.
- **Reverb/Echo** — 미구현.
- **Occlusion** — 미구현.

## Phase 6-3

- **동적 그룹 생성** — 고정 4개만.
- **그룹 간 라우팅/버스** — 미구현.
- **이펙트 체인** — Reverb, EQ 등 미구현.
- **비선형 페이드 커브** — 선형 페이드만.

## Phase 7-1

- **투명 오브젝트** — 디퍼드에서 처리 불가. 별도 포워드 패스 필요.
- **G-Buffer 압축** — 미적용.
- **Light Volumes** — 풀스크린 라이팅만.
- **Stencil 최적화** — 미구현.

## Phase 7-2

- **Bilateral Blur** — SSGI 노이즈 제거 블러 미구현.
- **반해상도 렌더링** — 풀 해상도 SSGI.
- **Temporal Accumulation** — 미구현.
- **Light Probes** — 미구현.

## Phase 7-3

- **RT Reflections** — 미구현.
- **RT AO** — 미구현.
- **Point/Spot Light RT shadows** — Directional만 구현.
- **Soft RT shadows** — 단일 ray만.
- **BLAS 업데이트** — 정적 지오메트리만.
- **Fallback** — RT 미지원 GPU 폴백 미구현.

## Phase 7-4

- **TAA** — 미구현.
- **SSR** — 미구현.
- **Motion Blur, DOF** — 미구현.
- **Auto Exposure** — 고정 exposure만.
- **Bilateral Bloom Blur** — 단순 tent filter.

## Phase 8-1

- ~~**자동 내비메시 생성**~~ ✅ 복셀화 기반 NavMeshBuilder 완료.
- ~~**String Pulling (Funnel)**~~ ✅ SSF 알고리즘 완료.
- ~~**동적 장애물 회피**~~ ✅ velocity obstacle 기반 완료.
- **ECS 통합** — AI 컴포넌트 미구현.
- **내비메시 직렬화** — 미구현.

## Phase 8-2

- ~~**상태 동기화 (스냅샷)**~~ ✅ Snapshot + delta 압축 완료.
- ~~**보간 / 예측**~~ ✅ InterpolationBuffer 완료.
- **지연 보상** — 미구현.
- ~~**신뢰성 계층**~~ ✅ ReliableChannel + OrderedChannel 완료.
- **암호화 / 인증** — 미구현.

## Phase 8-3

- ~~**핫 리로드**~~ ✅ reload_file 완료.
- ~~**엔진 API 노출**~~ ✅ 기본 API (spawn, position, entity_count, velocity, rotation, scale, destroy, find_by_tag) 완료.
- ~~**Lua 테이블 ↔ Rust 구조체**~~ ✅ push_table/read_table/push_vec3/read_vec3 완료.
- ~~**코루틴**~~ ✅ LuaCoroutine (create/resume/is_finished) 완료.
- ~~**샌드박싱**~~ ✅ os/io/loadfile/dofile/require 차단 완료.

## Phase 8-4

- **씬 뷰포트** — 3D 렌더러 임베드 미구현.
- **엔티티 인스펙터** — ECS 컴포넌트 편집 미구현.
- **에셋 브라우저** — 파일 시스템 탐색 미구현.
- ~~**텍스트 입력**~~ ✅ text_input 위젯 완료.
- ~~**스크롤, 드래그앤드롭**~~ ✅ scroll_panel + drag/drop 완료.
- **도킹, 탭, 윈도우 드래그** — 미구현.
- **TTF 폰트** — 비트맵 고정폭만.

## 렌더링 한계

- **per-entity dynamic UBO** — 수천 개 이상은 인스턴싱 필요.
- **max_bind_groups=4** — 리소스 합치기로 해결 중.
