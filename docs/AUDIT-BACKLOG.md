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

- [~] **B1** `DependencyScheduler`/`SystemAccess`/`build_schedule` — 소비자 0, 실행 불가. **보류**: CLAUDE.md 용어집에 "의존성 기반 병렬 = future work" 로 이미 명시 + missing_docs 통과(문서 있음) → churn 회피, 실 병렬 실행 구현 시 재방문.
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
- [ ] **E2** `query2/3/4` 가 `Vec` 반환(매 프레임 할당) → iterator화 + min-selection 로직 추출. (borrow 제약 검토)
- [ ] **E3** `Transform`/snapshot 회전이 Euler `Vec3` → quaternion 저장 검토. (광범위)

## F. 기존 v1.1 백로그 (DEFERRED 출처)

- [ ] **F1** missing_docs ~1476개 (11 crate) 점진 작성
- [ ] **F2** `validate_hierarchy` pass — 직렬화 씬의 pre-existing 사이클 방어
- [x] **F3** JSON deserializer entity-count cap (text/binary 는 완료) — `deserialize_scene_json` 에 `MAX_SCENE_ENTITIES` cap 추가 + `_with_cap` 테스트 헬퍼 + boundary/reject 테스트 2개. 파서 자체 array-length cap 은 별도 (이 fix 는 deserializer 경계 방어). (이번 커밋)
- [ ] **F4** coverage-guided fuzzing (cargo-fuzz/honggfuzz)
- [ ] **F5** GPU/winit 의존 4 crate headless 커버리지 + `--fail-under-lines 80` CI gate
- [ ] **F6** Linux/macOS 오디오 백엔드 실 머신 검증
- [ ] **F7** CI 첫 green run (GitHub mirror/self-hosted runner)
- [ ] **F8** `pub(crate)` 다운그레이드 audit 실행
- [x] **F9** `voltex_image` 의 moved 디코더 파일들에 남아있는 `#![allow(missing_docs)]` 제거 — 7개 파일 doc 작성 + gltf/types.rs·parser.rs pub 항목 doc 추가. (이번 커밋)

## P3 — 보류 (저가치)

- `AuthToken::validate` 비상수시간 비교 (dev-only 디스클레이머 있음)
- `query_with`/`query2_with` 이중 패스 (E2 와 함께)
- `voltex_cli` 수기 `Display` vs thiserror (외부 dep 0 정책상 의도적)
