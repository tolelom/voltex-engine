# Voltex Examples — Catalog

각 example 은 `cargo run -p <name>` 으로 실행. 더 자세한 정보는 example
디렉토리의 `README.md` (v1.0.38 부터) 또는 소스 코드의 모듈 헤더 주석 참고.

> **시각 검증 한계**: GUI 자동화가 없어 README 의 동작 묘사는 _개발 호스트
> (Windows) 에서의 수동 실행 결과 기준_. 다른 OS / 머신에서의 실측은
> 환영. 자세한 검증 한계는 `docs/STATUS.md` 의 "v1.0.x 추가 한계" 섹션.

## 추천 학습 순서

처음 보는 사용자는 위에서 아래로 따라가면 엔진 모든 주요 시스템을 한
바퀴 돌아볼 수 있다.

| # | Example | 학습 포인트 | 의존 crate |
|---|---|---|---|
| 1 | [`triangle`](triangle) | wgpu 파이프라인 첫 픽셀 | renderer, platform |
| 2 | [`model_viewer`](model_viewer) | OBJ/glTF 로딩 + FPS 카메라 + Blinn-Phong | renderer, asset |
| 3 | [`many_cubes`](many_cubes) | 400 entity dynamic UBO + ECS query | renderer, ecs |
| 4 | [`hierarchy_demo`](hierarchy_demo) | Parent/Children + WorldTransform 전파 (solar system) | ecs |
| 5 | [`asset_demo`](asset_demo) | AssetLoader + FileWatcher 핫 리로드 | asset |
| 6 | [`pbr_demo`](pbr_demo) | Cook-Torrance PBR + 7×7 metallic/roughness grid | renderer |
| 7 | [`multi_light_demo`](multi_light_demo) | Directional + Point + Spot 16-light 컬링 | renderer |
| 8 | [`shadow_demo`](shadow_demo) | CSM (2 cascade) + Point/Spot shadow map | renderer |
| 9 | [`ibl_demo`](ibl_demo) | SH irradiance + GPU BRDF LUT + Hosek-Wilkie sky | renderer |
| 10 | [`deferred_demo`](deferred_demo) | G-Buffer 4 MRT + RT Shadows + Bloom + TAA | renderer |
| 11 | [`audio_demo`](audio_demo) | WAV/OGG 로딩 + WASAPI/ALSA/CoreAudio 출력 | audio |
| 12 | [`editor_demo`](editor_demo) | IMGUI dock + 3D viewport + Inspector + Lua + Play/Stop | editor, script |
| 13 | [`runtime_demo`](runtime_demo) | RuntimeApp 솔라 시스템, `main` 4 줄 | runtime |
| 14 | [`tutorial_minigame`](tutorial_minigame) | 30분 튜토리얼 최종 산출물 (D1+D2) | runtime, script |
| -- | [`survivor_game`](survivor_game) | ECS+physics+facade 로 재작성한 archive (대조용) | (모든 sub-crate) |

게임 풀 예시 (flagship): [`../games/voltex_survivor`](../games/voltex_survivor)
— 게임 로직 100% Lua + scene 으로 표현 (Rust 측 부트스트랩 + input ~80 라인).

## 카테고리별

### Rendering 기초
`triangle` → `model_viewer` → `many_cubes`

### ECS / Scene Graph
`many_cubes` → `hierarchy_demo` → `asset_demo`

### PBR + Lighting + Shadows
`pbr_demo` → `multi_light_demo` → `shadow_demo` → `ibl_demo`

### Advanced Rendering (deferred / RT / TAA / Bloom)
`deferred_demo` 한 곳에 통합

### Audio
`audio_demo`

### Editor + Scripting
`editor_demo`

### Runtime + 게임 작성
`runtime_demo` → `tutorial_minigame` → `../games/voltex_survivor`

## 실행 공통

```bash
cargo run -p <name>            # debug, 빌드 후 즉시 launch
cargo run -p <name> --release  # release 빌드 (PBR / shadow 부드러움)
```

각 example 은 자체 윈도우를 열고 60 FPS 로 동작. 5초 timeout 후 종료해도
panic 없음 (v1.0.2 smoke-test 검증).

## 작성 정책

신규 example 추가 시:
- `Cargo.toml` 의 `[package].description` 1~2줄로 학습 포인트 명시.
- 워크스페이스 `Cargo.toml` `members` 에 등록.
- 이 catalog 의 표에 한 줄 추가.
- 모듈 헤더 (`//!`) 에 핵심 흐름 (3~5 줄) 코멘트.

예제 코드는 voltex facade (`use voltex::prelude::*;`) 를 사용하는 게
권장 (M6 — facade crate 정책). 기존 예제 (triangle, model_viewer 등) 는
voltex_<crate> 직접 import 도 그대로 동작.
