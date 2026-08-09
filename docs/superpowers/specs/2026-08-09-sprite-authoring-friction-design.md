# 2D 스프라이트 저작 1급 지원 — friction-2 P1 해소 설계

날짜: 2026-08-09
입력: `docs/dogfood-friction-2.md` #1, #2, #3 (P1 전건)
상태: 자율 세션 (leap 도그푸딩 직후 후속 사이클)

## 목적

voltex_leap 도그푸딩이 드러낸 P1 마찰 3건을 엔진에서 해소하고, **leap 자체를
새 API로 마이그레이션**해 실사용 검증까지 닫는다.

## 변경 1 — `SpriteAnimation.first_frame` (friction #1)

한 시트에 여러 클립(idle/run/jump)을 담을 수 있도록 재생 시작 오프셋 추가.

- `voltex_renderer_ecs::SpriteAnimation`에 `pub first_frame: u32` 추가
  (기본 0 — 기존 코드 무변경 호환. 구조체 리터럴 생성처는 워크스페이스 내
  전부 `new()` 경유 확인됨).
- 생성자: `new()`는 first_frame 0 유지, `clip(cols, rows, first_frame,
  frame_count, fps)` 신설.
- `update_sprite_animations`: 표시 프레임 = `first_frame + wrapped(raw)`.
  wrap/clamp은 기존과 동일하게 `frame_count` 기준 (클립 내부 순환).
- leap 마이그레이션: gen_assets가 플레이어 시트를 **한 장**(8x1: idle 0-1,
  run 2-5, jump 6, 여분 7)으로 생성, 상태 전환은 `SpriteAnimation` 교체만
  (텍스처 교체 제거).

## 변경 2 — `Sprite.flip_x` (friction #2)

- `voltex_renderer_ecs::Sprite`에 `pub flip_x: bool` 추가 (Default false).
- 적용 지점은 **렌더러** (`BasicRenderer`가 SpriteUniform을 채울 때 u 성분
  스왑) — 애니메이션 시스템/`set_sheet_frame`은 uv_rect를 계속 정방향으로
  소유하고, flip은 표시 속성으로 분리. 이로써 "매 프레임 재적용 + tick 순서
  의존" 문제가 소멸.
- leap 마이그레이션: action system의 uv 스왑 제거, `sprite.flip_x =
  player.facing < 0.0` 한 줄로.

## 변경 3 — `render_world_ortho` (friction #3)

- `voltex_capture::render_world_ortho(gpu, width, height, world, cam_pos,
  yaw, pitch, ortho_height)` 신설 — 기존 `render_world`와 동일 계약에서
  projection만 직교 (`Camera`의 orthographic 경로, 런타임
  `with_orthographic`과 동일 의미: view height = ortho_height, width =
  height × aspect).
- 기존 `render_world`는 무변경 (depths 회귀 보존). 공통 로직은 내부 헬퍼로
  추출해 중복 제거.
- leap 마이그레이션: visual 테스트가 게임과 동일한 직교 구도(ORTHO_HEIGHT
  12) 샷을 추가 단언 + 스크린샷 교체.

## 비범위

friction #4(grid2d 헬퍼) / #5(타일맵 배치 렌더) / #6(빌보드 z-rot) — P2,
별도 사이클. SpriteAnimation의 row 단위 클립 선택(2D 엔진식 "row = 클립"
규약)도 first_frame으로 충분해 미도입 (YAGNI).

## 검증

- 단위 TDD: first_frame 재생/wrap/clamp, flip_x 렌더 유니폼 스왑(uniform
  빌드 함수 단위 또는 capture 픽셀), ortho 캡처 픽셀(정면 직교에서 타일
  좌표가 화면 좌표에 선형 대응 — 특정 픽셀 색 단언 가능).
- leap 회귀 전체(단위 + 플레이스루 + visual) 그린 + capture 기존 회귀 그린.
- workspace build/clippy(-D warnings)/test 그린.
- CODEMAP 갱신: voltex_renderer_ecs, voltex_capture.
- friction-2 로그 #1~#3 취소선 + 해소 요약.
