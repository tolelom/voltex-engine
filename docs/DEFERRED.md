# 미뤄진/간소화된 구현 항목

## Phase 2

- **PNG 디코더 자체 구현** — deflate + 필터링. 현재 BMP만 지원.
- **JPG 디코더 자체 구현** — Huffman + DCT. 현재 미구현.
- **glTF 파서** — OBJ만 지원 중.

## Phase 3a

- **Archetype 기반 스토리지** → SparseSet 사용 중. 대규모 씬에서 성능 이슈 시 전환.
- **시스템 스케줄러** — 의존성 기반 실행 순서/병렬 실행 미구현. 시스템은 함수 호출.
- **쿼리 필터** — With, Without, Changed 미구현. query/query2만 존재.
- **query3+** — query2까지만 있음.

## Phase 3b

- **JSON 직렬화** → 커스텀 .vscn 텍스트 포맷 사용.
- **바이너리 씬 포맷** — 미구현.
- **임의 컴포넌트 직렬화** — Transform/Parent/Tag만 지원.

## Phase 3c

- **비동기 로딩** — 동기 insert만.
- **핫 리로드** — 파일 변경 감지 미구현.

## Phase 4a

- **Metallic/Roughness/AO 텍스처 맵** → 파라미터 값만 사용. 텍스처 샘플링 미구현.
- **Emissive 맵** — 미구현.

## Phase 4b

- **CSM (Cascaded Shadow Maps)** → 단일 캐스케이드만. 원거리 그림자 해상도 낮음.
- **Point Light Shadow (큐브맵)** — 미구현.
- **Spot Light Shadow** — 미구현.
- **라이트 컬링** — 타일/클러스터 기반 미구현.

## Phase 4c

- **HDR 큐브맵 환경맵** → 프로시저럴 sky 함수로 대체.
- **Irradiance/Prefiltered Map 컨볼루션** → 프로시저럴 근사.
- **GPU 컴퓨트 BRDF LUT** → CPU 생성 (256x256).

## Phase 5-1

- **Capsule, Convex Hull 콜라이더** — Sphere + Box만 구현. 추후 GJK/EPA와 함께 추가.
- **OBB (회전된 박스) 충돌** — 축 정렬 AABB만 지원. OBB는 GJK/EPA로 대체 예정.
- **Incremental BVH 업데이트** — 매 프레임 전체 rebuild. 성능 이슈 시 incremental update 적용.
- **연속 충돌 감지 (CCD)** — 이산 충돌만. 빠른 물체의 터널링 미처리.

## Phase 5-2

- **각속도/회전 물리** — angular_velocity 필드만 존재, 적분 미구현. 관성 텐서 필요.
- **마찰 (Coulomb)** — 미구현. 물체가 미끄러짐 없이 반발만.
- **Sequential Impulse 솔버** — 단일 반복 충돌 응답만. 다중 물체 쌓기 불안정.
- **Sleep/Island 시스템** — 정지 물체 최적화 미구현.

## Phase 5-3

- **Ray vs Plane, Triangle, Mesh** — 콜라이더 기반만 지원. 메시 레벨 레이캐스트 미구현.
- **raycast_all (다중 hit)** — 가장 가까운 hit만 반환.
- **BVH 조기 종료 최적화** — 모든 리프 검사 후 최소 t 선택. front-to-back 순회 미구현.

## Phase 8-2

- **상태 동기화 (스냅샷)** — 미구현. 서버→클라이언트 월드 상태 전송.
- **보간 / 예측** — 미구현. 클라이언트 측 스무딩.
- **지연 보상** — 미구현. 서버 측 히트 판정 보정.
- **신뢰성 계층** — 미구현. 패킷 재전송, 순서 보장.
- **암호화 / 인증** — 미구현.

## Phase 8-1

- **자동 내비메시 생성** — Recast 스타일 복셀화 미구현. 수동 정의만.
- **String Pulling (Funnel)** — 삼각형 중심점 경로만. 최적 경로 스무딩 미구현.
- **동적 장애물 회피** — 정적 내비메시만. 런타임 장애물 미처리.
- **ECS 통합** — AI 컴포넌트 미구현. 함수 직접 호출.
- **내비메시 직렬화** — 미구현.

## Phase 7-4

- **TAA** — Temporal Anti-Aliasing 미구현. Motion vector 필요.
- **SSR** — Screen-Space Reflections 미구현.
- **Motion Blur, DOF** — 미구현.
- **Auto Exposure** — 고정 exposure만. 적응형 노출 미구현.
- **Bilateral Bloom Blur** — 단순 box/tent filter. Kawase blur 미적용.

## Phase 7-3

- **RT Reflections** — 미구현. BLAS/TLAS 인프라 재사용 가능.
- **RT AO** — 미구현.
- **Point/Spot Light RT shadows** — Directional만 구현.
- **Soft RT shadows** — 단일 ray만. Multi-ray soft shadow 미구현.
- **BLAS 업데이트** — 정적 지오메트리만. 동적 메시 변경 시 BLAS 재빌드 필요.
- **Fallback** — RT 미지원 GPU에서 자동 PCF 폴백 미구현.

## Phase 7-2

- **Bilateral Blur** — SSGI 노이즈 제거 블러 미구현. 4x4 노이즈 타일링만.
- **반해상도 렌더링** — 풀 해상도에서 SSGI 실행. 성능 최적화 미적용.
- **Temporal Accumulation** — 프레임 간 누적 미구현. 매 프레임 독립 계산.
- **Light Probes** — 베이크 기반 GI 미구현.

## Phase 7-1

- **투명 오브젝트** — 디퍼드에서 처리 불가. 별도 포워드 패스 필요.
- **G-Buffer 압축** — Position을 depth에서 복원, Normal을 octahedral 인코딩 등 미적용.
- **Light Volumes** — 풀스크린 라이팅만. 라이트별 sphere/cone 렌더 미구현.
- **Stencil 최적화** — 미구현.

## Phase 6-3

- **동적 그룹 생성** — 고정 4개(Master/Bgm/Sfx/Voice)만. 런타임 추가 불가.
- **그룹 간 라우팅/버스** — 미구현. 단순 Master → 개별 그룹 구조만.
- **이펙트 체인** — Reverb, EQ 등 미구현.
- **비선형 페이드 커브** — 선형 페이드만.

## Phase 6-2

- **도플러 효과** — 미구현. 상대 속도 기반 주파수 변조.
- **HRTF** — 미구현. 헤드폰용 3D 정위.
- **Reverb/Echo** — 미구현. 환경 반사음.
- **Occlusion** — 미구현. 벽 뒤 소리 차단.

## Phase 6-1

- **macOS/Linux 백엔드** — WASAPI(Windows)만 구현. CoreAudio, ALSA 미구현.
- **OGG/Vorbis 디코더** — WAV PCM 16-bit만 지원.
- **24-bit/32-bit WAV** — 16-bit만 파싱.
- **ECS 통합** — AudioSource 컴포넌트 미구현. AudioSystem 직접 호출.
- **비동기 로딩** — 동기 로딩만.

## 렌더링 한계

- **per-entity dynamic UBO** — 수천 개 이상은 인스턴싱 필요.
- **max_bind_groups=4** — IBL을 shadow group에 합쳐서 해결. 추가 group 필요 시 리소스 합치거나 bindless 활용.
