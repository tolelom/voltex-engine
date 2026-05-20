# voltex_script

## 책임 (한 줄)
Lua 5.4 내장 + 엔진 API 바인딩 + 샌드박싱 + 핫 리로드.

## 공개 API 진입점
- `LuaState` — Lua 인터프리터 (src/state.rs)
- `LuaCoroutine` — 코루틴 (src/coroutine.rs)
- `Sandbox` — os/io 차단 (src/sandbox.rs)
- engine_api — spawn/position/velocity/rotation/scale/destroy/find_by_tag (src/engine_api.rs)
- bindings — voltex_print 등 (src/bindings.rs)
- ffi — Lua C API FFI (src/ffi.rs)

## 모듈 구조
- `state` — LuaState + push/read table/vec3, register_fn, exec (src/state.rs, 505줄 — M3)
- `coroutine` — LuaCoroutine (src/coroutine.rs)
- `sandbox` — 샌드박싱 (src/sandbox.rs)
- `engine_api` — Rust ↔ Lua 엔진 API (src/engine_api.rs)
- `bindings` — 표준 라이브러리 바인딩 (src/bindings.rs)
- `ffi` — Lua C FFI 선언 (src/ffi.rs)

## 의존성
- voltex crate: voltex_math (Vec3), voltex_ecs (Entity, World)
- 외부 crate: cc 1 (빌드 타임 Lua 컴파일)

## 변경 시 주의점
- engine_api는 현재 4개 함수만 노출 (DEFERRED 확장)
- Entity.generation은 하드코딩 0 버그 있음 (DEFERRED)
- state.rs는 505줄 (M3 분리 예정)
- Lua 5.4는 cc로 내장 빌드 — Windows MSVC 환경 의존

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_script --lib`
- 현재 테스트 개수: 18개
