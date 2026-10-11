# voltex_image

## 책임 (한 줄)
GPU 무의존(L0) asset codecs — PNG/JPG/glTF/OBJ/JSON 디코더 + PNG 인코더 + `MeshVertex` POD.

## 공개 API 진입점

- `parse_png(data: &[u8]) -> Result<(u32, u32, Vec<u8>)>` — PNG → RGBA8 픽셀 (src/png.rs)
- `parse_jpg(data: &[u8]) -> Result<(u32, u32, Vec<u8>)>` — JPEG Baseline + Progressive → RGBA8 (src/jpg/, src/progressive_jpeg.rs)
- `parse_gltf(data: &[u8]) -> Result<GltfData>` — glTF 2.0 / GLB 파서 (src/gltf/). 정점 속성은 `bufferView.byteStride`(인터리브, v1.0.132)와 정규화 정수 componentType(v1.0.133, `gltf/component.rs`) 지원. 노드 `matrix`는 TRS로 분해(v1.0.137)
- `encode_rgba8(w: u32, h: u32, rgba: &[u8]) -> Vec<u8>` — RGBA8 → PNG (zlib stored block, 무압축) (src/png_encode.rs)
- `GltfData`, `GltfMesh`, `GltfMaterial` — glTF 구조체 (src/gltf/). 머티리얼은 계수만(텍스처 미임포트 — DEFERRED 참고)
- `MeshVertex` — 위치/법선/UV/탄젠트 정점 POD (bytemuck::Pod + Zeroable) (src/vertex.rs)
- `ImageError`, `Result` — thiserror 기반 에러 (Generic) (src/error.rs)

### 모듈 경로 (pub mod)
`deflate`, `gltf`, `jpg`, `json_parser`, `obj`, `png`, `png_encode`, `progressive_jpeg`, `vertex`, `error`

## 모듈 구조
- `error` — `ImageError` enum + `Result<T>` alias (src/error.rs)
- `vertex` — `MeshVertex` POD struct; 위치/법선/UV/탄젠트 (src/vertex.rs)
- `checksum` — CRC32/Adler32 구현 (**crate-private** — `mod checksum;`로만 선언, 외부 노출 금지) (src/checksum.rs)
- `deflate` — zlib/DEFLATE inflate (src/deflate.rs)
- `png` — PNG 디코더: IHDR 검증, 필터 재구성, `parse_png` (src/png.rs)
- `png_encode` — PNG 인코더: `encode_rgba8` (stored deflate, CRC32/Adler32 사용) (src/png_encode.rs)
- `jpg` — JPEG Baseline 8-bit RGB→RGBA 디코더 (src/jpg/)
- `progressive_jpeg` — JPEG Progressive 디코더 (src/progressive_jpeg.rs)
- `obj` — OBJ 파서: `parse_obj`, `compute_tangents`, `ObjData` (src/obj.rs)
- `json_parser` — 소형 JSON 파서: `parse_json`, `JsonValue` (src/json_parser.rs)
- `gltf` — glTF 2.0/GLB 파서: `parse_gltf`, `GltfData`/`GltfMesh`/`GltfMaterial`/`Channel` + 애니메이션 타입 (src/gltf/)

## 의존성
- voltex crate: **없음** (L0 — GPU/voltex 의존 0)
- 외부 crate: `bytemuck` (MeshVertex POD derive), `thiserror` (에러 타입)

## 변경 시 주의점
- 디코더 결과 포맷(픽셀 채널 순서, 스트라이드 등)이 바뀌면 `voltex_renderer`의 GPU 업로드 경로(`texture.rs` 등) 및 `MESH_VERTEX_LAYOUT`(`renderer/vertex.rs`)도 같이 검토.
- `MeshVertex` 필드 추가/제거 시 renderer의 `MESH_VERTEX_LAYOUT` offsets/stride 동기화 필요 (현재: offset 0/12/24/32, stride = `size_of::<MeshVertex>()`).
- `checksum` 모듈은 crate-private(`mod checksum;` — `pub mod` 아님). 외부에 노출 금지. PNG 인코더 내부 전용.
- 모든 파서는 panic 금지 — 잘못된 입력에는 `ImageError::Generic` 반환 (fuzz 가드).
- `parse_png`/`parse_jpg`는 dimension 0 및 과대 dimension을 거부 (v1.0.40 보강 상속).
- `voltex_renderer`는 역호환을 위해 `pub use voltex_image::{parse_png, parse_jpg, parse_gltf, MeshVertex, ...}` 재export를 유지 — 기존 코드는 `voltex_renderer::parse_png` 경로로도 동작.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_image --lib`
- 현재 테스트 개수: 91개 (renderer에서 이관 + crate 자체 테스트)
