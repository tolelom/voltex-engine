# Voltex Leap — 2D 플랫포머 도그푸딩 게임 (2편) 설계

날짜: 2026-08-09
상태: 승인 대기 없이 자율 세션 진행 (사용자 위임: "플랜 세워서 개발, 멈추라 할 때까지")

## 목적

엔진의 2D 스택(v1.0.60에서 추가: orthographic camera, `Sprite`, `SpriteAnimation`,
`set_sheet_frame`, `Particle`, `Lifetime`, `add_texture_png`)은 capture 회귀로만
검증됐고 **실게임으로 도그푸딩된 적이 없다**. Depths(3D 탑다운 슈터, 100% Rust)와
겹치지 않는 장르로 2D 워크플로 전반을 실전 투입해 **새 마찰 로그
(`docs/dogfood-friction-2.md`)** 를 시작한다.

도그푸딩 원칙은 1편과 동일: 게임 로직 100% Rust, 엔진 즉석 수정은 진행을 실제로
막을 때만, 마찰은 로그에 기록하고 다음 엔진 사이클 입력으로 쓴다.

## 장르 선택 (접근안 비교)

| 안 | 커버리지 | 판정 |
|---|---|---|
| **A. 사이드뷰 플랫포머 (채택)** | 중력/점프/2D AABB 충돌 응답, 카메라 팔로우, 애니메이션 상태 전환(idle/run/jump), ASCII 타일맵, 파티클, 스프라이트 시트 | Depths와 안 겹치는 이동/충돌 모델 — 새 마찰 기대치 최대 |
| B. 세로 스크롤 슈팅 | 스프라이트 물량, 단순 충돌 | 이동/충돌이 Depths와 유사(자유 이동 + 원거리) — 커버리지 좁음 |
| C. 브레이크아웃 | 최소 | 마찰 산출량 부족 |

## 게임 개요 — `games/voltex_leap`

한 개의 짧은 사이드뷰 레벨. 문에 도달하면 클리어 (코인은 선택 수집 — HUD 점수만,
클리어 조건 아님). 가시/적에 닿으면 사망 → R로 재시작.

- **뷰**: XY 평면이 게임 평면 (X 수평, Y 수직, 모든 엔티티 z=0). 직교 카메라
  (`with_orthographic`)가 +Z에서 -Z를 봄. 카메라는 플레이어 X를 따라감
  (`set_camera_fn`), Y는 레벨 중앙 고정(레벨이 낮아 수직 스크롤 불요).
- **레벨 저작**: ASCII 맵 문자열 → 타일 그리드 (Depths의 `level.rs` 패턴 재사용).
  문자: `#` 지형 블록, `^` 가시, `o` 코인, `E` 순찰 적, `P` 플레이어 스폰, `D` 문.
- **플레이어**: 좌우 이동 + 점프. 중력 상수 적분, 타일 그리드 vs AABB 축 분리
  충돌 해소(수평 → 수직 순, Depths grid-slide의 2D 변형). 바닥 접지 판정으로
  점프 허용. 코요테 타임/더블 점프 없음 (YAGNI).
- **적**: 걷는 순찰자 1종. 벽 또는 플랫폼 가장자리에서 방향 전환. 접촉 시 사망.
- **하자드**: 가시 타일 접촉 시 사망.
- **수집**: 코인 — `SpriteAnimation` 회전 시트. 전부 모을 필요는 없음(HUD 카운트만).
- **문**: 도달 시 클리어. 상태: Title → Playing → Dead/Clear → (R) Playing.
- **파티클**: 점프/착지 먼지, 코인 획득 스파클 (`spawn_particle` + `Lifetime`).
- **오디오**: 절차 생성 SFX(점프/코인/사망/클리어) + BGM — Depths `gen_sfx` 패턴
  재사용 (`bin/gen_assets.rs`), `with_audio_files` 사용.
- **텍스처**: 절차 생성 PNG 스프라이트 시트 — 같은 `gen_assets` 바이너리가
  `voltex_image::encode_rgba8` 로 생성 (플레이어 시트: idle 2 + run 4 + jump 1,
  코인 시트 4프레임). `add_texture_png` 런타임 로딩 + `set_sheet_frame` 실전 검증.
  타일/적/문은 솔리드 컬러 `Sprite::solid` (에셋 제작이 목적이 아님).
- **HUD**: 런타임 HUD 오버레이로 코인 수 + 상태 텍스트 (Depths HUD 패턴).

## 비범위 (YAGNI)

멀티 레벨, 체크포인트, 메뉴 UI, 수직 스크롤, 슬로프, 사다리, 코요테 타임,
적 2종 이상, 외부 아트 에셋, Lua, 에디터 저작(씬 파일) — 이번 편은 Rust 코드
저작 경로에 집중. 에디터 저작 도그푸딩은 별도 편에서.

## 구조 (Depths 미러)

```
games/voltex_leap/
  Cargo.toml            (default-run = voltex_leap)
  src/lib.rs            (게임 로직 — 테스트 대상)
  src/main.rs           (RuntimeApp 부트스트랩 + 시스템 배선)
  src/level.rs          (ASCII 파싱, 그리드↔월드 좌표, 타일 쿼리)
  src/components.rs     (Player, Walker, Coin, Door, Spike 마커/상태)
  src/physics2d.rs      (중력 + AABB vs 타일 그리드 축 분리 해소, 접지 판정)
  src/game_state.rs     (Title/Playing/Dead/Clear 전이, 재시작)
  src/bin/gen_assets.rs (SFX wav + 스프라이트시트 PNG 절차 생성)
  assets/               (생성물 커밋)
  tests/playthrough.rs  (헤드리스 풀 플레이스루)
  tests/visual.rs       (GPU 시각 회귀 — render_world)
```

물리는 `voltex_physics`를 쓰지 않는다 — 3D 강체 솔버는 플랫포머 캐릭터
컨트롤러에 부적합(의도적 선택이며, "2D에 쓸 물리 헬퍼 부재"가 마찰인지 여부를
로그에 판정 기록). 자체 2D 이동/충돌은 ~150줄 예상.

## 검증

1. **단위**: 레벨 파싱, 좌표 변환, 충돌 해소(관통 불가/코너/접지), 걷기 AI 전환,
   상태 전이, 코인 획득 — TDD.
2. **헤드리스 플레이스루**: 실제 tick 순서를 미러한 시뮬레이션으로 스폰→이동→
   점프→코인→사망 경로와 클리어 경로 완주 (Depths `playthrough.rs` 패턴).
3. **GPU 시각 회귀**: `voltex_capture::render_world` 로 레벨 씬 픽셀 단언.
   render_world가 perspective 전제라면 orthographic 갭을 마찰 로그에 기록하고,
   perspective 샷으로라도 타일/스프라이트 색 비율 단언.
4. **워크스페이스**: build/clippy(-D warnings)/test 그린. `voltex build`
   kind="rust" dist 스모크는 여력 시(이미 Depths가 E2E 커버 — 중복 검증 회피).

## 성공 기준

- 게임이 `cargo run -p voltex_leap` 로 부팅·플레이 가능 (수동 확인은 사용자
  배치 검증으로 위임 — 헤드리스/GPU 검증까지가 이번 세션 책임).
- 위 검증 4종 그린.
- `docs/dogfood-friction-2.md` 신설 + 발견 마찰 전건 기록 (P0~P2 등급).
- STATUS/CHANGELOG 갱신, atomic commit 체인.
