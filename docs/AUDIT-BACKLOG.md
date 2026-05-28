# 심층 감사 후속 백로그 (v1.0.56 audit)

2026-05-28 아키텍처/테스트 품질 심층 감사에서 나온 미처리 발견 + 기존 DEFERRED 의 v1.1 항목.
각 항목은 발견 위치 / 작업량 / 리스크 표기. 처리 시 `[x]` 체크 + 커밋 해시 기록.

> 추천 진행 순서: 저리스크·고가치 **C(중복) → B(데드코드) → A2/A4(타입안전)** 한 묶음 →
> **D(테스트 갭)** → 신중히 **E1(voltex_image 분리)**.

## A. 타입 안전성 · 캡슐화

- [ ] **A1** `Entity` 가 `pub id/generation` 노출 — `Handle<T>` 는 캡슐화돼 비대칭. 임의 Entity 위조 가능. (`voltex_ecs/entity.rs`, 中/中)
- [ ] **A2** `AudioSource` 가 "ECS 컴포넌트" 문서화됐지만 ECS 의존 없음 + `clip_id: u32` primitive obsession → `ClipId` newtype + 문서 정정. (`voltex_audio/audio_source.rs`, 小/低)
- [ ] **A3** `MeshHandle`/`MaterialHandle` 의 `pub` inner `Handle` + `from_raw_parts` 로 lockstep 불변식 비강제. (`voltex_ecs/render_components.rs`, `renderer_ecs/assets.rs`, 小/中)
- [ ] **A4** Lua `(id,gen)` 마샬링이 17개 함수에 산재 → `entity_from_lua`/`push_entity` 헬퍼 일원화. (`voltex_script/engine_api.rs`, 中/低)

## B. 데드코드 정리 (YAGNI)

- [~] **B1** `DependencyScheduler`/`SystemAccess`/`build_schedule` — 소비자 0, 실행 불가. **보류**: CLAUDE.md 용어집에 "의존성 기반 병렬 = future work" 로 이미 명시 + missing_docs 통과(문서 있음) → churn 회피, 실 병렬 실행 구현 시 재방문.
- [x] **B2** `net::OrderedChannel` — **CODEMAP 문서화** (제거 X — in-order delivery 는 정당한 공개 네트워킹 primitive, 라이브러리 공개 API 는 내부 소비자 0 정상). (커밋 후속)
- [x] **B3** `CliError::SceneNotFound` 데드 variant 제거 + Display arm 제거. (커밋 후속)

## C. 중복 제거 (DRY)

- [ ] **C1** CRC32/Adler32/stored-deflate 가 renderer 와 capture 에 각각 구현. (`capture/png_encode.rs` ↔ `renderer/deflate.rs`)
- [x] **C2** `LoadState` vs `PendingState` 동일 상태공간 이중 정의 → `PendingState` 제거, `LoadState`(+Clone) 단일화. (커밋 후속)
- [x] **C3** `SparseSet::increment_tick` trait impl 이 inherent 메서드로 위임. (커밋 후속)
- [ ] **C4** LE-f32 직렬화 헬퍼가 ecs/net/audio 에 재발명. (3 crate)

## D. 남은 테스트 / 커버리지 갭

- [ ] **D1** `ssgi` `pseudo_random`/`generate_noise_data` 미테스트. (`renderer/ssgi.rs`, P2)
- [ ] **D2** `BlasTracker::mark_dirty` 미등록 id no-op, `BonePalette::set_bone` OOB drop 미테스트. (renderer, P3)
- [ ] **D3** `test_extrapolation_returns_latest` 가 사실 클램핑만 검증 — 이름 오해. (`net/interpolation.rs`, P3)

## E. 대형 리팩터링 (영향 큼, 신중)

- [ ] **E1** `voltex_image` crate 분리 — renderer 의 PNG/JPG/deflate/glTF/OBJ/json_parser 는 wgpu 무의존 → God-crate(~120 export) 해소, L0/L1 강등. (大)
- [ ] **E2** `query2/3/4` 가 `Vec` 반환(매 프레임 할당) → iterator화 + min-selection 로직 추출. (borrow 제약 검토)
- [ ] **E3** `Transform`/snapshot 회전이 Euler `Vec3` → quaternion 저장 검토. (광범위)

## F. 기존 v1.1 백로그 (DEFERRED 출처)

- [ ] **F1** missing_docs ~1476개 (11 crate) 점진 작성
- [ ] **F2** `validate_hierarchy` pass — 직렬화 씬의 pre-existing 사이클 방어
- [ ] **F3** JSON deserializer entity-count cap (text/binary 는 완료)
- [ ] **F4** coverage-guided fuzzing (cargo-fuzz/honggfuzz)
- [ ] **F5** GPU/winit 의존 4 crate headless 커버리지 + `--fail-under-lines 80` CI gate
- [ ] **F6** Linux/macOS 오디오 백엔드 실 머신 검증
- [ ] **F7** CI 첫 green run (GitHub mirror/self-hosted runner)
- [ ] **F8** `pub(crate)` 다운그레이드 audit 실행

## P3 — 보류 (저가치)

- `AuthToken::validate` 비상수시간 비교 (dev-only 디스클레이머 있음)
- `query_with`/`query2_with` 이중 패스 (E2 와 함께)
- `voltex_cli` 수기 `Display` vs thiserror (외부 dep 0 정책상 의도적)
