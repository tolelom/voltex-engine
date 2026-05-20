# voltex_script

## 책임 (한 줄)
Lua 5.4 내장 + 엔진 API 바인딩 + 샌드박싱 + 핫 리로드.

## 공개 API 진입점
- `LuaState` — Lua 인터프리터 (src/state.rs)
- `LuaCoroutine` — 코루틴 (src/coroutine.rs)
- `Sandbox` — os/io 차단 (src/sandbox.rs)
- engine_api — spawn/position/rotation/scale/destroy/tag/find/hierarchy (15 fns) (src/engine_api.rs)
- bindings — voltex_print 등 (src/bindings.rs)
- ffi — Lua C API FFI (src/ffi.rs)
- `ScriptError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `state` — LuaState + push/read table/vec3, register_fn, exec (src/state.rs, 505줄)
- `coroutine` — LuaCoroutine (src/coroutine.rs)
- `sandbox` — 샌드박싱 (src/sandbox.rs)
- `engine_api` — Rust ↔ Lua 엔진 API (src/engine_api.rs)
- `bindings` — 표준 라이브러리 바인딩 (src/bindings.rs)
- `ffi` — Lua C FFI 선언 (src/ffi.rs)

## 의존성
- voltex crate: voltex_math (Vec3), voltex_ecs (Entity, World)
- 외부 crate: cc 1 (빌드 타임 Lua 컴파일)

## 변경 시 주의점
- engine_api: spawn_entity / destroy_entity / set_position / get_position / set_rotation / get_rotation / set_scale / get_scale / set_tag / get_tag / find_by_tag / find_all_by_tag / add_child / remove_child / entity_count (15개)
- **Entity handle 규약**: Lua ↔ Rust 모든 함수가 `(id, gen)` **두 값**으로 entity 전달. spawn_entity → 2 returns; set/get_position → id+gen+payload. stale handle은 `is_alive` 로 거부.
- **find_all_by_tag**: 항상 Lua 테이블 1개 반환 (매치 없으면 빈 테이블). 각 원소는 `{id=, gen=}` 서브테이블. `#result` 로 카운트.
- state.rs는 505줄 (spec 만족, 분리 대상 아님)
- Lua 5.4는 cc로 내장 빌드 — Windows MSVC 환경 의존

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_script --lib`
- 현재 테스트 개수: 30개 (engine_api 8-7 +5: add_child/remove_child/stale-parent/find_all_by_tag×2)
