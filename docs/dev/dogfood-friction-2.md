# 도그푸딩 마찰 로그 2 — voltex_leap (2D 스택)

게임 2편(2D 사이드뷰 플랫포머, `games/voltex_leap`) 제작 중 발견한 엔진 워크플로
마찰. 형식은 1편(`docs/dev/dogfood-friction.md`)과 동일: 하려던 것 / 마찰 / 우회 /
제안 / P0~P2. 엔진 즉석 수정은 진행을 실제로 막을 때만. 이 로그가 다음 엔진
사이클 입력이 된다.

## 현재 상태 요약

**2026-10-12: 전 항목 해소 (6/6).** 2026-08-09 게임 완성 시점 6건(P1 3 +
P2 3) 중 P1 3건은 같은 날, #4는 v1.0.113, #5·#6은 2026-10-12(v1.0.122~124)에
해소. 다음 도그푸딩은 새 로그로 시작한다.

**2026-08-09: 게임 완성 시점 6건 (P1 3 + P2 3) → 같은 날 후속 사이클에서 P1
3건 해소, P2 3건 잔여.** 진행을 막는 마찰은 없었다 — 전부 우회 가능했고,
우회 비용이 곧 P 등급이다. 해소 설계:
`docs/superpowers/specs/2026-08-09-sprite-authoring-friction-design.md`.

- ~~#1 SpriteAnimation 프레임 오프셋 부재~~ — **해소** (`SpriteAnimation::clip(cols, rows, first_frame, frames, fps)` + `first_frame` 필드. leap이 플레이어 단일 8×1 시트로 마이그레이션해 실검증)
- ~~#2 Sprite 좌우 반전 1급 지원 부재~~ — **해소** (`Sprite::flip_x` 표시 전용 플래그 — 렌더러가 `effective_uv_rect()`로 u-스왑 적용, `uv_rect`는 애니 시스템이 정방향 소유. 매 프레임 재적용/tick 순서 의존 소멸)
- ~~#3 render_world 직교 미지원~~ — **해소** (`voltex_capture::render_world_ortho(..., ortho_height)` — leap visual 회귀가 게임과 동일한 직교 구도로 전환)
- ~~#4 2D 타일 충돌 헬퍼 부재~~ — **해소** (v1.0.113 `voltex_physics::grid2d::move_and_collide_2d` — `is_solid(gx,gy)` 클로저 그리드, 경계 정책 호출자 소유. 캐릭터 튜닝 상수는 게임 잔류 판정. leap physics2d는 어댑터로 축소, 기존 테스트 4개가 동작 동일성 가드)
- ~~#5 타일맵 배치 렌더 부재~~ — **해소** (v1.0.122 `voltex_renderer_ecs::Tilemap` + 맵당 draw 1회 배치 패스, v1.0.123 leap 마이그레이션 — 렌더 결과 바이트 동일)
- ~~#6 빌보드 z-rot 불가~~ — **해소** (v1.0.124 `Sprite::roll_from_transform` 옵트인 — Transform z 회전을 화면 평면 회전으로 반영, 기본은 기존 빌보드)

참고 — 마찰이 *아니었던* 것 (지난 사이클 개선의 실사용 검증):
`with_gpu_init`(텍스처 업로드 훅), `with_asset_root`, `with_audio_files`
(+ AudioClipCache 시드), orphan voice 자동 stop, `Sprite`/`Particle`/`Lifetime`
빌트인 tick, `render_world` 공개 진입점 — 모두 문서대로 동작했고 우회가
필요 없었다.

| # | 하려던 것 | 마찰 | 우회 | 제안 | P |
|---|---|---|---|---|---|
| 1 | 한 스프라이트 시트에 idle/run/jump를 담고 상태별 재생 | `SpriteAnimation`이 시트의 `0..frame_count` 프리픽스만 재생 — 시작 프레임 오프셋/프레임 범위/행 선택이 없어 상태 전환이 불가능 | 상태별로 텍스처를 분리 생성(idle/run/jump 3장) + 상태 변화 시 `Sprite.texture`와 `SpriteAnimation`을 통째로 교체 | `SpriteAnimation`에 `first_frame`(또는 `frame_range`) 추가 — 한 아틀라스에 여러 클립을 담는 게 2D 표준 워크플로 | P1 |
| 2 | 좌우 이동 시 스프라이트 좌우 반전 | `Sprite`에 flip 개념이 없음. `uv_rect` u 성분 스왑은 셰이더가 `mix(uv_rect.xy, uv_rect.zw, uv)`라 동작하지만, `update_sprite_animations`가 매 프레임 `set_sheet_frame`으로 정방향 uv를 다시 써서 **덮어씀** — flip을 애니 갱신 *이후* 시스템에서 매 프레임 재적용해야 하고, 이는 runtime tick 내부 순서(엔진 애니 tick → user/action system) 지식에 의존 | action system(애니 갱신 뒤 실행)에서 facing<0이면 매 프레임 u 스왑 재적용 | `Sprite.flip_x: bool` 1급 필드 — `set_sheet_frame`/애니 시스템이 flip을 보존하며 uv 계산 | P1 |
| 3 | 게임의 직교 카메라 그대로 GPU 픽셀 회귀 | `voltex_capture::render_world`가 perspective FOV 45° 고정 계약 — 직교 2D 게임의 실제 화면을 재현 불가. (계약 doc에 "두 번째 게임이 필요로 하면 파라미터화"라고 명시돼 있었고, 그 시점이 왔다) | 원거리 perspective 샷으로 색 비율만 단언 (구도 검증 포기) | `render_world`에 ortho height(또는 `Camera` 전체) 파라미터 변형 추가 | P1 |
| 4 | 플랫포머 이동/충돌 (중력, 타일 AABB, 접지) | `voltex_physics`는 3D 강체 솔버 전용 — 캐릭터 컨트롤러에 부적합(의도적 미사용). 타일 그리드 vs AABB 축 분리 해소 같은 2D 공통 로직의 엔진 헬퍼가 없음 | 자체 구현 `physics2d.rs` ~130줄 (스윕 없음, 프레임 이동량 < TILE 전제) | 판정 필요: 캐릭터 컨트롤러는 게임 영역이라는 입장도 성립. 다만 "타일 그리드 충돌 + 축 분리 해소"는 2D 게임마다 재작성될 것 — `voltex_physics::grid2d` 모듈 후보 | P2 |
| 5 | ASCII 맵 → 화면 (타일 렌더링) | 타일맵 개념이 없어 솔리드 셀당 `Sprite` 엔티티 1개 (이번 레벨 ~100개, 대형 맵이면 수천) — `BasicRenderer` per-entity dynamic UBO와 `max_entities` 상한에 정면 충돌하는 구조 | 레벨이 작아 그대로 진행 | 타일맵 배치 렌더(단일 드로우) 또는 정적 스프라이트 배칭 | P2 |
| 6 | (관찰) 회전하는 스프라이트 | `Sprite`는 항상 카메라를 향하는 빌보드 — Transform rotation이 무시되어 z축 회전(빙글 도는 코인 등) 표현 불가 | 코인 회전을 4프레임 시트 애니메이션으로 표현 (충분했음) | 직교/2D 모드에서 Transform z-rot 반영 옵션. 이번 편에선 비차단이라 관찰만 | P2 |

## 메모

- #1+#2는 묶어서 "2D 스프라이트 저작 1급 지원" 사이클 하나로 처리하는 게
  효율적 (같은 파일 `sprite.rs`, 같은 테스트 스위트).
- #4 구현 중 얻은 것: 축 분리 해소 순서(X→Y), flush 접촉 EPS 처리, 발밑
  프로브 접지 판정 — grid2d 모듈로 승격 시 `voltex_leap/src/physics2d.rs`가
  출발점이자 회귀 테스트 소스.
