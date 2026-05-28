# voltex_platform

## 책임 (한 줄)
윈도우/입력/게임 루프 — winit 0.30 위 얇은 래퍼.

## 공개 API 진입점
- `VoltexWindow` — winit 윈도우 래퍼 (src/window.rs)
- `InputState` — 키보드/마우스 입력 추적 (src/input.rs)
- `GameTimer` — frame timing, delta time (src/game_loop.rs)

## 모듈 구조
- `window` — VoltexWindow + winit 통합 (src/window.rs)
- `input` — InputState (src/input.rs)
- `game_loop` — GameTimer (src/game_loop.rs)

## 의존성
- voltex crate: voltex_math (cursor position 등에 Vec2 사용)
- 외부 crate: winit 0.30, log (로깅)

## 변경 시 주의점
- winit 버전 업그레이드 시 ApplicationHandler API 변경 점검 필요
- InputState 키 코드는 winit의 PhysicalKey 기준
- 모든 GPU 컨텍스트(voltex_renderer)는 VoltexWindow의 raw window handle 필요

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_platform --lib`
- 현재 테스트 개수: 3개
