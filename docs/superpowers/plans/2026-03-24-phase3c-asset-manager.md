# Phase 3c: Asset Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 핸들 기반 에셋 관리 시스템으로 Mesh, Texture 등의 에셋을 타입별로 저장/조회/삭제하고, 참조 카운팅으로 메모리를 관리한다.

**Architecture:** 새 `voltex_asset` crate를 만든다. `Handle<T>`는 경량 타입 안전 참조(u32 id + generation). `AssetStorage<T>`는 제네릭 에셋 저장소로 참조 카운팅 지원. `Assets`는 중앙 관리자로 타입별 스토리지를 type-erased로 관리한다. 로딩은 앱에서 기존 파서(parse_obj, parse_bmp)로 에셋을 만들고 `insert`하는 방식.

**Tech Stack:** Rust 1.94

**Spec:** `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` Phase 3 (3-3. 에셋 매니저)

**스코프 제한:** 비동기 로딩, 핫 리로드는 별도 Phase로 분리. 이번에는 핸들 + 스토리지 + 참조 카운팅만 구현.

---

## File Structure

```
crates/
└── voltex_asset/
    ├── Cargo.toml
    └── src/
        ├── lib.rs          # 모듈 re-export
        ├── handle.rs       # Handle<T> 제네릭 핸들
        ├── storage.rs      # AssetStorage<T> 참조 카운팅 스토리지
        └── assets.rs       # Assets 중앙 관리자 (type-erased)
examples/
└── asset_demo/             # 에셋 매니저 통합 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

---

## Task 1: Handle<T> + AssetStorage<T>

**Files:**
- Create: `crates/voltex_asset/Cargo.toml`
- Create: `crates/voltex_asset/src/lib.rs`
- Create: `crates/voltex_asset/src/handle.rs`
- Create: `crates/voltex_asset/src/storage.rs`
- Modify: `Cargo.toml` (워크스페이스에 voltex_asset 추가)

Handle은 에셋을 가리키는 경량 식별자. AssetStorage는 에셋 + 참조 카운트를 관리.

- [ ] **Step 1: Cargo.toml 작성**

```toml
# crates/voltex_asset/Cargo.toml
[package]
name = "voltex_asset"
version = "0.1.0"
edition = "2021"

[dependencies]
```

- [ ] **Step 2: 워크스페이스 업데이트**

`Cargo.toml` root: members에 `"crates/voltex_asset"` 추가, workspace.dependencies에 `voltex_asset = { path = "crates/voltex_asset" }` 추가.

- [ ] **Step 3: handle.rs 작성**

```rust
// crates/voltex_asset/src/handle.rs
use std::marker::PhantomData;

/// 타입 안전 에셋 핸들. 경량 식별자로 에셋을 참조.
#[derive(Debug)]
pub struct Handle<T> {
    pub id: u32,
    pub generation: u32,
    _marker: PhantomData<T>,
}

// PhantomData로 인해 자동 구현이 안되므로 수동 구현
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self { id: self.id, generation: self.generation, _marker: PhantomData }
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
    }
}

impl<T> Handle<T> {
    pub(crate) fn new(id: u32, generation: u32) -> Self {
        Self { id, generation, _marker: PhantomData }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_copy() {
        let h: Handle<String> = Handle::new(0, 0);
        let h2 = h;
        assert_eq!(h, h2); // Copy 확인
    }

    #[test]
    fn test_handle_eq() {
        let a: Handle<i32> = Handle::new(1, 0);
        let b: Handle<i32> = Handle::new(1, 0);
        let c: Handle<i32> = Handle::new(1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
```

- [ ] **Step 4: storage.rs 작성**

```rust
// crates/voltex_asset/src/storage.rs
use crate::handle::Handle;
use std::any::Any;

struct AssetEntry<T> {
    asset: T,
    generation: u32,
    ref_count: u32,
}

/// 타입별 에셋 스토리지. 참조 카운팅으로 메모리 관리.
pub struct AssetStorage<T> {
    entries: Vec<Option<AssetEntry<T>>>,
    free_ids: Vec<u32>,
}

impl<T> AssetStorage<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_ids: Vec::new(),
        }
    }

    /// 에셋 추가. 핸들 반환. 초기 ref_count = 1.
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        if let Some(id) = self.free_ids.pop() {
            let idx = id as usize;
            let generation = match &self.entries[idx] {
                Some(e) => e.generation + 1,
                None => 0,
            };
            self.entries[idx] = Some(AssetEntry {
                asset,
                generation,
                ref_count: 1,
            });
            Handle::new(id, generation)
        } else {
            let id = self.entries.len() as u32;
            self.entries.push(Some(AssetEntry {
                asset,
                generation: 0,
                ref_count: 1,
            }));
            Handle::new(id, 0)
        }
    }

    /// 핸들로 에셋 참조
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let entry = self.entries.get(handle.id as usize)?.as_ref()?;
        if entry.generation == handle.generation {
            Some(&entry.asset)
        } else {
            None
        }
    }

    /// 핸들로 에셋 가변 참조
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let entry = self.entries.get_mut(handle.id as usize)?.as_mut()?;
        if entry.generation == handle.generation {
            Some(&mut entry.asset)
        } else {
            None
        }
    }

    /// 참조 카운트 증가
    pub fn add_ref(&mut self, handle: Handle<T>) {
        if let Some(Some(entry)) = self.entries.get_mut(handle.id as usize) {
            if entry.generation == handle.generation {
                entry.ref_count += 1;
            }
        }
    }

    /// 참조 카운트 감소. 0이 되면 에셋 삭제.
    /// 삭제되었으면 true 반환.
    pub fn release(&mut self, handle: Handle<T>) -> bool {
        let idx = handle.id as usize;
        if let Some(Some(entry)) = self.entries.get_mut(idx) {
            if entry.generation == handle.generation {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                if entry.ref_count == 0 {
                    self.entries[idx] = None;
                    self.free_ids.push(handle.id);
                    return true;
                }
            }
        }
        false
    }

    /// 현재 저장된 에셋 수
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 참조 카운트 조회
    pub fn ref_count(&self, handle: Handle<T>) -> u32 {
        self.entries.get(handle.id as usize)
            .and_then(|e| e.as_ref())
            .filter(|e| e.generation == handle.generation)
            .map(|e| e.ref_count)
            .unwrap_or(0)
    }

    /// 모든 에셋 순회 (handle, &asset)
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.entries.iter().enumerate().filter_map(|(i, entry)| {
            entry.as_ref().map(|e| (Handle::new(i as u32, e.generation), &e.asset))
        })
    }
}

/// Type erasure trait for Assets manager
pub trait AssetStorageDyn: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn count(&self) -> usize;
}

impl<T: 'static> AssetStorageDyn for AssetStorage<T> {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn count(&self) -> usize { self.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut storage = AssetStorage::new();
        let h = storage.insert("hello".to_string());
        assert_eq!(storage.get(h), Some(&"hello".to_string()));
    }

    #[test]
    fn test_get_mut() {
        let mut storage = AssetStorage::new();
        let h = storage.insert(vec![1, 2, 3]);
        storage.get_mut(h).unwrap().push(4);
        assert_eq!(storage.get(h).unwrap().len(), 4);
    }

    #[test]
    fn test_release_removes_at_zero() {
        let mut storage = AssetStorage::new();
        let h = storage.insert(42);
        assert_eq!(storage.len(), 1);
        let removed = storage.release(h);
        assert!(removed);
        assert_eq!(storage.len(), 0);
        assert_eq!(storage.get(h), None);
    }

    #[test]
    fn test_ref_counting() {
        let mut storage = AssetStorage::new();
        let h = storage.insert(42);
        assert_eq!(storage.ref_count(h), 1);
        storage.add_ref(h);
        assert_eq!(storage.ref_count(h), 2);
        assert!(!storage.release(h)); // ref_count 2 → 1, not removed
        assert_eq!(storage.ref_count(h), 1);
        assert!(storage.release(h)); // ref_count 1 → 0, removed
        assert_eq!(storage.ref_count(h), 0);
    }

    #[test]
    fn test_stale_handle() {
        let mut storage = AssetStorage::new();
        let h1 = storage.insert(10);
        storage.release(h1);
        let h2 = storage.insert(20); // reuse slot, generation+1
        assert_eq!(storage.get(h1), None); // old handle invalid
        assert_eq!(storage.get(h2), Some(&20));
    }

    #[test]
    fn test_id_reuse() {
        let mut storage = AssetStorage::new();
        let h1 = storage.insert("first");
        let id1 = h1.id;
        storage.release(h1);
        let h2 = storage.insert("second");
        assert_eq!(h2.id, id1); // ID 재사용
        assert_eq!(h2.generation, 1); // generation 증가
    }

    #[test]
    fn test_iter() {
        let mut storage = AssetStorage::new();
        storage.insert(10);
        storage.insert(20);
        storage.insert(30);
        let values: Vec<i32> = storage.iter().map(|(_, v)| *v).collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&10));
        assert!(values.contains(&20));
        assert!(values.contains(&30));
    }
}
```

- [ ] **Step 5: lib.rs 작성**

```rust
// crates/voltex_asset/src/lib.rs
pub mod handle;
pub mod storage;

pub use handle::Handle;
pub use storage::AssetStorage;
```

- [ ] **Step 6: 테스트 통과 확인**

Run: `cargo test -p voltex_asset`
Expected: handle 2개 + storage 7개 = 9개 PASS

- [ ] **Step 7: 커밋**

```bash
git add Cargo.toml crates/voltex_asset/
git commit -m "feat(asset): add voltex_asset crate with Handle<T> and AssetStorage<T>"
```

---

## Task 2: Assets 중앙 관리자

**Files:**
- Create: `crates/voltex_asset/src/assets.rs`
- Modify: `crates/voltex_asset/src/lib.rs`

Assets는 여러 타입의 AssetStorage를 type-erased로 보관. World처럼 TypeId 기반.

- [ ] **Step 1: assets.rs 작성**

```rust
// crates/voltex_asset/src/assets.rs
use std::any::TypeId;
use std::collections::HashMap;
use crate::handle::Handle;
use crate::storage::{AssetStorage, AssetStorageDyn};

/// 중앙 에셋 관리자. 타입별 AssetStorage를 관리.
pub struct Assets {
    storages: HashMap<TypeId, Box<dyn AssetStorageDyn>>,
}

impl Assets {
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    /// 에셋 추가. 스토리지 자동 등록.
    pub fn insert<T: 'static>(&mut self, asset: T) -> Handle<T> {
        self.storage_mut::<T>().insert(asset)
    }

    /// 핸들로 에셋 참조
    pub fn get<T: 'static>(&self, handle: Handle<T>) -> Option<&T> {
        self.storage::<T>()?.get(handle)
    }

    /// 핸들로 에셋 가변 참조
    pub fn get_mut<T: 'static>(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storage_mut::<T>().get_mut(handle)
    }

    /// 참조 카운트 증가
    pub fn add_ref<T: 'static>(&mut self, handle: Handle<T>) {
        self.storage_mut::<T>().add_ref(handle);
    }

    /// 참조 카운트 감소
    pub fn release<T: 'static>(&mut self, handle: Handle<T>) -> bool {
        self.storage_mut::<T>().release(handle)
    }

    /// 타입별 에셋 수
    pub fn count<T: 'static>(&self) -> usize {
        self.storage::<T>().map(|s| s.len()).unwrap_or(0)
    }

    /// 타입별 스토리지 읽기 참조
    pub fn storage<T: 'static>(&self) -> Option<&AssetStorage<T>> {
        self.storages.get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref())
    }

    /// 타입별 스토리지 가변 참조 (없으면 생성)
    pub fn storage_mut<T: 'static>(&mut self) -> &mut AssetStorage<T> {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(AssetStorage::<T>::new()))
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct MeshData { vertex_count: u32 }

    #[derive(Debug, PartialEq)]
    struct TextureData { width: u32, height: u32 }

    #[test]
    fn test_insert_and_get_different_types() {
        let mut assets = Assets::new();
        let mesh_h = assets.insert(MeshData { vertex_count: 100 });
        let tex_h = assets.insert(TextureData { width: 256, height: 256 });

        assert_eq!(assets.get(mesh_h).unwrap().vertex_count, 100);
        assert_eq!(assets.get(tex_h).unwrap().width, 256);
    }

    #[test]
    fn test_count_per_type() {
        let mut assets = Assets::new();
        assets.insert(MeshData { vertex_count: 10 });
        assets.insert(MeshData { vertex_count: 20 });
        assets.insert(TextureData { width: 64, height: 64 });

        assert_eq!(assets.count::<MeshData>(), 2);
        assert_eq!(assets.count::<TextureData>(), 1);
    }

    #[test]
    fn test_release_through_assets() {
        let mut assets = Assets::new();
        let h = assets.insert(42_i32);
        assert_eq!(assets.count::<i32>(), 1);
        assets.release(h);
        assert_eq!(assets.count::<i32>(), 0);
    }

    #[test]
    fn test_ref_counting_through_assets() {
        let mut assets = Assets::new();
        let h = assets.insert("shared".to_string());
        assets.add_ref(h);
        assert!(!assets.release(h)); // 2 → 1
        assert!(assets.get(h).is_some()); // still alive
        assert!(assets.release(h)); // 1 → 0
        assert!(assets.get(h).is_none());
    }

    #[test]
    fn test_storage_access() {
        let mut assets = Assets::new();
        assets.insert(10_i32);
        assets.insert(20_i32);

        let storage = assets.storage::<i32>().unwrap();
        let values: Vec<i32> = storage.iter().map(|(_, v)| *v).collect();
        assert_eq!(values.len(), 2);
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// crates/voltex_asset/src/lib.rs
pub mod handle;
pub mod storage;
pub mod assets;

pub use handle::Handle;
pub use storage::AssetStorage;
pub use assets::Assets;
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_asset`
Expected: 9 + 5 = 14개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_asset/
git commit -m "feat(asset): add Assets central manager with type-erased storage"
```

---

## Task 3: asset_demo 예제

**Files:**
- Create: `examples/asset_demo/Cargo.toml`
- Create: `examples/asset_demo/src/main.rs`
- Modify: `Cargo.toml` (워크스페이스에 추가)

에셋 매니저를 사용하여 메시와 텍스처를 관리하는 데모. many_cubes를 기반으로 하되, Mesh와 GpuTexture를 Assets에 등록하고 Handle로 참조.

- [ ] **Step 1: 워크스페이스 + Cargo.toml**

workspace members에 `"examples/asset_demo"` 추가.

```toml
# examples/asset_demo/Cargo.toml
[package]
name = "asset_demo"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
voltex_renderer.workspace = true
voltex_ecs.workspace = true
voltex_asset.workspace = true
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
env_logger.workspace = true
log.workspace = true
```

- [ ] **Step 2: main.rs 작성**

이 데모는 many_cubes와 비슷하지만:
1. `Assets` 매니저에 Mesh를 등록: `let mesh_handle = assets.insert(mesh);`
2. ECS 엔티티가 `Handle<Mesh>` 컴포넌트로 에셋을 참조
3. 렌더링 시 `assets.get(mesh_handle)` 로 Mesh를 가져와 사용
4. 참조 카운팅 데모: 타이틀바에 에셋 수 표시
5. R키: 랜덤 엔티티 10개 despawn (에셋은 다른 엔티티가 참조하므로 유지)
6. 단일 큐브 메시를 모든 엔티티가 공유

파일을 작성하기 전에 `examples/many_cubes/src/main.rs`의 dynamic UBO 패턴을 반드시 읽을 것.

핵심 구조:
```rust
struct AppState {
    // ...
    assets: Assets,
    world: World,
    // mesh_handle은 world의 엔티티 컴포넌트로 저장
}

// MeshRef 컴포넌트: Handle<Mesh>를 감싸는 newtype
#[derive(Clone, Copy)]
struct MeshRef(Handle<Mesh>);
```

렌더링 루프:
```rust
// WorldTransform가 있는 엔티티 + MeshRef 쿼리
let entities = state.world.query2::<WorldTransform, MeshRef>();
for (_, wt, mesh_ref) in &entities {
    if let Some(mesh) = state.assets.get(mesh_ref.0) {
        // set vertex/index buffer, draw
    }
}
```

- [ ] **Step 3: 빌드 + 테스트**

Run: `cargo build --workspace`
Run: `cargo test --workspace`

- [ ] **Step 4: 실행 확인**

Run: `cargo run -p asset_demo`
Expected: 큐브 그리드가 렌더링됨. R키로 엔티티 삭제 시 메시 에셋은 유지.

- [ ] **Step 5: 커밋**

```bash
git add Cargo.toml examples/asset_demo/
git commit -m "feat: add asset_demo with Handle-based mesh management"
```

---

## Phase 3c 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과 (기존 80 + asset 14 = 94개)
- [ ] Handle<T>: Copy, Eq, Hash, generation으로 stale 감지
- [ ] AssetStorage<T>: insert/get/release, 참조 카운팅, ID 재사용
- [ ] Assets: 타입별 스토리지 관리, 자동 등록
- [ ] `cargo run -p asset_demo` — 에셋 핸들로 메시 관리, R키 엔티티 삭제
- [ ] 기존 예제 모두 동작
