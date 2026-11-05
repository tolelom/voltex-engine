# voltex_platform

## 책임 (한 줄)
윈도우/입력/게임 루프 — winit 0.30 위 얇은 래퍼.

## 공개 API 진입점
- `VoltexWindow` — winit 윈도우 래퍼 (src/window.rs)
- `InputState` — 키보드/마우스 입력 추적 + `window_size()`/`set_window_size()` (물리 픽셀, mouse_position과 동일 단위 — screen→NDC 변환용; runtime이 창 생성/Resized 시 주입, clear_frame_edges에 유지) (src/input.rs)
- `InputMap` / `Trigger` — 시맨틱 액션/축 매핑 레이어 (src/input_map.rs). `bind_key`/`bind_mouse`/`bind_axis` 로 바인딩, `is_pressed`/`just_pressed`/`just_released`/`axis`/`vector(&InputState)` 로 질의. InputState 위 순수 매핑(상태 없음 → 윈도우/GPU 없이 테스트 가능)
- `KeyCode` / `MouseButton` — winit enum 재export (게임이 winit 직접 의존 회피)
- `GameTimer` — frame timing, delta time (src/game_loop.rs)

## 모듈 구조
- `window` — VoltexWindow + winit 통합 (src/window.rs)
- `input` — InputState (src/input.rs)
- `input_map` — InputMap / Trigger (src/input_map.rs)
- `game_loop` — GameTimer (src/game_loop.rs)

## 의존성
- voltex crate: voltex_math (cursor position 등에 Vec2 사용)
- 외부 crate: winit 0.30, log (로깅)

## 변경 시 주의점
- winit 버전 업그레이드 시 ApplicationHandler API 변경 점검 필요
- InputState 키 코드는 winit의 PhysicalKey 기준
- `InputMap` 은 마우스 just-released 를 지원 안 함 (InputState 에 public 질의 없음) — mouse 트리거의 `just_released` 는 항상 false
- 모든 GPU 컨텍스트(voltex_renderer)는 VoltexWindow의 raw window handle 필요

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_platform --lib`
- 현재 테스트 개수: 21개
