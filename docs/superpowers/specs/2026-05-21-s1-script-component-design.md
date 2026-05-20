# Cycle 5 (S1) — per-entity Lua ScriptComponent + world query iterator 설계

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 5
**Sub-project**: S (Scripting v1)
**세션 수**: 2
**의존**: Cycle 4 (E2 — 라이프사이클 훅), Cycle 1 (C0a — Component trait 패턴) — 모두 완료

---

## 1. 목적

한 엔티티에 Lua 스크립트를 첨부 (`ScriptComponent { script_path, instance_id }`) 해서 매 프레임 `on_update(id, gen, dt)` 가 호출되도록. Play 진입 시 `on_start(id, gen)` 콜백, Play 종료 시 인스턴스 정리. Lua 측에서 World 의 entity 들을 순회할 수 있는 `iter_entities()` / `iter_by_tag(tag)` 두 iterator API 노출. 스크립트 파일이 변경되면 영향받은 인스턴스만 hot reload.

## 2. Scope / Out-of-scope

### 2.1 In-scope

#### 2.1.1 voltex_ecs
- 신규 모듈 `script_component.rs`: `ScriptComponent { script_path: String, instance_id: u32 }`
- `Component` trait 구현. `NAME = "script"`. 직렬화 포맷: `[u32 instance_id LE][UTF-8 script_path bytes]`. `instance_id` 는 런타임 상태이지만 직렬화/역직렬화시 보존 (저장 시점에 마지막으로 알았던 instance_id 그대로) — 실제 매핑은 pool 측에서 다시 해석. 미바인딩 sentinel: `u32::MAX`.
- `register_defaults()` 에 `ScriptComponent` 포함 → 에디터 snapshot/restore가 자동으로 보존
- 단위 테스트: roundtrip, too-short error path

#### 2.1.2 voltex_script
- 신규 모듈 `instance.rs`: `ScriptInstance { lua: LuaState, path: String, started: bool }`
- 신규 모듈 `pool.rs`: `ScriptInstancePool` — `instances: HashMap<u32, ScriptInstance>`, `next_id: u32`, `path_to_id: HashMap<String, u32>` (한 경로 = 한 instance 공유 정책: 같은 .lua 를 N개 entity 가 붙여도 LuaState 는 하나 — 글로벌 변수가 충돌하지 않도록 스크립트 작성자가 entity 인자만으로 동작하도록 한다).
- `pool.get_or_create(path) -> u32` — 경로 처음 보면 LuaState 생성 + exec_file + `register_engine_api(&lua, world)` + entry를 풀에 추가, 이미 있으면 같은 id 반환
- `pool.reload(path) -> bool` — 경로의 LuaState 에 `reload_file` 호출, `started = false` 로 다시 PlayStart 시 `on_start` 재호출 가능하게 (현재 Play 중 reload면 즉시 `on_start` 호출). 경로 미존재면 false.
- 신규 모듈 `system.rs`:
  - `script_start_all(world, pool)` — `ScriptComponent` 있는 모든 entity 에 대해 instance 확보 → `on_start(id, gen)` 호출 (instance.started=true 표시)
  - `script_update_all(world, pool, dt: f32)` — 모든 ScriptComponent 에 대해 `on_update(id, gen, dt)` 호출
- engine_api 신규 함수 2개 (현재 15 → 17):
  - `iter_entities() → Lua array table of {id, gen}` — 살아있는 모든 entity
  - `iter_by_tag(tag_string) → Lua array table of {id, gen}` — `find_all_by_tag` 와 동일 구현, 별도 이름으로 등록 (둘 다 유지 — `find_all_by_tag` 은 "single result expected" 사용패턴, `iter_by_tag` 은 "loop over all matches" 사용패턴, 의미 동일)
- Lua FFI 규약: `__voltex_world` light userdata 갱신은 `register_engine_api` 가 처리. 각 ScriptInstance 마다 한 번씩 호출.
- 단위 테스트 ≥8 (요구: ≥8)

#### 2.1.3 Hot reload 통합
- voltex_script 에 신규 helper `pool.poll_file_watcher(&mut FileWatcher)` — watcher가 보고한 변경 파일을 풀에서 reload 호출 (FileWatcher 의존성을 voltex_script → voltex_asset 으로 노출. 이미 voltex_ecs 가 voltex_asset 의존하므로 추가 의존 OK).
- 신규 의존성: `voltex_script` → `voltex_asset` (FileWatcher). 현재 voltex_script 는 L2 (math + ecs), voltex_asset 은 L0 이므로 L2→L0 추가 OK.

#### 2.1.4 데모 / 검증
- `assets/scripts/spinner.lua` — 매 update 마다 rotation.y 를 dt 만큼 증가
- voltex_script 통합 테스트: 5개 큐브 + tag "spinner" + `iter_by_tag` 로 모든 큐브 회전 → frame 진행 후 모두 rotation.y 증가 검증
- editor_demo / 다른 예제 변경 **없음** (S1 는 라이브러리 인프라; 시각 데모는 Cycle 6 의 runtime_demo 또는 future 에서 GUI 통합)

### 2.2 Out-of-scope

- Lua → Rust 사용자 컴포넌트 정의 (raw byte API 만 사용)
- 디버거 / 프로파일러
- 멀티 스크립트 인스턴스 (한 경로당 하나의 LuaState — 한 .lua 가 여러 entity 에 붙어도 LuaState 공유)
- `on_destroy` 콜백 (PlayStop 시 instance 정리는 silent — script 가 cleanup 필요하면 `on_start` 에서 매번 리셋)
- 스크립트별 cooldown / scheduling
- Lua coroutine 통합 (별도 `LuaCoroutine` 존재하지만 ScriptComponent 와 합치지 않음)
- 멀티 스레드 안전성 (voltex_script 는 단일 스레드)

## 3. 아키텍처

### 3.1 의존성 변경

```
voltex_script (L2)  → voltex_math (L0)
                     voltex_ecs   (L1)
                     voltex_asset (L0)   ← 신규 (FileWatcher)
```

다이어그램상 voltex_script 는 L2 유지. voltex_asset 은 L0 (no voltex deps) — cross-layer 허용.

### 3.2 ScriptComponent (voltex_ecs)

```rust
// crates/voltex_ecs/src/script_component.rs
use crate::component_registry::Component;
use crate::entity::Entity;
use crate::world::World;

/// A Lua script attached to an entity. The runtime maps `script_path` to a
/// shared LuaState owned by `voltex_script::ScriptInstancePool`. `instance_id`
/// is the runtime binding — `u32::MAX` means "not yet bound" and will be
/// resolved on the next `script_start_all` / `script_update_all` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptComponent {
    pub script_path: String,
    pub instance_id: u32,
}

impl ScriptComponent {
    pub const UNBOUND: u32 = u32::MAX;

    pub fn new(path: impl Into<String>) -> Self {
        Self {
            script_path: path.into(),
            instance_id: Self::UNBOUND,
        }
    }
}

impl Component for ScriptComponent {
    const NAME: &'static str = "script";

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let s = world.get::<ScriptComponent>(entity)?;
        let path_bytes = s.script_path.as_bytes();
        let mut data = Vec::with_capacity(4 + path_bytes.len());
        data.extend_from_slice(&s.instance_id.to_le_bytes());
        data.extend_from_slice(path_bytes);
        Some(data)
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> crate::Result<()> {
        if data.len() < 4 {
            return Err("ScriptComponent data too short".into());
        }
        let instance_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let script_path = std::str::from_utf8(&data[4..])
            .map_err(|_| crate::EcsError::from("Invalid UTF-8 in script path"))?
            .to_string();
        world.add(entity, ScriptComponent { script_path, instance_id });
        Ok(())
    }
}
```

`register_defaults()` 에 한 줄 추가: `self.register::<crate::ScriptComponent>();`. `lib.rs` 에 `pub mod script_component; pub use script_component::ScriptComponent;`.

**핵심 결정**: `instance_id` 는 직렬화하지만 deserialize 후 의미가 변할 수 있다. 풀 측은 `instance_id` 가 살아있는 매핑을 가리키지 않으면 (또는 `UNBOUND` 면) 첫 `script_update_all` 호출 시 경로로부터 인스턴스를 재확보하고 ID 를 갱신한다.

### 3.3 ScriptInstance + ScriptInstancePool (voltex_script)

```rust
// crates/voltex_script/src/instance.rs
use crate::state::LuaState;

pub struct ScriptInstance {
    pub lua: LuaState,
    pub path: String,
    pub started: bool,
}

// crates/voltex_script/src/pool.rs
use std::collections::HashMap;
use voltex_ecs::World;
use crate::engine_api::register_engine_api;
use crate::bindings::register_default_bindings;
use crate::instance::ScriptInstance;
use crate::state::LuaState;

pub struct ScriptInstancePool {
    instances: HashMap<u32, ScriptInstance>,
    path_to_id: HashMap<String, u32>,
    next_id: u32,
}

impl ScriptInstancePool {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            path_to_id: HashMap::new(),
            next_id: 0,
        }
    }

    /// Return the instance id for `path`, creating and loading the LuaState if
    /// this is the first time we see it. The world pointer is bound to each
    /// fresh LuaState so engine_api functions can reach it.
    pub fn get_or_create(&mut self, path: &str, world: *mut World) -> crate::Result<u32> {
        if let Some(&id) = self.path_to_id.get(path) {
            return Ok(id);
        }
        let lua = LuaState::new();
        register_default_bindings(&lua);
        register_engine_api(&lua, world);
        lua.exec_file(path)?;
        let id = self.next_id;
        self.next_id += 1;
        self.instances.insert(id, ScriptInstance {
            lua, path: path.to_string(), started: false,
        });
        self.path_to_id.insert(path.to_string(), id);
        Ok(id)
    }

    pub fn get(&self, id: u32) -> Option<&ScriptInstance> { self.instances.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut ScriptInstance> { self.instances.get_mut(&id) }
    pub fn find_by_path(&self, path: &str) -> Option<u32> { self.path_to_id.get(path).copied() }
    pub fn len(&self) -> usize { self.instances.len() }
    pub fn is_empty(&self) -> bool { self.instances.is_empty() }

    /// Re-execute the file on the existing LuaState (preserving any globals
    /// the previous version set up) and mark `started = false` so the next
    /// `script_start_all` re-runs `on_start`. Returns true if the instance
    /// existed.
    pub fn reload(&mut self, path: &str) -> crate::Result<bool> {
        let Some(&id) = self.path_to_id.get(path) else {
            return Ok(false);
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            return Ok(false);
        };
        inst.lua.reload_file(path)?;
        inst.started = false;
        Ok(true)
    }
}

impl Default for ScriptInstancePool {
    fn default() -> Self { Self::new() }
}
```

### 3.4 script_update_system + script_start_all (voltex_script::system)

`Scheduler::add(fn(&mut World))` 시그너처가 단일 인자라서 직접 등록은 불가. 두 함수는 자유 함수로 두고 호출자 (editor_demo, Cycle 6 의 RuntimeApp) 가 `pool` 과 `dt` 를 캡쳐한 클로저를 만들어 호출한다.

```rust
// crates/voltex_script/src/system.rs
use voltex_ecs::{World, Entity, ScriptComponent};
use crate::pool::ScriptInstancePool;
use crate::ffi;
use std::ffi::CString;

/// Fire `on_start(id, gen)` for every ScriptComponent whose instance has not
/// yet been started. Idempotent — only un-started instances get the call.
pub fn script_start_all(world: &mut World, pool: &mut ScriptInstancePool) {
    let world_ptr = world as *mut World;
    let entries: Vec<(Entity, String, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.script_path.clone(), s.instance_id))
        .collect();

    for (entity, path, mut iid) in entries {
        if iid == ScriptComponent::UNBOUND || pool.get(iid).is_none() {
            iid = match pool.get_or_create(&path, world_ptr) {
                Ok(id) => id,
                Err(_) => continue,
            };
            if let Some(s) = world.get_mut::<ScriptComponent>(entity) {
                s.instance_id = iid;
            }
        }
        let Some(inst) = pool.get_mut(iid) else { continue; };
        if inst.started { continue; }
        invoke_callback(&inst.lua, "on_start", entity, None);
        inst.started = true;
    }
}

/// Fire `on_update(id, gen, dt)` for every started ScriptComponent. Components
/// that have not yet been started are skipped here — call `script_start_all`
/// first.
pub fn script_update_all(world: &mut World, pool: &mut ScriptInstancePool, dt: f32) {
    let entries: Vec<(Entity, u32)> = world
        .query::<ScriptComponent>()
        .map(|(e, s)| (e, s.instance_id))
        .collect();

    for (entity, iid) in entries {
        let Some(inst) = pool.get_mut(iid) else { continue; };
        if !inst.started { continue; }
        invoke_callback(&inst.lua, "on_update", entity, Some(dt));
    }
}

fn invoke_callback(lua: &crate::state::LuaState, fn_name: &str, entity: Entity, dt: Option<f32>) {
    unsafe {
        let L = lua.raw();
        let c = CString::new(fn_name).unwrap();
        let ty = ffi::lua_getglobal(L, c.as_ptr());
        if ty != ffi::LUA_TFUNCTION {
            ffi::lua_pop(L, 1);
            return;
        }
        ffi::lua_pushnumber(L, entity.id as f64);
        ffi::lua_pushnumber(L, entity.generation as f64);
        let nargs = if let Some(dt) = dt {
            ffi::lua_pushnumber(L, dt as f64);
            3
        } else { 2 };
        let _ = ffi::lua_pcall(L, nargs, 0, 0);
        // discard any error message left on the stack
        if ffi::lua_gettop(L) > 0 {
            // best-effort cleanup; don't panic the host thread
            // (lua_pcall pushes an error string on failure)
        }
    }
}
```

**borrow-check 주의**: `script_start_all` 안에서 `world.get_mut::<ScriptComponent>` 와 `pool.get_mut(iid)` 가 겹친다. world_ptr 는 raw pointer 로 풀에 전달하고, ScriptComponent 갱신은 별도 단계에서 처리. 위 코드는 `world.query::<ScriptComponent>()` 결과를 먼저 Vec 으로 수집 → pool 호출 → 다시 `world.get_mut` 으로 갱신. 두 mutable borrow 가 겹치지 않는다.

**미해결 미니 이슈**: invoke_callback 안의 `ffi::lua_pcall` 가 실패할 때 에러 메시지가 스택에 남는다. v1 에서는 silent — 에러 로그는 향후 cycle 에서 (`log::error!` 통합). 스택 정리는 lua_pcall 의 에러 동작에 따라 `lua_settop(L, 0)` 으로 단순 비움.

### 3.5 engine_api 신규 함수 (voltex_script::engine_api)

```rust
pub fn register_engine_api(lua: &LuaState, world: *mut World) {
    // 기존 15개 ...
    lua.register_fn("iter_entities", lua_iter_entities);
    lua.register_fn("iter_by_tag", lua_iter_by_tag);
}

/// `iter_entities()` → array table of `{id=, gen=}` for every live entity.
unsafe extern "C" fn lua_iter_entities(L: *mut ffi::lua_State) -> c_int {
    let world = get_world(L);
    ffi::lua_createtable(L, 0, 0);
    let id_key = std::ffi::CString::new("id").unwrap();
    let gen_key = std::ffi::CString::new("gen").unwrap();
    let mut index: ffi::lua_Integer = 1;
    for entity in world.live_entities() {
        ffi::lua_createtable(L, 0, 2);
        ffi::lua_pushnumber(L, entity.id as f64);
        ffi::lua_setfield(L, -2, id_key.as_ptr());
        ffi::lua_pushnumber(L, entity.generation as f64);
        ffi::lua_setfield(L, -2, gen_key.as_ptr());
        ffi::lua_rawseti(L, -2, index);
        index += 1;
    }
    1
}

/// `iter_by_tag(tag)` → array table of `{id=, gen=}` (alias of find_all_by_tag).
unsafe extern "C" fn lua_iter_by_tag(L: *mut ffi::lua_State) -> c_int {
    lua_find_all_by_tag(L)
}
```

`world.live_entities()` 는 voltex_ecs 측에 추가 필요. 현재 World 는 `entity_count()` 만 있고 모든 살아있는 엔티티 iterator 가 없다 (할당기 내부 정보만 있음). EntityAllocator 가 alive 만 iterate 하는 메서드를 노출하거나 `World::live_entities() -> impl Iterator<Item=Entity>` 헬퍼 추가.

#### 3.5.1 World::live_entities 추가
```rust
// voltex_ecs::world
impl World {
    /// Iterate every currently alive entity (in id order).
    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.allocator.live_entities()
    }
}
```

`EntityAllocator::live_entities` 는 generation table + free list 를 기반으로 구현:

```rust
// voltex_ecs::entity::EntityAllocator
impl EntityAllocator {
    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.generations.iter().enumerate()
            .filter(|(id, _)| !self.is_free(*id as u32))
            .map(|(id, &gen)| Entity { id: id as u32, generation: gen })
    }
    // is_free 는 free_list 에 포함되어 있는지 — 별도 HashSet 캐시 또는 작은 N 에서 linear scan
}
```

기존 `EntityAllocator` 가 free list 로 어떻게 관리하는지 확인 필요 (구현 시 점검). 작은 데모용이라 O(N) scan 으로 충분.

### 3.6 Hot reload (voltex_script::pool)

```rust
impl ScriptInstancePool {
    /// Polls the given watcher for changes and reloads any tracked script
    /// file. Returns the list of paths that were reloaded.
    pub fn poll_reload(&mut self, watcher: &mut voltex_asset::FileWatcher) -> Vec<String> {
        let mut reloaded = Vec::new();
        for path in watcher.poll_changes() {
            let p = path.to_string_lossy().to_string();
            if self.reload(&p).is_ok() {
                reloaded.push(p);
            }
        }
        reloaded
    }

    pub fn watch(&self, path: &str, watcher: &mut voltex_asset::FileWatcher) {
        watcher.watch(std::path::PathBuf::from(path));
    }
}
```

이미 watch 된 파일은 또 watch 해도 무해 (FileWatcher 가 HashMap 이라 덮어쓰기 — 첫 mtime 기록만 reset). v1 수용.

### 3.7 spinner.lua 데모

```lua
-- assets/scripts/spinner.lua
function on_start(id, gen)
    -- Optional: bookkeeping. Empty in this demo.
end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
```

이 스크립트는 entity 의 rotation.y 를 매 update 마다 dt 증가. 5 entity 가 같은 LuaState 를 공유해도 인자만 다르므로 글로벌 충돌 없음.

## 4. 데이터 흐름

### 4.1 Edit 시 ScriptComponent 부착
```
사용자 (테스트 / 향후 editor UI):
  world.add(entity, ScriptComponent::new("scripts/spinner.lua"))
  // instance_id = UNBOUND
```

### 4.2 Play 진입
```
session.set_mode(Play)
  → lifecycle PlayStart 콜백 (사용자 등록):
       script_start_all(&mut world, &mut pool)
         → 각 ScriptComponent:
             pool.get_or_create(path, world_ptr) → 새 LuaState + exec_file + register_engine_api
             ScriptComponent.instance_id 갱신
             on_start(id, gen) 호출 (함수가 정의돼 있으면)
             inst.started = true
```

### 4.3 Play 중 매 프레임
```
사용자 메인 루프:
  if mode == Play:
    script_update_all(&mut world, &mut pool, dt)
      → 각 ScriptComponent (started=true 인 것만):
          on_update(id, gen, dt) 호출
    pool.poll_reload(&mut watcher) // 옵션: 핫리로드 점검
      → 변경 보고된 파일들에 대해 reload + started=false
      → 다음 script_start_all 가 on_start 다시 호출
    // (또는: 동일 프레임에서 즉시 script_start_all 재호출)
```

### 4.4 Stop → Edit
```
session.set_mode(Edit)
  → lifecycle PlayStop 콜백:
       pool.clear()  // 모든 LuaState drop
       // ScriptComponent 의 instance_id 는 snapshot 복원으로 어차피 stale → 다음 Play 시 재바인딩
  → world.clear() + deserialize_scene_binary
```

`pool.clear()` 메서드도 추가: `self.instances.clear(); self.path_to_id.clear(); self.next_id = 0;`. 단순 drop 으로 LuaState 가 정리됨 (Drop impl 이 lua_close).

## 5. 모듈 구조 변경

```
crates/voltex_ecs/src/
├── script_component.rs   # 신규
└── lib.rs                # pub mod script_component; pub use ScriptComponent;

crates/voltex_ecs/src/world.rs       # + live_entities() 헬퍼
crates/voltex_ecs/src/entity.rs      # + EntityAllocator::live_entities()
crates/voltex_ecs/src/component_registry.rs   # register_defaults() 에 한 줄

crates/voltex_script/src/
├── instance.rs           # 신규: ScriptInstance
├── pool.rs               # 신규: ScriptInstancePool
├── system.rs             # 신규: script_start_all, script_update_all
├── engine_api.rs         # + iter_entities, iter_by_tag
└── lib.rs                # pub mod instance/pool/system; pub use ScriptInstancePool 등

crates/voltex_script/Cargo.toml   # + voltex_asset workspace dep

assets/scripts/spinner.lua        # 신규
```

세션별 LOC 추정:
- voltex_ecs script_component.rs ~60줄
- voltex_ecs world/entity live_entities 추가 ~25줄
- voltex_script instance.rs ~10줄
- voltex_script pool.rs ~130줄
- voltex_script system.rs ~80줄
- voltex_script engine_api.rs +50줄 (현재 ~500줄 → ~550줄, 한도 600 유지)

600줄 한도 영향 없음.

## 6. 에러 처리

- `pool.get_or_create` 가 exec_file 실패 (스크립트 syntax error 등) 시 `ScriptError` 반환. caller (script_start_all) 는 silent skip — entity 가 그냥 동작 안 함.
- `pool.reload` 가 실패하면 false / Err 반환, 다음 update 도 silent.
- `lua_pcall` 실패는 silent (v1). log::error 통합은 future.
- `live_entities` 가 빈 iterator 면 빈 Lua table 반환.
- ScriptComponent deserialize 가 너무 짧으면 EcsError.
- `register_defaults` 가 ScriptComponent 등록 후 stale `instance_id` 가 들어와도 다음 update 가 자동 재바인딩.

## 7. 테스트 (≥8, 작성 10)

### voltex_ecs (3 신규)
| # | 파일 | 케이스 |
|---|---|---|
| 1 | script_component.rs | `roundtrip_preserves_path_and_instance_id` |
| 2 | script_component.rs | `deserialize_too_short_errors` |
| 3 | component_registry.rs | `register_defaults_includes_script` |

### voltex_script (7 신규, 누계 30→37)
| # | 파일 | 케이스 |
|---|---|---|
| 4 | pool.rs | `get_or_create_same_path_returns_same_id` |
| 5 | pool.rs | `get_or_create_loads_lua_file` (임시 .lua 작성 → exec_file 성공 검증) |
| 6 | pool.rs | `reload_resets_started_flag` |
| 7 | system.rs | `start_all_invokes_on_start_and_sets_flag` |
| 8 | system.rs | `update_all_calls_on_update_for_started_only` |
| 9 | system.rs | `five_cubes_rotate_via_iter_by_tag` — 5 entity + tag "spinner" + spinner.lua → 3 frame 진행 → rotation.y > 0 |
| 10 | engine_api.rs | `iter_entities_returns_all_live` (Lua 측에서 `iter_entities()` 후 `#result` 검증) |

테스트는 임시 디렉토리에 .lua 파일 작성 후 진행 (기존 `test_hot_reload` 패턴 재사용).

## 8. 시각 검증 (수동)

S1 자체는 라이브러리만 추가. UI 데모는 Cycle 6 의 runtime_demo 에서 통합. 본 cycle 에서는 통합 테스트 (#9) 가 행위 검증 역할.

editor_demo 에 ScriptComponent 통합은 future work (Cycle 12 의 AddComponent UI 가 나오면 자연스럽게 가능).

## 9. 세션 분할

### Session 1: ScriptComponent + Pool + system_* + 데모
1. `voltex_ecs::script_component` + Component impl + register_defaults + tests #1~3
2. `voltex_ecs::World::live_entities` + EntityAllocator helper
3. `voltex_script::instance` (ScriptInstance struct)
4. `voltex_script::pool` (ScriptInstancePool + get_or_create + reload + clear)
5. `voltex_script::system` (script_start_all + script_update_all + invoke_callback)
6. `assets/scripts/spinner.lua`
7. 테스트 #4~9 (pool + system)
8. cargo build/test workspace 깨끗
9. commit

### Session 2: Lua iterator API + hot reload + 통합 테스트
1. `voltex_script::engine_api` 에 `iter_entities` + `iter_by_tag` 추가
2. CODEMAP 의 engine_api 카운트 갱신 (15 → 17)
3. `voltex_script::pool::poll_reload` (FileWatcher 통합)
4. `voltex_script` Cargo.toml 에 voltex_asset 의존성 추가
5. 테스트 #10 (iter_entities)
6. 추가 테스트: hot reload 통합 (임시 디렉토리 + 파일 mtime 변경 + poll_reload + on_start 재호출 검증) — 옵션. 시간 남으면.
7. CODEMAP / STATUS / 로드맵 갱신
8. commit + 로드맵 `[✅]`

## 10. 위험 / 미지수

### 10.1 LuaState 공유 (다중 entity, 같은 .lua)
같은 .lua 가 5 entity 에 붙으면 같은 LuaState. on_update 가 글로벌 변수를 쓰면 모든 entity 가 공유. 정책: 스크립트 작성자가 entity 인자만 사용. spinner.lua 가 좋은 예 — 글로벌 안 씀.

향후 v1.1+ 에서 per-instance state 가 필요하면 LuaState 분리 또는 instance-keyed sub-table 패턴 도입.

### 10.2 world_ptr lifetime
`get_or_create(path, world_ptr)` 가 `*mut World` 를 LuaState 에 저장. World 가 drop 되면 Lua 측 호출이 use-after-free. v1 정책: pool 의 lifetime ≤ World 의 lifetime (사용자가 World 보다 pool 을 먼저 drop). 문서화. `EditorSession` 통합 시 둘 다 session 안에 두면 lifetime 자연 보장.

### 10.3 invoke_callback 의 lua_pcall 에러
silent. v1 OK. 디버깅이 어려울 수 있으므로 향후 log 통합 필요.

### 10.4 instance_id 직렬화의 의미
저장 후 다음 세션에서 deserialize 하면 `instance_id` 가 무의미한 값. 우리 정책: 첫 `script_start_all` 호출이 `UNBOUND` 또는 풀에 없는 id 를 보면 재바인딩. 따라서 저장된 값은 단순 기억일 뿐 정확성 영향 없음.

### 10.5 EntityAllocator 의 live_entities 비용
O(N) scan. 큰 World 에서는 비효율적이지만 v1 에는 충분. v1.1 에서 sparse set 의 dense 배열로 직접 iterate 가능하면 그쪽으로 최적화.

### 10.6 FileWatcher 가 voltex_script 의 인터페이스에 노출되는 적절성
대안: poll_reload 를 호출자가 손수 호출 (`for path in watcher.poll_changes() { pool.reload(path)?; }`). pool 안에 헬퍼 두는 게 사용성 좋음. voltex_script → voltex_asset 의존 추가는 다이어그램 변화이지만 cross-layer (L2 → L0) 단순.

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_ecs::script_component | 신규 (`ScriptComponent` + `Component` impl) |
| voltex_ecs::world / entity | + `live_entities()` 헬퍼 |
| voltex_ecs::component_registry | `register_defaults()` 에 ScriptComponent 한 줄 |
| voltex_ecs::lib | `pub mod script_component`, re-export |
| voltex_script::instance | 신규 |
| voltex_script::pool | 신규 |
| voltex_script::system | 신규 |
| voltex_script::engine_api | +2 함수 |
| voltex_script::lib | re-exports |
| voltex_script::Cargo.toml | + voltex_asset dep |
| assets/scripts/spinner.lua | 신규 |
| CODEMAP.md (voltex_ecs, voltex_script) | 갱신 |
| STATUS.md / roadmap | Cycle 5 `[✅]` |
| 다른 crate / examples | 변경 없음 |

## 12. Done 기준 (로드맵 § Cycle 5)

- [ ] spinner.lua 가 한 entity 의 rotation.y 를 매 frame 증가시킴 (통합 테스트 #9 가 검증)
- [ ] iter_by_tag 로 5 entity 동시 회전 (통합 테스트 #9)
- [ ] voltex_script 단위 테스트 +8 이상 (작성: 7 voltex_script + 3 voltex_ecs = 10)
- [ ] hot reload — 파일 변경 후 `pool.poll_reload(&mut watcher)` 가 reload 되고 다음 `script_start_all` 가 `on_start` 재호출 (선택 테스트)
- [ ] `cargo build --workspace` 깨끗
- [ ] `cargo test --workspace --lib` 통과 (현재 850 + 약 10 = ~860)
- [ ] STATUS.md, 로드맵 Cycle 5 `[✅]` + done commit hash
- [ ] CODEMAP.md (voltex_ecs / voltex_script) 갱신
