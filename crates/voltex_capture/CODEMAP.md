# voltex_capture

Headless offscreen 렌더링 + PNG 캡처. 윈도우 없는 환경에서 *실제* GPU 파이프라인을 텍스처에 그려 픽셀을 읽어 PNG 로 저장한다. "엔진이 실제로 화면을 올바르게 그리는가" 를 자동화 가능한 산출물로 바꾼다 (그 전엔 6초 smoke-test = panic 없음만 보장).

## 책임

- surface 없는 wgpu device/queue 획득 (`HeadlessGpu`) — `compatible_surface: None` 으로 헤드리스에서 동작
- 오프스크린 color(Rgba8UnormSrgb, COPY_SRC) + depth 타겟 (`OffscreenTarget`) + 256-byte row 정렬을 처리하는 CPU readback
- PNG 인코더 (`encode_rgba8`) — `voltex_image::encode_rgba8` 재export. zlib stored block + CRC32/Adler32 (C1 감사 해소: capture의 중복 구현 제거, voltex_image로 단일화). `voltex_renderer::parse_png` 로 라운드트립 검증됨
- 실제 렌더 경로로 데모 씬 렌더 (`scene::*`) — BasicRenderer (Blinn-Phong) + create_pbr_pipeline (Cook-Torrance + IBL)
- 프레임이 명백히 깨졌는지 사람 없이 판정하는 통계 (`ImageStats`: black_fraction / distinct_colors / mean_luminance / looks_blank)

## 공개 API

| 항목 | 파일 | 메모 |
|---|---|---|
| `HeadlessGpu::new() -> Result<Self, String>` | `headless.rs` | GPU 없으면 Err (CI graceful skip) |
| `OffscreenTarget::new(device, w, h)` / `read_rgba8(device, queue) -> Vec<u8>` | `target.rs` | `CAPTURE_FORMAT` = Rgba8UnormSrgb |
| `encode_rgba8(w, h, &[u8]) -> Vec<u8>` / `save_png(path, w, h, &[u8])` | `png_encode.rs`, `lib.rs` | stored deflate, 무압축 |
| `ImageStats::analyze(&[u8])` / `looks_blank()` | `lib.rs` | blank/단색 프레임 자동 탐지 |
| `scene::render_single_cube` / `render_cube_grid` / `render_pbr_grid` | `scene.rs` | `(gpu, w, h) -> Vec<u8>` |
| `scene::render_world(gpu, w, h, &mut World, cam_pos, yaw, pitch) -> Vec<u8>` | `scene.rs` | 외부 게임 crate가 자기 씬을 픽셀 검증하는 공개 진입점 (voltex_depths 도그푸딩용). 고정 라이트/FOV/1024 entity 상한 — doc 주석 참고 |
| `scene::render_world_ortho(gpu, w, h, &mut World, cam_pos, yaw, pitch, ortho_height) -> Vec<u8>` | `scene.rs` | `render_world`의 직교 변형 — 런타임 `with_orthographic`과 동일 의미의 뷰 높이. 2D 게임이 실제 화면 구도로 픽셀 검증 (voltex_leap 도그푸딩용) |
| bin `capture` | `bin/capture.rs` | `cargo run -p voltex_capture --bin capture [out_dir]` |

## 의존성

- voltex_math, voltex_ecs, voltex_asset, voltex_renderer, voltex_renderer_ecs, voltex_image
- wgpu, bytemuck, pollster

## 모듈 구조

```
src/
├── lib.rs              # re-exports + save_png + ImageStats
├── headless.rs         # HeadlessGpu (surface 없는 device/queue)
├── target.rs           # OffscreenTarget + readback
├── png_encode.rs       # 무의존 PNG 인코더 (+ CRC32/Adler32)
├── scene.rs            # 데모 씬 렌더 (BasicRenderer / PBR)
└── bin/capture.rs      # 모든 씬을 PNG 로 저장하는 CLI
```

## 변경 시 주의점

- GPU 가 필요한 테스트는 `#[ignore]` 처리 (CI 무 GPU). `cargo test -p voltex_capture -- --ignored` 로 실행. 무 GPU 면 graceful skip.
- `scene::*` 는 각 example (pbr_demo 등) 의 셋업을 *재현* 한 것 — example 코드 자체가 아니다. example 의 셋업이 바뀌면 여기도 갱신해야 검증 의미 유지.
- PNG 인코더(`encode_rgba8`)는 `voltex_image`에서 재export — 구현 변경은 `voltex_image::png_encode`에서. 무압축이라 파일이 큼 (검증용 단명 산출물 의도). 압축이 필요하면 deflate fixed/dynamic Huffman 도입.
- 기본 라이트 `LightUniform::new()` 의 방향 (0,-1,-1) 은 +Y/+Z 면을 동일하게 비춤 — 면별 음영 검증엔 비대칭 라이트 사용 (single_cube 참조).
- 캡처 PNG 출력은 `target/captures/` (gitignore). reference 이미지는 `docs/visual/` 에 별도 커밋.
