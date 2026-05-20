# voltex_script

## 책임 (한 줄)
Lua 5.4 내장 + 엔진 API 바인딩 + 샌드박싱 + 핫 리로드 + per-entity ScriptComponent 통합 (S1).

## 공개 API 진입점
- `LuaState` — Lua 인터프리터 (src/state.rs)
- `LuaCoroutine` — 코루틴 (src/coroutine.rs)
- `Sandbox` — os/io 차단 (src/sandbox.rs)
- engine_api — spawn/position/rotation/scale/destroy/tag/find/hierarchy/iter (17 fns) (src/engine_api.rs)
- bindings — voltex_print 등 (src/bindings.rs)
- ffi — Lua C API FFI (src/ffi.rs)
- `ScriptInstance` — 한 .lua 파일 하나의 LuaState (src/instance.rs)
- `ScriptInstancePool` — 경로 → LuaState 매핑 + get_or_create / reload / clear / poll_reload (FileWatcher 통합) (src/pool.rs)
- `script_start_all`, `script_update_all` — World 의 ScriptComponent 들에 on_start / on_update 디스패치 (src/system.rs)
- `ScriptError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `state` — LuaState + push/read table/vec3, register_fn, exec (src/state.rs, 505줄)
- `coroutine` — LuaCoroutine (src/coroutine.rs)
- `sandbox` — 샌드박싱 (src/sandbox.rs)
- `engine_api` — Rust ↔ Lua 엔진 API (src/engine_api.rs)
- `bindings` — 표준 라이브러리 바인딩 (src/bindings.rs)
- `ffi` — Lua C FFI 선언 (src/ffi.rs)
- `instance` — ScriptInstance struct (lua + path + started flag) (src/instance.rs)
- `pool` — ScriptInstancePool (src/pool.rs)
- `system` — script_start_all / script_update_all + Lua callback invocation (src/system.rs)

## 의존성
- voltex crate: voltex_math (Vec3), voltex_ecs (Entity, World, ScriptComponent), voltex_asset (FileWatcher)
- 외부 crate: cc 1 (빌드 타임 Lua 컴파일)

## 변경 시 주의점
- engine_api: spawn_entity / destroy_entity / set_position / get_position / set_rotation / get_rotation / set_scale / get_scale / set_tag / get_tag / find_by_tag / find_all_by_tag / add_child / remove_child / entity_count / **iter_entities / iter_by_tag** (17개)
- **Entity handle 규약**: Lua ↔ Rust 모든 함수가 `(id, gen)` **두 값**으로 entity 전달. spawn_entity → 2 returns; set/get_position → id+gen+payload. stale handle은 `is_alive` 로 거부.
- **find_all_by_tag / iter_by_tag**: 항상 Lua 테이블 1개 반환 (매치 없으면 빈 테이블). 각 원소는 `{id=, gen=}` 서브테이블. `#result` 로 카운트.
- **iter_entities**: 살아있는 모든 entity 의 `{id=, gen=}` 배열. World 가 비면 빈 테이블.
- **ScriptInstancePool**: 한 .lua 파일당 **하나**의 LuaState 공유 (여러 entity 가 같은 path 면 같은 인터프리터). per-entity 상태는 스크립트가 인자(id, gen) 만으로 처리해야 함. 글로벌 사용은 entity 간 공유.
- **`script_start_all` 호출 순서**: ScriptComponent.instance_id == UNBOUND (or pool 에 없는 id) → 자동 재바인딩 + `on_start` 호출 + started=true. 이후 `script_update_all` 가 매 프레임 `on_update(id, gen, dt)`.
- **Lua callback 미정의**: 함수가 없으면 silent skip (panic 없음). `lua_pcall` 실패도 silent (v1).
- **Drop 순서**: ScriptInstancePool 이 World 보다 먼저 drop 되어야 LuaState 내부의 `__voltex_world` light userdata 가 dangling 안 됨 (실질적 use-after-free 는 발생 안 하지만 정책적 정렬).
- state.rs는 505줄 (spec 만족, 분리 대상 아님)
- Lua 5.4는 cc로 내장 빌드 — Windows MSVC 환경 의존

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_script --lib`
- 현재 테스트 개수: 42개 (S1 +12: pool ×6, system ×4, engine_api iter ×2)
