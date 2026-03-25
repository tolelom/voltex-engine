# Phase 3b-4a Deferred Items Spec

## A. 씬 직렬화 (`voltex_ecs`)

### JSON 직렬화 (`scene.rs` 확장)

- `serialize_scene_json(world, registry) -> String`
- `deserialize_scene_json(world, json, registry) -> Result<Vec<Entity>, String>`
- voltex_ecs 내부에 미니 JSON writer + 미니 JSON parser (voltex_renderer 의존 없음)

포맷:
```json
{"version":1,"entities":[
  {"transform":{"position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]},
   "tag":"player","parent":null,"components":{"rigid_body":{...}}}
]}
```

### 바이너리 씬 포맷 (`binary_scene.rs` 신규)

- `serialize_scene_binary(world, registry) -> Vec<u8>`
- `deserialize_scene_binary(world, data, registry) -> Result<Vec<Entity>, String>`

포맷:
```
[4] magic "VSCN"
[4] version u32 LE
[4] entity_count u32 LE
per entity:
  [4] component_count u32 LE
  per component:
    [2] name_len u16 LE
    [N] name bytes (UTF-8)
    [4] data_len u32 LE
    [N] data bytes
```

### 임의 컴포넌트 등록 (`component_registry.rs` 신규)

```rust
pub type SerializeFn = fn(&World, Entity) -> Option<Vec<u8>>;
pub type DeserializeFn = fn(&mut World, Entity, &[u8]) -> Result<(), String>;

pub struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

struct ComponentEntry {
    name: String,
    serialize: SerializeFn,
    deserialize: DeserializeFn,
}

impl ComponentRegistry {
    pub fn new() -> Self
    pub fn register(&mut self, name: &str, ser: SerializeFn, deser: DeserializeFn)
    pub fn register_defaults(&mut self)  // Transform, Parent, Tag
}
```

---

## B. 비동기 로딩 + 핫 리로드 (`voltex_asset`)

### 비동기 로딩 (`loader.rs` 신규)

```rust
pub enum LoadState {
    Loading,
    Ready,
    Failed(String),
}

pub struct AssetLoader {
    sender: Sender<LoadRequest>,
    receiver: Receiver<LoadResult>,
    thread: Option<JoinHandle<()>>,
    pending: HashMap<u64, PendingAsset>,
}

impl AssetLoader {
    pub fn new() -> Self  // 워커 스레드 1개 시작
    pub fn load<T: Send + 'static>(
        &mut self, path: PathBuf,
        parse: fn(&[u8]) -> Result<T, String>,
    ) -> Handle<T>  // 즉시 핸들 반환, 백그라운드 로딩
    pub fn state<T: 'static>(&self, handle: Handle<T>) -> LoadState
    pub fn process_loaded(&mut self, assets: &mut Assets)  // 매 프레임 호출
    pub fn shutdown(self)
}
```

### 핫 리로드 (`watcher.rs` 신규)

```rust
pub struct FileWatcher {
    watched: HashMap<PathBuf, SystemTime>,  // path → last_modified
    poll_interval: Duration,
    last_poll: Instant,
}

impl FileWatcher {
    pub fn new(poll_interval: Duration) -> Self
    pub fn watch(&mut self, path: PathBuf)
    pub fn unwatch(&mut self, path: &Path)
    pub fn poll_changes(&mut self) -> Vec<PathBuf>  // std::fs::metadata 기반
}
```

- 외부 크레이트 없음, `std::fs::metadata().modified()` 사용
- 변경 감지 시 AssetLoader로 재로딩 트리거
- 기존 Handle은 유지, AssetStorage에서 in-place swap (generation 유지)

---

## C. PBR 텍스처 맵 (`voltex_renderer`)

### 텍스처 바인딩 확장

Group 1 확장 (4→8 바인딩):
```
Binding 0-1: Albedo texture + sampler (기존)
Binding 2-3: Normal map texture + sampler (기존)
Binding 4-5: Metallic/Roughness/AO (ORM) texture + sampler (신규)
Binding 6-7: Emissive texture + sampler (신규)
```

- ORM 텍스처: R=AO, G=Roughness, B=Metallic (glTF ORM 패턴)
- 텍스처 없으면 기본 1x1 white 사용

### 셰이더 변경

`pbr_shader.wgsl` + `deferred_gbuffer.wgsl`:
```wgsl
@group(1) @binding(4) var t_orm: texture_2d<f32>;
@group(1) @binding(5) var s_orm: sampler;
@group(1) @binding(6) var t_emissive: texture_2d<f32>;
@group(1) @binding(7) var s_emissive: sampler;

// Fragment:
let orm = textureSample(t_orm, s_orm, in.uv);
let ao = orm.r * material.ao;
let roughness = orm.g * material.roughness;
let metallic = orm.b * material.metallic;
let emissive = textureSample(t_emissive, s_emissive, in.uv).rgb;
// ... add emissive to final color
```

### MaterialUniform 변경 없음
- 기존 metallic/roughness/ao 값은 텍스처 값의 승수(multiplier)로 작동
- 텍스처 없을 때 white(1,1,1) × material 값 = 기존과 동일 결과

### 텍스처 유틸 확장 (`texture.rs`)
- `pbr_full_texture_bind_group_layout()` — 8 바인딩 레이아웃
- `create_pbr_full_texture_bind_group()` — albedo + normal + ORM + emissive
- `black_1x1()` — emissive 기본값 (검정 = 발광 없음)
