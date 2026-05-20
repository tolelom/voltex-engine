# M2: 800줄+ 파일 분리 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 800줄+인 4개 파일을 모듈 디렉토리로 분리하여 모든 `.rs` 파일이 600줄 이하가 되도록 한다. 공개 API 시그니처 무변경.

**Architecture:** 각 큰 파일 `foo.rs`를 `foo/` 디렉토리 + `mod.rs`로 변환. `mod.rs`에서 sub-module들을 합쳐 재노출(`pub use`)하여 외부 호출자는 무수정. private 함수는 `pub(super)` 또는 `pub(crate)` 적절히 사용.

**Tech Stack:** Rust (cargo, rustfmt). 검증은 `cargo build --workspace` + `cargo test --workspace --lib` + 파일 줄 수 측정.

**Spec reference:** `docs/superpowers/specs/2026-05-20-engine-restructure-design.md` Layer 2, 마일스톤 M2.

---

## 분리 전 파일 줄 수 (현재)

| 파일 | 줄 수 | 분리 후 디렉토리 |
|---|---|---|
| `crates/voltex_audio/src/vorbis.rs` | 1493 | `crates/voltex_audio/src/vorbis/` |
| `crates/voltex_renderer/src/gltf.rs` | 982 | `crates/voltex_renderer/src/gltf/` |
| `crates/voltex_renderer/src/jpg.rs` | 959 | `crates/voltex_renderer/src/jpg/` |
| `crates/voltex_renderer/src/deferred_pipeline.rs` | 905 | `crates/voltex_renderer/src/deferred_pipeline/` |

## 공통 분리 원칙

1. 새 디렉토리 `crates/<crate>/src/<name>/` 생성
2. `mod.rs` 작성:
   - 각 sub-module 선언 (`mod sub_a;` 등 — 외부 노출 불필요한 경우 `pub`은 빼고, 필요시 `pub(super)`)
   - 공개 API 재노출 (`pub use sub_a::PublicType;`)
   - 통합이 필요한 경우 thin wrapper 함수 정의 (드물게 필요)
3. 원본 `<name>.rs`의 코드를 의미적 단위로 sub-module 파일에 이동
   - 다른 sub-module이 참조하는 항목은 `pub(super)` 또는 `pub(crate)`로 변경
   - 같은 sub-module 내부 항목은 private 유지
   - 외부 공개 API는 `pub` 유지
4. 원본 `<name>.rs` 파일 삭제 (`mod.rs`가 대체)
5. 테스트는 각 sub-module 안의 `#[cfg(test)] mod tests` 유지 (테스트가 분리되는 코드와 함께 이동)
6. 빌드 + 테스트 + 줄 수 검증 후 commit

**중요**: 외부 호출자(다른 crate, examples) 입장에서 `voltex_<crate>::<symbol>` 경로가 동일해야 함. `lib.rs`의 `pub use <name>::*` 또는 `pub mod <name>;` 선언은 유지.

---

## Task 1: vorbis.rs 분리

**현재 위치**: `crates/voltex_audio/src/vorbis.rs` (1493줄)
**목표 구조**: `crates/voltex_audio/src/vorbis/` 디렉토리, 모든 파일 600줄 이하.

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod vorbis;
```

### 모듈 매핑

| 새 파일 | 원본 line 범위 | 포함 항목 |
|---|---|---|
| `vorbis/mod.rs` | 1302-1346 + 1347-끝 | `decode_vorbis`(공개), tests, sub-module 선언, re-exports |
| `vorbis/bit_reader.rs` | 21-140, 337-351 | `BitReader`, `ilog`, `float32_unpack`, `lookup1_values`, `bit_reverse` |
| `vorbis/codebook.rs` | 141-336 | `Codebook`, `decode_codebook` |
| `vorbis/floor.rs` | 352-431, 788-930 | `Floor1Config`, `decode_floor1_config`, `decode_floor1_packet`, `floor1_inverse_db` |
| `vorbis/residue.rs` | 432-487, 931-1086 | `ResidueConfig`, `decode_residue_config`, `decode_residue` |
| `vorbis/headers.rs` | 488-758 | `Mapping`, `decode_mapping`, `Mode`, `decode_mode`, `VorbisIdHeader`, `parse_id_header`, `VorbisSetup`, `parse_setup_header` |
| `vorbis/imdct.rs` | 759-787 | `imdct`, `vorbis_window` |
| `vorbis/decoder.rs` | 1087-1301 | `decode_audio_packet` |

예상 줄 수: 각 파일 200~450줄.

- [ ] **Step 1: 디렉토리 생성 + mod.rs 작성**

```bash
mkdir -p crates/voltex_audio/src/vorbis
```

`crates/voltex_audio/src/vorbis/mod.rs`:

```rust
mod bit_reader;
mod codebook;
mod floor;
mod residue;
mod headers;
mod imdct;
mod decoder;

use crate::audio_clip::AudioClip;
use decoder::decode_audio_packet;
use headers::{parse_id_header, parse_setup_header};

/// OGG Vorbis 패킷 시퀀스를 디코드하여 AudioClip 반환.
pub fn decode_vorbis(packets: &[Vec<u8>]) -> Result<AudioClip, String> {
    // 원본 vorbis.rs line 1302-1346 코드를 그대로 이동
    // (실제 구현은 Step 8에서 원본 line을 그대로 옮기되, 함수 호출 경로를
    //  parse_id_header → headers::parse_id_header 등으로 조정)
}

#[cfg(test)]
mod tests {
    // 원본 vorbis.rs의 #[cfg(test)] 블록 line 1347-end를 그대로 이동
    // sub-module의 항목을 use하도록 조정
}
```

**참고**: Step 1에서는 mod.rs를 위 형태로 placeholder 만들고, 실제 함수 본문 코드는 sub-module이 다 만들어진 후 Step 8에서 채움.

- [ ] **Step 2: bit_reader.rs 작성**

`crates/voltex_audio/src/vorbis/bit_reader.rs`:

원본 `vorbis.rs` line 21-140 (BitReader struct + impl + ilog/float32_unpack/lookup1_values 헬퍼) + line 337-351 (bit_reverse) 를 옮긴다.

- 각 항목 `pub(super)` 가시성으로 변경 (다른 sub-module이 사용해야 하므로)
- BitReader는 `pub(super) struct BitReader<'a>` + 모든 메서드 `pub(super) fn`
- 헬퍼 함수 `ilog`, `float32_unpack`, `lookup1_values`, `bit_reverse`는 `pub(super) fn`

- [ ] **Step 3: codebook.rs 작성**

`crates/voltex_audio/src/vorbis/codebook.rs`:

원본 line 141-336 (Codebook struct + impl + decode_codebook) 을 옮긴다.

상단:
```rust
use super::bit_reader::{BitReader, ilog, float32_unpack, lookup1_values};
```

- `Codebook` struct: `pub(super)` (다른 모듈에서 참조)
- `Codebook` impl 메서드: 외부 호출되는 것은 `pub(super)`, 내부 헬퍼는 private
- `decode_codebook`: `pub(super) fn`

- [ ] **Step 4: floor.rs 작성**

`crates/voltex_audio/src/vorbis/floor.rs`:

원본 line 352-431 (Floor1Config + decode_floor1_config) + line 788-912 (decode_floor1_packet) + line 913-930 (floor1_inverse_db) 을 옮긴다.

상단:
```rust
use super::bit_reader::{BitReader, bit_reverse};
use super::codebook::Codebook;
```

- `Floor1Config`: `pub(super)`
- `decode_floor1_config`, `decode_floor1_packet`, `floor1_inverse_db`: `pub(super) fn`

- [ ] **Step 5: residue.rs 작성**

`crates/voltex_audio/src/vorbis/residue.rs`:

원본 line 432-487 (ResidueConfig + decode_residue_config) + line 931-1086 (decode_residue) 을 옮긴다.

상단:
```rust
use super::bit_reader::BitReader;
use super::codebook::Codebook;
```

- `ResidueConfig`: `pub(super)`
- `decode_residue_config`, `decode_residue`: `pub(super) fn`

- [ ] **Step 6: headers.rs 작성**

`crates/voltex_audio/src/vorbis/headers.rs`:

원본 line 488-758 을 옮긴다:
- `Mapping`, `decode_mapping`
- `Mode`, `decode_mode`
- `VorbisIdHeader`, `parse_id_header`
- `VorbisSetup`, `parse_setup_header`

상단:
```rust
use super::bit_reader::BitReader;
use super::codebook::{Codebook, decode_codebook};
use super::floor::{Floor1Config, decode_floor1_config};
use super::residue::{ResidueConfig, decode_residue_config};
```

- 모든 struct: `pub(super)` (단, `VorbisIdHeader`와 `VorbisSetup`은 decoder.rs가 사용)
- 모든 fn: `pub(super) fn`

- [ ] **Step 7: imdct.rs 작성**

`crates/voltex_audio/src/vorbis/imdct.rs`:

원본 line 759-787 (imdct, vorbis_window) 을 옮긴다. 외부 의존 없음.

- `imdct`, `vorbis_window`: `pub(super) fn`

- [ ] **Step 8: decoder.rs 작성 + mod.rs 본문 완성**

`crates/voltex_audio/src/vorbis/decoder.rs`:

원본 line 1087-1301 (decode_audio_packet) 을 옮긴다.

상단:
```rust
use super::headers::{VorbisIdHeader, VorbisSetup, Mode};
use super::floor::decode_floor1_packet;
use super::residue::decode_residue;
use super::imdct::{imdct, vorbis_window};
use super::bit_reader::BitReader;
```

- `decode_audio_packet`: `pub(super) fn`

`crates/voltex_audio/src/vorbis/mod.rs` 본문 완성:

원본 line 1302-1346의 `pub fn decode_vorbis` 본문을 mod.rs에 옮기되, 내부에서 호출하는 함수들의 경로를 sub-module로 조정 (`parse_id_header`, `parse_setup_header`, `decode_audio_packet` 등은 위에서 use로 가져옴).

원본 line 1347-끝의 `#[cfg(test)] mod tests`를 mod.rs 끝에 옮기되, 테스트가 참조하는 사항(예: 디코더 출력)을 super:: 또는 sub-module use로 조정.

- [ ] **Step 9: 원본 vorbis.rs 삭제**

```bash
rm crates/voltex_audio/src/vorbis.rs
```

- [ ] **Step 10: 빌드 + 테스트 + 줄 수 검증**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p voltex_audio 2>&1 | tail -5
cargo test -p voltex_audio --lib 2>&1 | grep "test result"
```

Expected:
- 빌드 성공
- 테스트 84개 통과

```bash
find crates/voltex_audio/src/vorbis -name "*.rs" -exec wc -l {} \;
```

Expected: 모든 파일 600줄 이하.

```bash
cargo build --workspace 2>&1 | tail -3
cargo test --workspace --lib 2>&1 | grep -E "^test result" | awk '{sum += $4} END {print "TOTAL:", sum}'
```

Expected: 빌드 성공, TOTAL: 760.

- [ ] **Step 11: Commit**

```bash
git add crates/voltex_audio/src/vorbis/ crates/voltex_audio/src/vorbis.rs
git commit -m "refactor(audio): split vorbis.rs (1493 lines) into module directory"
```

---

## Task 2: gltf.rs 분리

**현재 위치**: `crates/voltex_renderer/src/gltf.rs` (982줄)
**목표 구조**: `crates/voltex_renderer/src/gltf/`

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod gltf;
pub use gltf::{parse_gltf, GltfData, GltfMesh, GltfMaterial};
```

### 모듈 매핑

| 새 파일 | 원본 line 범위 | 포함 항목 |
|---|---|---|
| `gltf/mod.rs` | 1-92 + tests | `pub use` re-exports, sub-mod 선언, tests |
| `gltf/types.rs` | 5-91 | `GltfData`, `GltfMesh`, `GltfMaterial`, `GltfNode`, `GltfSkin`, `GltfAnimation`, `GltfChannel`, `AnimationPath`, `Interpolation`, `parse_animation_path`, `parse_interpolation` |
| `gltf/parser.rs` | 93-220 | GLB 상수, `parse_gltf`, `parse_glb`, `parse_gltf_json`, `decode_base64` |
| `gltf/accessor.rs` | 221-509 + 660-675 | `extract_meshes`, `get_buffer_data`, `read_accessor_*`, `read_floats`, `read_indices_u16` |
| `gltf/node.rs` | 510-640 | `parse_nodes`, `parse_skins`, `parse_animations` |
| `gltf/material.rs` | 641-659 | `extract_material` |

예상 줄 수: 각 파일 100~290줄.

- [ ] **Step 1: 디렉토리 생성 + mod.rs**

```bash
mkdir -p crates/voltex_renderer/src/gltf
```

`crates/voltex_renderer/src/gltf/mod.rs`:

```rust
mod types;
mod parser;
mod accessor;
mod node;
mod material;

pub use types::{
    GltfData, GltfMesh, GltfMaterial, GltfNode, GltfSkin, GltfAnimation,
    GltfChannel, AnimationPath, Interpolation, parse_animation_path, parse_interpolation,
};
pub use parser::parse_gltf;

#[cfg(test)]
mod tests {
    // 원본 gltf.rs의 #[cfg(test)] 블록 line 676-end를 그대로 이동
    // use super::*; 또는 use super::{parse_gltf, GltfData}; 등 적절히 조정
}
```

- [ ] **Step 2: types.rs 작성**

`crates/voltex_renderer/src/gltf/types.rs`:

원본 line 5-91 (모든 struct/enum + parse_animation_path, parse_interpolation) 을 옮긴다.

- 모든 `pub struct`/`pub enum`/`pub fn` 가시성 유지
- 외부 의존 없음 (순수 데이터)

- [ ] **Step 3: parser.rs 작성**

`crates/voltex_renderer/src/gltf/parser.rs`:

원본 line 93-220 (GLB 상수, parse_gltf, parse_glb, parse_gltf_json, decode_base64) 을 옮긴다.

상단:
```rust
use super::types::GltfData;
use super::accessor::extract_meshes;
use crate::json_parser::JsonValue;  // 원본에서 사용 중인 의존성
```

- `parse_gltf`: `pub fn` (외부 노출 — `mod.rs`에서 `pub use`)
- 나머지: 모듈 내부 가시성 (`fn` 또는 `pub(super) fn`)
- `GLB_MAGIC`, `GLB_VERSION`, `CHUNK_JSON`, `CHUNK_BIN`: `const` (private)

- [ ] **Step 4: accessor.rs 작성**

`crates/voltex_renderer/src/gltf/accessor.rs`:

원본 line 221-509 + 660-675 (extract_meshes, get_buffer_data, read_accessor_*, read_floats, read_indices_u16) 를 옮긴다.

상단:
```rust
use super::types::{GltfData, GltfMesh, GltfMaterial, GltfNode, GltfSkin, GltfAnimation};
use super::node::{parse_nodes, parse_skins, parse_animations};
use super::material::extract_material;
use crate::json_parser::JsonValue;
```

- `extract_meshes`: `pub(super) fn`
- 나머지 헬퍼: `pub(super) fn` 또는 private

- [ ] **Step 5: node.rs 작성**

`crates/voltex_renderer/src/gltf/node.rs`:

원본 line 510-640 (parse_nodes, parse_skins, parse_animations) 을 옮긴다.

상단:
```rust
use super::types::{GltfNode, GltfSkin, GltfAnimation, GltfChannel, parse_animation_path, parse_interpolation};
use crate::json_parser::JsonValue;
```

- 모두 `pub(super) fn`

- [ ] **Step 6: material.rs 작성**

`crates/voltex_renderer/src/gltf/material.rs`:

원본 line 641-659 (extract_material) 을 옮긴다.

상단:
```rust
use super::types::GltfMaterial;
use crate::json_parser::JsonValue;
```

- `extract_material`: `pub(super) fn`

- [ ] **Step 7: 원본 gltf.rs 삭제**

```bash
rm crates/voltex_renderer/src/gltf.rs
```

- [ ] **Step 8: 빌드 + 테스트 + 줄 수 검증**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p voltex_renderer 2>&1 | tail -5
cargo test -p voltex_renderer --lib 2>&1 | grep "test result"
find crates/voltex_renderer/src/gltf -name "*.rs" -exec wc -l {} \;
```

Expected: 빌드 성공, 200개 테스트 통과, 모든 파일 600줄 이하.

- [ ] **Step 9: Commit**

```bash
git add crates/voltex_renderer/src/gltf/ crates/voltex_renderer/src/gltf.rs
git commit -m "refactor(renderer): split gltf.rs (982 lines) into module directory"
```

---

## Task 3: jpg.rs 분리

**현재 위치**: `crates/voltex_renderer/src/jpg.rs` (959줄)
**목표 구조**: `crates/voltex_renderer/src/jpg/`

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod jpg;
pub use jpg::parse_jpg;
```

### 모듈 매핑

| 새 파일 | 원본 line 범위 | 포함 항목 |
|---|---|---|
| `jpg/mod.rs` | 1-5 + 6-121 + tests | `parse_jpg` 본체 (sub-mod 호출), re-exports, tests |
| `jpg/headers.rs` | 122-319 | `SofData`, `JpegComponent`, `HuffTable`, `parse_dqt`, `parse_sof`, `parse_dht` |
| `jpg/bit_reader.rs` | 320-430 | `BitReader` |
| `jpg/idct.rs` | 431-493 | `ZIGZAG`, `idct`, `idct_1d` |
| `jpg/decoder.rs` | 494-672 | `decode_scan`, `decode_block` |
| `jpg/mcu.rs` | 673-755 | `assemble_mcu`, `sample_component` |

예상 줄 수: 각 파일 60~200줄.

- [ ] **Step 1: 디렉토리 + mod.rs**

```bash
mkdir -p crates/voltex_renderer/src/jpg
```

`crates/voltex_renderer/src/jpg/mod.rs`:

```rust
mod headers;
mod bit_reader;
mod idct;
mod decoder;
mod mcu;

use headers::{SofData, parse_dqt, parse_sof, parse_dht};
use decoder::decode_scan;

/// JPEG Baseline 디코더. 반환값: (RGB pixels, width, height).
pub fn parse_jpg(data: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    // 원본 jpg.rs line 6-121의 parse_jpg 본문을 옮기되,
    // 함수 호출 경로를 sub-module로 조정.
}

#[cfg(test)]
mod tests {
    // 원본 jpg.rs의 #[cfg(test)] 블록 line 756-end를 그대로 이동
    use super::*;
    // ...
}
```

- [ ] **Step 2: headers.rs 작성**

원본 line 122-319 을 옮긴다.

- `SofData`, `JpegComponent`, `HuffTable`: `pub(super)`
- `parse_dqt`, `parse_sof`, `parse_dht`: `pub(super) fn`

- [ ] **Step 3: bit_reader.rs 작성**

원본 line 320-430 (BitReader + impl) 을 옮긴다.

- `BitReader`: `pub(super) struct`, 모든 메서드 `pub(super)`

- [ ] **Step 4: idct.rs 작성**

원본 line 431-493 (ZIGZAG, idct, idct_1d) 을 옮긴다.

- `ZIGZAG`: `pub(super) const`
- `idct`, `idct_1d`: `pub(super) fn`

- [ ] **Step 5: decoder.rs 작성**

원본 line 494-672 (decode_scan, decode_block) 을 옮긴다.

상단:
```rust
use super::bit_reader::BitReader;
use super::headers::{SofData, HuffTable};
use super::idct::{idct, ZIGZAG};
use super::mcu::{assemble_mcu, sample_component};
```

- `decode_scan`, `decode_block`: `pub(super) fn`

- [ ] **Step 6: mcu.rs 작성**

원본 line 673-755 (assemble_mcu, sample_component) 을 옮긴다.

상단:
```rust
use super::headers::SofData;
```

- 모두 `pub(super) fn`

- [ ] **Step 7: 원본 jpg.rs 삭제**

```bash
rm crates/voltex_renderer/src/jpg.rs
```

- [ ] **Step 8: 빌드 + 테스트 + 줄 수 검증**

```bash
cargo build -p voltex_renderer 2>&1 | tail -3
cargo test -p voltex_renderer --lib 2>&1 | grep "test result"
find crates/voltex_renderer/src/jpg -name "*.rs" -exec wc -l {} \;
```

Expected: 빌드 성공, 200개 테스트 통과, 모든 파일 600줄 이하.

- [ ] **Step 9: Commit**

```bash
git add crates/voltex_renderer/src/jpg/ crates/voltex_renderer/src/jpg.rs
git commit -m "refactor(renderer): split jpg.rs (959 lines) into module directory"
```

---

## Task 4: deferred_pipeline.rs 분리

**현재 위치**: `crates/voltex_renderer/src/deferred_pipeline.rs` (905줄)
**목표 구조**: `crates/voltex_renderer/src/deferred_pipeline/`

**lib.rs의 현재 선언** (변경 없어야 함):
```rust
pub mod deferred_pipeline;
pub use deferred_pipeline::{
    create_gbuffer_pipeline, create_lighting_pipeline,
    gbuffer_camera_bind_group_layout,
    lighting_gbuffer_bind_group_layout, lighting_lights_bind_group_layout, lighting_shadow_bind_group_layout,
    ssgi_gbuffer_bind_group_layout, ssgi_data_bind_group_layout, create_ssgi_pipeline,
    rt_shadow_gbuffer_bind_group_layout, rt_shadow_data_bind_group_layout, create_rt_shadow_pipeline,
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
    tonemap_bind_group_layout, create_tonemap_pipeline,
};
```

### 모듈 매핑

| 새 파일 | 원본 line 범위 | 포함 함수 |
|---|---|---|
| `deferred_pipeline/mod.rs` | (re-exports) | sub-mod 선언, `pub use` 17개 함수 |
| `deferred_pipeline/gbuffer.rs` | 15-116 | `gbuffer_camera_bind_group_layout`, `create_gbuffer_pipeline` |
| `deferred_pipeline/lighting.rs` | 117-361 | `lighting_gbuffer_bind_group_layout`, `lighting_lights_bind_group_layout`, `lighting_shadow_bind_group_layout`, `create_lighting_pipeline` |
| `deferred_pipeline/ssgi.rs` | 362-524 | `ssgi_gbuffer_bind_group_layout`, `ssgi_data_bind_group_layout`, `create_ssgi_pipeline` |
| `deferred_pipeline/rt_shadow.rs` | 525-628 | `rt_shadow_gbuffer_bind_group_layout`, `rt_shadow_data_bind_group_layout`, `create_rt_shadow_pipeline` |
| `deferred_pipeline/bloom.rs` | 629-799 | `bloom_bind_group_layout`, `create_bloom_downsample_pipeline`, `create_bloom_upsample_pipeline` |
| `deferred_pipeline/tonemap.rs` | 800-905 | `tonemap_bind_group_layout`, `create_tonemap_pipeline` |

예상 줄 수: 각 파일 100~250줄.

**참고**: `deferred_pipeline::bloom`은 `voltex_renderer::bloom`(src/bloom.rs)과 다른 모듈 경로이므로 충돌 없음.

- [ ] **Step 1: 디렉토리 + mod.rs**

```bash
mkdir -p crates/voltex_renderer/src/deferred_pipeline
```

`crates/voltex_renderer/src/deferred_pipeline/mod.rs`:

```rust
mod gbuffer;
mod lighting;
mod ssgi;
mod rt_shadow;
mod bloom;
mod tonemap;

pub use gbuffer::{gbuffer_camera_bind_group_layout, create_gbuffer_pipeline};
pub use lighting::{
    lighting_gbuffer_bind_group_layout,
    lighting_lights_bind_group_layout,
    lighting_shadow_bind_group_layout,
    create_lighting_pipeline,
};
pub use ssgi::{
    ssgi_gbuffer_bind_group_layout,
    ssgi_data_bind_group_layout,
    create_ssgi_pipeline,
};
pub use rt_shadow::{
    rt_shadow_gbuffer_bind_group_layout,
    rt_shadow_data_bind_group_layout,
    create_rt_shadow_pipeline,
};
pub use bloom::{
    bloom_bind_group_layout,
    create_bloom_downsample_pipeline,
    create_bloom_upsample_pipeline,
};
pub use tonemap::{tonemap_bind_group_layout, create_tonemap_pipeline};
```

- [ ] **Step 2: gbuffer.rs 작성**

원본 line 15-116 을 옮긴다.

상단 (원본 file 상단의 use를 검토 후 필요한 것 import):
```rust
// 원본 deferred_pipeline.rs 1-14줄에 있는 use 문 중 이 모듈에서 사용하는 것만 옮긴다.
// 예: use wgpu;
```

- 모든 함수: `pub fn` (mod.rs에서 `pub use`로 재노출)

- [ ] **Step 3: lighting.rs 작성**

원본 line 117-361 을 옮긴다. 필요한 use 추가.

- 모든 함수: `pub fn`

- [ ] **Step 4: ssgi.rs 작성**

원본 line 362-524 을 옮긴다.

- 모든 함수: `pub fn`

- [ ] **Step 5: rt_shadow.rs 작성**

원본 line 525-628 을 옮긴다.

- 모든 함수: `pub fn`

- [ ] **Step 6: bloom.rs 작성**

원본 line 629-799 을 옮긴다.

- 모든 함수: `pub fn`

- [ ] **Step 7: tonemap.rs 작성**

원본 line 800-905 을 옮긴다.

- 모든 함수: `pub fn`

- [ ] **Step 8: 원본 deferred_pipeline.rs 삭제**

```bash
rm crates/voltex_renderer/src/deferred_pipeline.rs
```

- [ ] **Step 9: 빌드 + 테스트 + 줄 수 검증**

```bash
cargo build -p voltex_renderer 2>&1 | tail -3
cargo test -p voltex_renderer --lib 2>&1 | grep "test result"
find crates/voltex_renderer/src/deferred_pipeline -name "*.rs" -exec wc -l {} \;
```

Expected: 빌드 성공, 200개 테스트 통과, 모든 파일 600줄 이하.

- [ ] **Step 10: Commit**

```bash
git add crates/voltex_renderer/src/deferred_pipeline/ crates/voltex_renderer/src/deferred_pipeline.rs
git commit -m "refactor(renderer): split deferred_pipeline.rs (905 lines) into module directory"
```

---

## Task 5: 최종 검증 및 push

- [ ] **Step 1: 800줄+ 파일 0개 확인**

```bash
find crates -name "*.rs" -type f | xargs wc -l 2>/dev/null | sort -rn | head -20
```

Expected: 모든 분리 대상이 600줄 이하. (500-800줄 파일은 여전히 존재 — M3에서 처리)

```bash
# 800줄 초과 파일 카운트
find crates -name "*.rs" -type f -exec wc -l {} \; | awk '$1 > 800 {print}'
```

Expected: 출력 없음 (0개).

- [ ] **Step 2: 전체 빌드/테스트**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --workspace 2>&1 | tail -3
cargo test --workspace --lib 2>&1 | grep -E "^test result"
```

Expected:
- 빌드 성공 (`Finished`)
- 11줄의 `test result: ok. N passed` (각 crate 1줄)
- 실패 없음

전체 카운트:
```bash
cargo test --workspace --lib 2>&1 | grep -E "^test result" | awk '{sum += $4} END {print sum}'
```
Expected: 760

- [ ] **Step 3: CODEMAP 갱신** (vorbis 등 큰 파일 정보 수정)

영향받는 CODEMAP:
- `crates/voltex_audio/CODEMAP.md`: "vorbis.rs는 1493줄" → "vorbis 모듈 (분리됨)"
- `crates/voltex_renderer/CODEMAP.md`: gltf/jpg/deferred_pipeline 분리 반영

`crates/voltex_audio/CODEMAP.md`의 다음 부분을 수정:

```markdown
- `wav`, `ogg`, `vorbis` — 디코더 (src/wav.rs, src/ogg.rs, src/vorbis/)
```

그리고 "변경 시 주의점"에서 "vorbis.rs는 1493줄로 매우 큼 (M2에서 분리 예정)" 라인을 제거하고 다음으로 대체:

```markdown
- vorbis는 src/vorbis/ 디렉토리로 분리됨 (bit_reader, codebook, floor, residue, headers, imdct, decoder)
```

`crates/voltex_renderer/CODEMAP.md`의 "변경 시 주의점"에서:
- "gltf.rs (982줄), jpg.rs (959줄), deferred_pipeline.rs (905줄)" 줄 수 부분 제거하고
- "gltf, jpg, deferred_pipeline은 src/<name>/ 디렉토리로 분리됨" 추가

그리고 모듈 구조 섹션에서 해당 파일들의 경로를 디렉토리로 갱신.

- [ ] **Step 4: CODEMAP 갱신 commit**

```bash
git add crates/voltex_audio/CODEMAP.md crates/voltex_renderer/CODEMAP.md
git commit -m "docs(codemap): update audio/renderer CODEMAPs for M2 splits"
```

- [ ] **Step 5: Push**

```bash
git log --oneline -10
git push origin master
```

Expected: 5개 새 commit 푸시.

---

## 완료 기준 (Definition of Done)

- [ ] `crates/voltex_audio/src/vorbis/` 디렉토리 존재, 원본 `vorbis.rs` 없음, 모든 파일 ≤600줄
- [ ] `crates/voltex_renderer/src/gltf/` 디렉토리 존재, 원본 `gltf.rs` 없음, 모든 파일 ≤600줄
- [ ] `crates/voltex_renderer/src/jpg/` 디렉토리 존재, 원본 `jpg.rs` 없음, 모든 파일 ≤600줄
- [ ] `crates/voltex_renderer/src/deferred_pipeline/` 디렉토리 존재, 원본 `deferred_pipeline.rs` 없음, 모든 파일 ≤600줄
- [ ] 800줄 초과 `.rs` 파일 0개
- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace --lib` 760개 통과 (regression 0)
- [ ] 영향받은 CODEMAP 갱신 (voltex_audio, voltex_renderer)
- [ ] Push 완료

## 리스크 및 대응

- **분리 중 빌드 깨짐**: 한 task가 끝나기 전까지 빌드 깨질 수 있음 — Step에서 task 마지막에 빌드 검증 후 commit
- **public API 변경 누락**: 외부 호출자에서 컴파일 오류 → 누락된 `pub use` 추가
- **테스트 분리 오류**: 테스트가 sub-module 내부를 참조 → `use super::*;` 또는 명시적 `use super::<sub_mod>::*;` 사용
- **순환 의존**: sub-module 간 순환 발견 시 공통 항목을 더 하위 모듈로 추출

## 다음 마일스톤

M3: 500-800줄 파일 11개 분리 (widgets, navmesh_builder, world, solver, dock, gjk, wasapi, ttf_parser, texture, state, navmesh).
