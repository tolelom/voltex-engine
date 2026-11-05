# voltex_script

## 책임 (한 줄)
Lua 5.4 내장 + 엔진 API 바인딩 + 샌드박싱 + 핫 리로드 + per-entity ScriptComponent 통합 (S1).

## 공개 API 진입점
- `LuaState` — Lua 인터프리터 (src/state/mod.rs)
- `LuaCoroutine` / `CoroutineStatus` — 코루틴 (create_coroutine/resume/is_finished) (src/coroutine.rs). M3 이후 미배선(침묵하는 죽은 코드)이었다가 module_wiring 가드 도입 시 배선
- `create_sandbox(&LuaState)` — os/io/require/package/debug 등 위험 글로벌 제거(`package`는 v1.0.141 — `package.loaded.os` 우회와 `loadlib` 차단). **자동 적용 아님** — 신뢰할 수 없는 스크립트 실행 전 명시 호출 (src/sandbox.rs). 동일하게 미배선이었다가 배선 (coroutine+sandbox 테스트 +17 부활)
- engine_api — spawn/position/rotation/scale/destroy/tag/find/hierarchy/iter/find_in_radius (18 fns) (src/engine_api/)
- engine_input — 읽기 전용 `input` Lua 테이블 + `ScriptInput` 스냅샷 + `set_current_input` (src/engine_input.rs)
- bindings — voltex_print 등 (src/bindings.rs)
- ffi — Lua C API FFI (src/ffi.rs)
- `ScriptInstance` — 한 .lua 파일 하나의 LuaState (src/instance.rs)
- `ScriptInstancePool` — 경로 → LuaState 매핑 + get_or_create / reload / clear / poll_reload (FileWatcher 통합) (src/pool.rs)
- `script_start_all`, `script_update_all` — World 의 ScriptComponent 들에 on_start / on_update 디스패치 (src/system.rs)
- `ScriptError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `state` — LuaState + push/read table/vec3, register_fn, exec (src/state/mod.rs, 테스트는 src/state/tests.rs)
- `coroutine` — LuaCoroutine (src/coroutine.rs)
- `sandbox` — 샌드박싱 (src/sandbox.rs)
- `engine_api` — Rust ↔ Lua 엔진 API (src/engine_api/)
- `engine_input` — Lua `input` 테이블 (키/마우스 읽기), thread-local 스냅샷 포인터 (src/engine_input.rs)
- `bindings` — 표준 라이브러리 바인딩 (src/bindings.rs)
- `ffi` — Lua C FFI 선언 (src/ffi.rs)
- `instance` — ScriptInstance struct (lua + path + started flag) (src/instance.rs)
- `pool` — ScriptInstancePool (src/pool.rs)
- `system` — script_start_all / script_update_all + Lua callback invocation (src/system.rs)

## 의존성
- voltex crate: voltex_math (Vec3), voltex_ecs (Entity, World, ScriptComponent), voltex_asset (FileWatcher), voltex_ai (NavAgent — Lua nav API), voltex_audio (AudioSource — Lua audio API)
- 외부 crate: cc 1 (빌드 타임 Lua 컴파일)

## 변경 시 주의점
- engine_api: spawn_entity / destroy_entity / set_position / get_position / set_rotation / get_rotation / set_scale / get_scale / set_tag / get_tag / find_by_tag / find_all_by_tag / add_child / remove_child / entity_count / iter_entities / iter_by_tag / **find_in_radius** (18개)
- **find_in_radius(x,y,z,r)**: SpatialGrid(voltex_ecs L1)로 반경 내 Transform 엔티티 탐색 → `{id,gen}` 테이블. Lua AI가 근처 적/대상 탐색(서바이버류). 비숫자/world 없음 → 빈 테이블. 매 호출 grid 재구축(O(n)).
- **Entity handle 규약**: Lua ↔ Rust 모든 함수가 `(id, gen)` **두 값**으로 entity 전달. spawn_entity → 2 returns; set/get_position → id+gen+payload. stale handle은 `is_alive` 로 거부.
- **find_all_by_tag / iter_by_tag**: 항상 Lua 테이블 1개 반환 (매치 없으면 빈 테이블). 각 원소는 `{id=, gen=}` 서브테이블. `#result` 로 카운트.
- **iter_entities**: 살아있는 모든 entity 의 `{id=, gen=}` 배열. World 가 비면 빈 테이블.
- **ScriptInstancePool**: 한 .lua 파일당 **하나**의 LuaState 공유 (여러 entity 가 같은 path 면 같은 인터프리터). per-entity 상태는 스크립트가 인자(id, gen) 만으로 처리해야 함. 글로벌 사용은 entity 간 공유.
- **`script_start_all` 호출 순서**: ScriptComponent.instance_id == UNBOUND (or pool 에 없는 id) → 자동 재바인딩 + `on_start` 호출 + started=true. 이후 `script_update_all` 가 매 프레임 `on_update(id, gen, dt)`.
- **Lua callback 미정의**: 함수가 없으면 silent skip (panic 없음). `lua_pcall` 실패도 silent (v1).
- **Lua nav/audio API**: `nav_set_target(id,gen,x,y,z)` / `nav_clear_target` / `nav_reached(id,gen)->bool` (NavAgent), `audio_play(id,gen)` / `audio_stop` (AudioSource). 모두 stale handle/컴포넌트 없음/비숫자 인자에 silent no-op. engine_api가 voltex_ai/voltex_audio 의존 — Transform/Tag처럼 엔진 컴포넌트를 아는 스크립팅 표면이라 의도적.
- **Lua input API**: `input` 전역 테이블 (is_key_pressed/is_key_just_pressed/mouse_position/mouse_delta/is_mouse_button_pressed/scroll). 키는 **문자열 이름**("W","Space",...) — 이름↔keycode 매핑은 winit 의존을 피하려 `voltex_runtime`(SCRIPT_KEY_NAMES)이 소유. `voltex_script`는 winit 무의존, POD `ScriptInput` 스냅샷만 안다. 런타임이 매 프레임 `set_current_input(&snapshot)` 발행(thread-local), 실행 후 null 로 클리어. 스냅샷 미발행 시 모든 input 함수는 false/0 반환.
- **Drop 순서**: ScriptInstancePool 이 World 보다 먼저 drop 되어야 LuaState 내부의 `__voltex_world` light userdata 가 dangling 안 됨 (실질적 use-after-free 는 발생 안 하지만 정책적 정렬).
- `state/mod.rs`는 SAFETY 주석 추가로 600줄을 넘어 테스트를 `state/tests.rs`로 분리했다.
- Lua 5.4는 cc로 내장 빌드 — Windows MSVC 환경 의존

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_script --lib`
- 현재 테스트 개수: 75개 (coroutine/sandbox 배선으로 +17 부활 — 59 표기는 미배선 시절 수치)
