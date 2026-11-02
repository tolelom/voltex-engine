# voltex_image Crate Split (E1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the wgpu-free asset decoders (PNG/JPG/deflate/OBJ/glTF/JSON) and the `MeshVertex` POD type out of the `voltex_renderer` God-crate into a new L0 crate `voltex_image`, and fold capture's PNG encoder into it to dedup CRC32/Adler32 (resolves audit items E1 + C1).

**Architecture:** `voltex_image` is a new L0 crate (no voltex deps; only `bytemuck` + `thiserror`). All decoders move there along with the `MeshVertex` data struct. The wgpu-specific vertex `LAYOUT` stays in `voltex_renderer` as a free const `MESH_VERTEX_LAYOUT`. `voltex_renderer` re-exports `voltex_image`'s public items so existing paths (`voltex_renderer::parse_png`, `voltex_renderer::vertex::MeshVertex`, `crate::gltf::*`) keep working. `voltex_capture`'s PNG encoder moves into `voltex_image`, sharing a single `checksum` module with the decoder.

**Tech Stack:** Rust 2021, cargo workspace, bytemuck (Pod/Zeroable), thiserror.

---

## Boundary facts (verified)

- Decoder files have **zero** wgpu/`GpuTexture`/`GpuContext` references.
- Only cross-coupling: `obj.rs` and `gltf/{accessor,types}.rs` use `crate::vertex::MeshVertex`; `gltf/accessor.rs` uses `crate::obj::compute_tangents`; `png.rs` uses `crate::deflate`; gltf submodules use `crate::json_parser`. All of these move together as one cluster.
- `animation.rs` (stays in renderer) uses `crate::gltf::{GltfChannel, AnimationPath, Interpolation}` → preserved via re-export.
- `MeshVertex::LAYOUT` is referenced in 7 renderer files (8 sites): `forward_pass.rs`, `deferred_pipeline/gbuffer.rs`, `instancing.rs`, `pbr_pipeline.rs`, `pipeline.rs`, `shadow_pipeline.rs`, `skinned_vertex.rs` (×2).
- External `MeshVertex` consumers (`voltex_capture/scene.rs`, `games/voltex_survivor`, `examples/tutorial_minigame`) only **construct** it — none reference `LAYOUT` — so they need no change (they import via `voltex_renderer::...`).
- `parse_png` external consumer: only `voltex_capture/png_encode.rs` tests (`voltex_renderer::parse_png`).
- Decoders use `crate::Result` (= `RendererError`) and `crate::RendererError::from(...)`.
- gltf uses **no** voltex_math types (raw `f32`/`[[f32;4];4]` arrays) → voltex_image is **L0**.
- capture exposes `voltex_capture::encode_rgba8` (pub) used internally at `lib.rs:34`.

---

## File Structure (target)

```
crates/voltex_image/
  Cargo.toml                 # deps: bytemuck, thiserror
  CODEMAP.md
  src/
    lib.rs                   # module decls + public re-exports, #![warn(missing_docs)]
    error.rs                 # ImageError + Result + From<String>/From<&str>
    vertex.rs                # MeshVertex (data only, Pod/Zeroable) — NO wgpu
    checksum.rs              # crc32, adler32 (shared by png decode tests + encoder) [C1]
    deflate.rs               # inflate (moved)
    png.rs                   # parse_png decoder (moved)
    png_encode.rs            # encode_rgba8 (moved from voltex_capture) [C1]
    jpg/{mod,bit_reader,decoder,headers,idct,mcu}.rs   # moved
    progressive_jpeg.rs      # moved
    obj.rs                   # ObjData, parse_obj, compute_tangents (moved)
    json_parser.rs           # parse_json, JsonValue (moved)
    gltf/{mod,accessor,material,node,parser,types}.rs  # moved
```

`voltex_renderer/src/vertex.rs` keeps `Vertex` (triangle) + `pub use voltex_image::MeshVertex;` + `pub const MESH_VERTEX_LAYOUT`.

---

### Task 1: Scaffold `voltex_image` crate

**Files:**
- Create: `crates/voltex_image/Cargo.toml`
- Create: `crates/voltex_image/src/lib.rs`
- Create: `crates/voltex_image/src/error.rs`
- Modify: `Cargo.toml` (workspace members + workspace.dependencies)

- [ ] **Step 1: Create the crate manifest**

Create `crates/voltex_image/Cargo.toml`:

```toml
[package]
name = "voltex_image"
version = "0.1.0"
edition = "2021"
license.workspace = true

[dependencies]
bytemuck.workspace = true
thiserror = { workspace = true }
```

- [ ] **Step 2: Create the error type** (mirrors the M4 standard pattern)

Create `crates/voltex_image/src/error.rs`:

```rust
//! Crate-wide error type and `Result` alias for `voltex_image`.

/// All errors produced by `voltex_image` (PNG/JPG/glTF/OBJ/JSON parsing,
/// deflate inflation, PNG encoding).
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    /// String 메시지를 그대로 보존하는 generic variant (마이그레이션 호환).
    #[error("{0}")]
    Generic(String),

    /// IO 에러.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<String> for ImageError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for ImageError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

/// Type alias for `Result<T, ImageError>`.
pub type Result<T> = std::result::Result<T, ImageError>;
```

- [ ] **Step 3: Create the lib skeleton**

Create `crates/voltex_image/src/lib.rs`:

```rust
//! # voltex_image
//!
//! Dependency-free (no GPU) asset codecs for the Voltex engine: PNG/JPG
//! decoders, a minimal PNG encoder, raw DEFLATE inflation, OBJ + glTF mesh
//! parsing, and a small JSON parser. Decoders return CPU pixel/geometry
//! buffers; GPU upload stays in `voltex_renderer`.
//!
//! This is an L0 crate (no `voltex_*` dependencies) so the codecs can be
//! reused without pulling in wgpu.

#![warn(missing_docs)]

pub mod error;
pub use error::{ImageError, Result};
```

- [ ] **Step 4: Register the crate in the workspace**

In root `Cargo.toml`, add `"crates/voltex_image",` to `members` (after the `"crates/voltex_renderer",` line is fine), and add to `[workspace.dependencies]` (after the `voltex_renderer_ecs` line):

```toml
voltex_image = { path = "crates/voltex_image" }
```

- [ ] **Step 5: Verify it builds**

Run: `cargo build -p voltex_image`
Expected: compiles clean (only `error` module).

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_image Cargo.toml
git commit -m "feat(image): scaffold voltex_image L0 crate (E1 step 1)"
```

---

### Task 2: Move the decoder cluster + MeshVertex + checksums into `voltex_image`

This moves the connected decoder cluster as one unit (they are mutually dependent). The moved tests come with the files, preserving coverage. `voltex_capture` still keeps its own `png_encode.rs` copy until Task 4, so the workspace stays green (temporary duplication only).

**Files:**
- Create: `crates/voltex_image/src/vertex.rs`
- Create: `crates/voltex_image/src/checksum.rs`
- Copy → `crates/voltex_image/src/`: `deflate.rs`, `png.rs`, `progressive_jpeg.rs`, `obj.rs`, `json_parser.rs`, `jpg/` (dir), `gltf/` (dir)
- Copy: `crates/voltex_capture/src/png_encode.rs` → `crates/voltex_image/src/png_encode.rs`
- Modify: `crates/voltex_image/src/lib.rs`

- [ ] **Step 1: Copy the decoder files from renderer into voltex_image**

Run (PowerShell, from repo root):

```powershell
$src = "crates\voltex_renderer\src"
$dst = "crates\voltex_image\src"
Copy-Item "$src\deflate.rs"          "$dst\deflate.rs"
Copy-Item "$src\png.rs"              "$dst\png.rs"
Copy-Item "$src\progressive_jpeg.rs" "$dst\progressive_jpeg.rs"
Copy-Item "$src\obj.rs"              "$dst\obj.rs"
Copy-Item "$src\json_parser.rs"      "$dst\json_parser.rs"
Copy-Item "$src\jpg"                 "$dst\jpg"  -Recurse
Copy-Item "$src\gltf"                "$dst\gltf" -Recurse
Copy-Item "crates\voltex_capture\src\png_encode.rs" "$dst\png_encode.rs"
```

- [ ] **Step 2: Create the data-only `MeshVertex`** (no wgpu, no `LAYOUT`)

Create `crates/voltex_image/src/vertex.rs`:

```rust
//! PBR mesh vertex — the geometry layout produced by the OBJ/glTF decoders.
//!
//! This is a plain POD struct with no GPU coupling. The wgpu vertex buffer
//! layout for it lives in `voltex_renderer` (`MESH_VERTEX_LAYOUT`).

use bytemuck::{Pod, Zeroable};

/// PBR-ready mesh vertex used by all the engine's mesh pipelines.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    /// Local-space position.
    pub position: [f32; 3],
    /// Surface normal (unit vector).
    pub normal: [f32; 3],
    /// UV coordinates `[u, v]`.
    pub uv: [f32; 2],
    /// Tangent vector + handedness `[tx, ty, tz, w]` for normal mapping.
    pub tangent: [f32; 4],
}
```

- [ ] **Step 3: Create the shared checksum module** (consolidates CRC32/Adler32 — C1)

Create `crates/voltex_image/src/checksum.rs`:

```rust
//! Checksums shared by the PNG decoder tests and the PNG encoder.

/// CRC-32 (ISO 3309, as used by PNG chunk checksums) of `bytes`.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Adler-32 (as used by the zlib stream wrapping the deflate data).
pub(crate) fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in bytes {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}
```

- [ ] **Step 4: Repoint the PNG encoder to the shared checksums**

In `crates/voltex_image/src/png_encode.rs`:
1. Delete the local `fn crc32(bytes: &[u8]) -> u32 { ... }` definition (≈ lines 8–19) and the local `fn adler32(bytes: &[u8]) -> u32 { ... }` definition (≈ lines 21–31).
2. Add at the top of the file (below the `//!` header):

```rust
use crate::checksum::{adler32, crc32};
```

3. Update the doc comment reference: change `//! fully valid PNG: it round-trips through [`voltex_renderer::parse_png`].` to `//! fully valid PNG: it round-trips through [`crate::parse_png`].`
4. In the test module, change `voltex_renderer::parse_png(&png)` (2 occurrences) to `crate::parse_png(&png)`.

- [ ] **Step 5: Repoint the PNG decoder test checksums to the shared module**

In `crates/voltex_image/src/png.rs`, the `#[cfg(test)] mod tests` defines local `fn crc32(chunk_type: &[u8], data: &[u8]) -> u32` and `fn adler32(data: &[u8]) -> u32`. Replace both local definitions with a use of the shared module and adapt call sites:
1. Delete the two local `fn crc32(...)` / `fn adler32(...)` definitions inside the test module.
2. Add `use crate::checksum::{crc32 as crc32_raw, adler32};` inside the test module (below `use super::*;`).
3. Replace the call `crc32(chunk_type, data)` with `crc32_raw(&[chunk_type, data].concat())`.
4. Replace `adler32(data)` call as-is (signature matches).

- [ ] **Step 6: Migrate error references in the moved files**

The moved decoders reference `crate::RendererError`. Replace every occurrence with `crate::ImageError` across the copied files. Run:

```powershell
Get-ChildItem -Recurse "crates\voltex_image\src" -Filter *.rs |
  ForEach-Object {
    (Get-Content $_.FullName -Raw) -replace 'crate::RendererError', 'crate::ImageError' |
      Set-Content $_.FullName -NoNewline
  }
```

Then verify none remain:

Run: `Select-String -Path crates\voltex_image\src\*.rs, crates\voltex_image\src\**\*.rs -Pattern "RendererError"`
Expected: no matches.

(`crate::Result` references need no change — they now resolve to `voltex_image::Result`.)

- [ ] **Step 7: Wire up `voltex_image/src/lib.rs`**

Replace `crates/voltex_image/src/lib.rs` body (keep the `//!` header + `#![warn(missing_docs)]`) with:

```rust
#![warn(missing_docs)]

pub mod error;
pub mod vertex;
mod checksum;
pub mod deflate;
pub mod png;
pub mod png_encode;
pub mod jpg;
pub mod progressive_jpeg;
pub mod obj;
pub mod json_parser;
pub mod gltf;

pub use error::{ImageError, Result};
pub use vertex::MeshVertex;
pub use png::parse_png;
pub use png_encode::encode_rgba8;
pub use jpg::parse_jpg;
pub use gltf::{parse_gltf, GltfData, GltfMesh, GltfMaterial};
```

(Keep the original `//!` module doc comment at the top.)

- [ ] **Step 8: Build and test the new crate standalone**

Run: `cargo test -p voltex_image`
Expected: compiles; all moved decoder/encoder tests pass (PNG/JPG/glTF/OBJ fuzz + round-trip tests, encode_rgba8 round-trip).

If `compute_tangents` or other `pub(crate)`/`pub` visibility errors appear, ensure the items keep the same visibility they had in renderer (they were `pub`).

- [ ] **Step 9: Commit**

```bash
git add crates/voltex_image
git commit -m "feat(image): move PNG/JPG/glTF/OBJ/JSON decoders + MeshVertex + PNG encoder into voltex_image (E1 step 2, C1)"
```

---

### Task 3: Make `voltex_renderer` consume `voltex_image`

Delete the now-duplicated decoders from renderer and re-export from `voltex_image`. Keep `Vertex` and add `MESH_VERTEX_LAYOUT`.

**Files:**
- Modify: `crates/voltex_renderer/Cargo.toml`
- Modify: `crates/voltex_renderer/src/error.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`
- Modify: `crates/voltex_renderer/src/vertex.rs`
- Modify (LAYOUT call sites): `forward_pass.rs`, `deferred_pipeline/gbuffer.rs`, `instancing.rs`, `pbr_pipeline.rs`, `pipeline.rs`, `shadow_pipeline.rs`, `skinned_vertex.rs`
- Delete: `deflate.rs`, `png.rs`, `progressive_jpeg.rs`, `obj.rs`, `json_parser.rs`, `jpg/`, `gltf/`

- [ ] **Step 1: Add the dependency**

In `crates/voltex_renderer/Cargo.toml`, add under `[dependencies]` (after `voltex_platform.workspace = true`):

```toml
voltex_image.workspace = true
```

- [ ] **Step 2: Absorb `ImageError` into `RendererError`**

In `crates/voltex_renderer/src/error.rs`, add a variant to the `RendererError` enum (after the `Io` variant):

```rust
    /// 이미지/메시 디코더(voltex_image) 에러.
    #[error(transparent)]
    Image(#[from] voltex_image::ImageError),
```

- [ ] **Step 3: Rewrite `vertex.rs` to re-export `MeshVertex` + keep the wgpu layout**

Replace `crates/voltex_renderer/src/vertex.rs` entirely with:

```rust
//! Vertex types used by the renderer's GPU pipelines.

use bytemuck::{Pod, Zeroable};

/// PBR mesh vertex (POD geometry) — re-exported from `voltex_image`.
pub use voltex_image::MeshVertex;

/// Position + color vertex used by the triangle example pipeline.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    /// World-space position `[x, y, z]`.
    pub position: [f32; 3],
    /// RGB color `[r, g, b]` in linear space.
    pub color: [f32; 3],
}

impl Vertex {
    /// wgpu vertex buffer layout for [`Vertex`].
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
        ],
    };
}

/// wgpu vertex buffer layout for [`MeshVertex`].
///
/// `MeshVertex` itself lives in `voltex_image` (GPU-free); this const keeps
/// the wgpu layout in the renderer where wgpu is a dependency.
pub const MESH_VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: 12,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: 24,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
    ],
};
```

- [ ] **Step 4: Delete the moved decoder files from renderer**

```powershell
$s = "crates\voltex_renderer\src"
Remove-Item "$s\deflate.rs","$s\png.rs","$s\progressive_jpeg.rs","$s\obj.rs","$s\json_parser.rs"
Remove-Item "$s\jpg" -Recurse
Remove-Item "$s\gltf" -Recurse
```

- [ ] **Step 5: Update `lib.rs` module decls + re-exports**

In `crates/voltex_renderer/src/lib.rs`:
1. Delete these `pub mod` lines: `pub mod json_parser;`, `pub mod gltf;`, `pub mod deflate;`, `pub mod png;`, `pub mod jpg;`, `pub mod progressive_jpeg;`, and `pub mod obj;`.
2. Add a re-export block (place it right after `pub use error::{RendererError, Result};`):

```rust
// Asset decoders now live in the GPU-free `voltex_image` crate. Re-export
// them so existing paths (`voltex_renderer::parse_png`, `crate::gltf::*`,
// etc.) keep working.
pub use voltex_image::{deflate, gltf, jpg, json_parser, obj, png, progressive_jpeg};
```

3. Replace the old top-level decoder re-exports (the three lines `pub use png::parse_png;`, `pub use jpg::parse_jpg;`, `pub use gltf::{parse_gltf, GltfData, GltfMesh, GltfMaterial};`) with:

```rust
pub use voltex_image::{parse_gltf, parse_jpg, parse_png, GltfData, GltfMaterial, GltfMesh};
```

4. Leave `pub use vertex::{Vertex, MeshVertex};` (line ~71) unchanged, and add right after it:

```rust
pub use vertex::MESH_VERTEX_LAYOUT;
```

- [ ] **Step 6: Fix the 8 `MeshVertex::LAYOUT` call sites**

In each of these files, replace `MeshVertex::LAYOUT` with `MESH_VERTEX_LAYOUT` and ensure it is in scope. For each file, if it has `use crate::vertex::MeshVertex;` (or imports `MeshVertex` from crate root), add `MESH_VERTEX_LAYOUT` to that import; otherwise add `use crate::vertex::MESH_VERTEX_LAYOUT;`.

Files (occurrence count):
- `crates/voltex_renderer/src/forward_pass.rs` (1)
- `crates/voltex_renderer/src/deferred_pipeline/gbuffer.rs` (1)
- `crates/voltex_renderer/src/instancing.rs` (1)
- `crates/voltex_renderer/src/pbr_pipeline.rs` (1)
- `crates/voltex_renderer/src/pipeline.rs` (1)
- `crates/voltex_renderer/src/shadow_pipeline.rs` (1)
- `crates/voltex_renderer/src/skinned_vertex.rs` (2)

For `skinned_vertex.rs`: it builds `SkinnedMeshVertex::LAYOUT` by referencing `MeshVertex::LAYOUT`'s attributes/stride. Replace both `MeshVertex::LAYOUT` references with `crate::vertex::MESH_VERTEX_LAYOUT`.

- [ ] **Step 7: Build the renderer and the workspace**

Run: `cargo build -p voltex_renderer`
Expected: compiles. If a call site still says `MeshVertex::LAYOUT`, the error `no associated item named LAYOUT found for struct MeshVertex` points to the remaining site — fix it.

Run: `cargo build --workspace`
Expected: compiles (capture still uses its own `png_encode.rs` — fine).

- [ ] **Step 8: Test renderer**

Run: `cargo test -p voltex_renderer --lib`
Expected: pass. (Decoder tests now live in voltex_image; renderer's remaining tests — animation/skinning/pipelines — still pass.)

- [ ] **Step 9: Commit**

```bash
git add crates/voltex_renderer
git commit -m "refactor(renderer): consume voltex_image; re-export decoders, MeshVertex via crate boundary (E1 step 3)"
```

---

### Task 4: Fold capture's PNG encoder into `voltex_image` (C1 dedup)

The encoder already lives in `voltex_image` (copied in Task 2). Now delete capture's copy and re-point.

**Files:**
- Modify: `crates/voltex_capture/Cargo.toml`
- Modify: `crates/voltex_capture/src/lib.rs`
- Delete: `crates/voltex_capture/src/png_encode.rs`

- [ ] **Step 1: Add the dependency**

In `crates/voltex_capture/Cargo.toml`, add under `[dependencies]` (after `voltex_renderer.workspace = true`):

```toml
voltex_image.workspace = true
```

- [ ] **Step 2: Re-point capture to the shared encoder**

In `crates/voltex_capture/src/lib.rs`:
1. Delete the line `mod png_encode;` (line ~20).
2. Replace `pub use png_encode::encode_rgba8;` (line ~29) with `pub use voltex_image::encode_rgba8;`.

The internal caller `let png = encode_rgba8(width, height, rgba);` (line ~34) is unchanged.

- [ ] **Step 3: Delete capture's encoder file**

```powershell
Remove-Item "crates\voltex_capture\src\png_encode.rs"
```

- [ ] **Step 4: Build and test capture + image**

Run: `cargo test -p voltex_capture -p voltex_image`
Expected: compiles; tests pass. `voltex_capture::encode_rgba8` still resolves (re-exported from voltex_image).

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_capture
git commit -m "refactor(capture): use voltex_image::encode_rgba8, drop duplicate CRC32/Adler32 (C1)"
```

---

### Task 5: Facade, docs, CODEMAPs, and final workspace verification

**Files:**
- Modify: `crates/voltex/Cargo.toml`
- Modify: `crates/voltex/src/lib.rs`
- Create: `crates/voltex_image/CODEMAP.md`
- Modify: `crates/voltex_renderer/CODEMAP.md`
- Modify: `crates/voltex_capture/CODEMAP.md`
- Modify: `CLAUDE.md`
- Modify: `docs/dev/AUDIT-BACKLOG.md`
- Modify: `docs/dev/STATUS.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add `voltex_image` to the facade crate**

In `crates/voltex/Cargo.toml`, add `voltex_image.workspace = true` under `[dependencies]`.

In `crates/voltex/src/lib.rs`, add the re-export in alphabetical position (between `pub use voltex_ecs as ecs;` group and `pub use voltex_math as math;` — place after the `editor` line):

```rust
pub use voltex_image as image;
```

- [ ] **Step 2: Create `voltex_image` CODEMAP**

Create `crates/voltex_image/CODEMAP.md` documenting: responsibility (GPU-free asset codecs), public API (`parse_png`, `parse_jpg`, `parse_gltf`/`GltfData`/`GltfMesh`/`GltfMaterial`, `parse_obj`/`ObjData`/`compute_tangents`, `parse_json`/`JsonValue`, `deflate::inflate`, `encode_rgba8`, `MeshVertex`, `ImageError`/`Result`), dependencies (bytemuck, thiserror — L0), and the boundary note: "decoders return CPU buffers; the wgpu `MESH_VERTEX_LAYOUT` for `MeshVertex` lives in voltex_renderer."

- [ ] **Step 3: Update renderer + capture CODEMAPs**

- `crates/voltex_renderer/CODEMAP.md`: note that PNG/JPG/glTF/OBJ/JSON decoders + `MeshVertex` moved to `voltex_image` (re-exported for compat); renderer adds `MESH_VERTEX_LAYOUT` (wgpu layout) and depends on `voltex_image`.
- `crates/voltex_capture/CODEMAP.md`: change the `encode_rgba8` line to note it now comes from `voltex_image` (re-exported), CRC32/Adler32 dedup done.

- [ ] **Step 4: Update `CLAUDE.md` dependency graph**

In the "Crate 의존성 그래프" section, add `voltex_image` under L0:

```
L0 (no voltex deps):
  voltex_math       — Vec/Mat/AABB/Ray
  voltex_asset      — Handle, AssetStorage, AssetLoader, FileWatcher
  voltex_net        — UDP, Packet, Reliable, Snapshot, Interpolation
  voltex_image      — PNG/JPG/glTF/OBJ/JSON 디코더 + PNG 인코더 + MeshVertex (GPU 무의존)
```

Update the `voltex_renderer` line to show it now depends on `voltex_image`, and add `voltex_image` to `voltex_capture`'s dep list. Also update the facade note (voltex re-exports `image`).

- [ ] **Step 5: Mark audit items done**

In `docs/dev/AUDIT-BACKLOG.md`:
- Change `- [ ] **E1**` to `- [x] **E1**` and append the commit hashes.
- Change `- [~] **C1**` to `- [x] **C1**` noting it was resolved as part of E1 (checksums consolidated in `voltex_image::checksum`).

- [ ] **Step 6: Update STATUS + CHANGELOG**

In `docs/dev/STATUS.md`: add `voltex_image` to the crate list/structure and note the renderer God-crate reduction. In `CHANGELOG.md`: add an entry describing the E1 split + C1 dedup.

- [ ] **Step 7: Full workspace verification**

Run: `cargo build --workspace`
Expected: clean build.

Run: `cargo test --workspace --lib`
Expected: all tests pass.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings (watch for `missing_docs` in the new crate — every pub item has a doc).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "docs(image): facade re-export, CODEMAPs, dep graph; mark E1+C1 done"
```

---

## Self-Review notes

- **Spec coverage:** E1 (crate split) = Tasks 1–3 + 5; C1 (CRC32/Adler32 dedup) = `checksum.rs` (Task 2) + capture re-point (Task 4). Both audit items closed in Task 5.
- **Compat preserved:** `voltex_renderer::parse_png` (Task 3 step 5), `voltex_renderer::vertex::MeshVertex` (Task 3 step 3 re-export + lib `pub use vertex::{Vertex, MeshVertex}`), `crate::gltf::{GltfChannel,...}` for animation.rs (Task 3 step 5 module re-export), `voltex_capture::encode_rgba8` (Task 4 step 2). External `MeshVertex` constructors unaffected.
- **Type consistency:** `MESH_VERTEX_LAYOUT` defined once in `vertex.rs` (Task 3 step 3), referenced in step 6 + lib re-export step 5.4. `ImageError`/`Result` defined Task 1, consumed via `#[from]` Task 3 step 2 and via `crate::` migration Task 2 step 6. `crc32`/`adler32` signatures: canonical single-slice in `checksum.rs`; png decoder test adapts via `.concat()` (Task 2 step 5).
- **Green per commit:** Task 2 leaves a temporary duplicate (`png_encode` in both capture + image) but compiles; Task 3 removes renderer dup; Task 4 removes capture dup.
