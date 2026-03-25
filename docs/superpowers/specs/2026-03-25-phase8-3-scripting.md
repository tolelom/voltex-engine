# Phase 8-3: Lua Scripting — Design Spec

## Overview

`voltex_script` crate를 신규 생성한다. Lua 5.4 소스를 내장 빌드하고, FFI로 Lua C API를 호출하는 안전한 래퍼를 구현한다.

## Scope

- Lua 5.4 소스 내장 (cc crate로 빌드)
- Lua C API FFI 선언
- LuaState 안전한 래퍼 (exec, exec_file, 글로벌 변수 읽기/쓰기)
- Rust 함수를 Lua에 등록 (register_fn)
- 기본 바인딩 (voltex_print)

## Out of Scope

- 핫 리로드 (파일 변경 감지)
- 엔진 API 노출 (ECS, 물리 등)
- Lua 테이블 ↔ Rust 구조체 변환
- 코루틴 지원
- 에러 핸들링 고도화

## Module Structure

```
crates/voltex_script/
├── Cargo.toml
├── build.rs              — cc::Build로 Lua 5.4 컴파일
├── lua/                  — Lua 5.4 소스코드 (.c, .h)
└── src/
    ├── lib.rs
    ├── ffi.rs            — Lua C API extern 선언
    ├── state.rs          — LuaState 래퍼
    └── bindings.rs       — 기본 Rust→Lua 바인딩
```

## Dependencies

- `cc` (build dependency) — C 소스 컴파일

## Lua 5.4 소스 내장

lua/ 디렉토리에 Lua 5.4.7 소스코드를 배치. build.rs에서 cc::Build로 컴파일.

제외 파일: lua.c (standalone interpreter), luac.c (standalone compiler) — main() 충돌 방지.

Windows: `LUA_USE_WINDOWS` 또는 기본값 사용.

## Types

### ffi.rs

```rust
pub enum lua_State {}
pub type lua_CFunction = unsafe extern "C" fn(*mut lua_State) -> c_int;
pub type lua_Number = f64;
pub type lua_Integer = i64;
```

핵심 함수들:
- luaL_newstate, luaL_openlibs, lua_close
- luaL_dostring, luaL_loadfilex
- lua_pcallk (lua_pcall 매크로 대체)
- lua_pushnumber, lua_pushstring, lua_pushcclosure
- lua_tonumberx, lua_tolstring
- lua_setglobal, lua_getglobal
- lua_gettop, lua_settop
- lua_type, lua_typename

### LuaState

```rust
pub struct LuaState {
    state: *mut ffi::lua_State,
}
```

- `new() -> Self` — luaL_newstate + luaL_openlibs
- `exec(code: &str) -> Result<(), String>` — luaL_dostring, 에러 시 스택 메시지 반환
- `exec_file(path: &str) -> Result<(), String>` — luaL_loadfilex + lua_pcall
- `register_fn(name: &str, f: ffi::lua_CFunction)` — lua_pushcclosure + lua_setglobal
- `get_global_number(name: &str) -> Option<f64>` — lua_getglobal + lua_tonumberx
- `get_global_string(name: &str) -> Option<String>` — lua_getglobal + lua_tolstring
- `set_global_number(name: &str, value: f64)` — lua_pushnumber + lua_setglobal
- `set_global_string(name: &str, value: &str)` — lua_pushstring + lua_setglobal
- `Drop` — lua_close

unsafe impl Send for LuaState {} (Lua state는 단일 스레드에서만 사용)

### bindings.rs

```rust
pub fn register_default_bindings(lua: &LuaState)
```

기본 바인딩:
- `voltex_print(msg)` — Rust println!으로 출력
- `voltex_log(msg)` — 로그

## build.rs

```rust
fn main() {
    let mut build = cc::Build::new();
    build.include("lua");

    for entry in std::fs::read_dir("lua").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "c") {
            let name = path.file_name().unwrap().to_str().unwrap();
            if name != "lua.c" && name != "luac.c" {
                build.file(&path);
            }
        }
    }

    build.compile("lua54");
}
```

## Test Plan

### state.rs
- LuaState 생성/삭제 (new + drop)
- exec 간단한 코드 ("x = 1 + 2")
- exec 에러 코드 → Err
- get_global_number: Lua에서 설정한 변수 읽기
- set_global_number + get: 라운드트립
- get_global_string: 문자열 변수
- register_fn: Rust 함수 등록 후 Lua에서 호출

### bindings.rs
- voltex_print 호출 (크래시 없이 실행)
