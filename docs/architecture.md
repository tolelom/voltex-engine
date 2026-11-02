# 구조

> 엔진 crate가 어떻게 쌓여 있는지, 한 프레임이 어떤 순서로 도는지, 코드를 어디서부터 읽으면 되는지 설명한다.

## 저장소

| 폴더 | 내용 |
|---|---|
| `engine/` | 엔진 crate |
| `tools/` | 엔진을 쓰는 도구. `voltex_cli`(게임 빌드), `voltex_capture`(렌더 결과 PNG 캡처) |
| `games/` | 엔진으로 만든 게임. `survivor`, `depths`, `leap` |
| `examples/` | 기능별 짧은 예제 |

게임과 예제는 엔진을 `voltex` crate 하나로 가져다 쓴다. 엔진 crate는 게임이나 도구를 모른다.

## crate 층

아래 층만 의존한다. 화살표는 "가져다 쓴다"는 뜻이다.

```
voltex             facade. 아래 crate를 모듈로 다시 내보내고 prelude 제공
  │
voltex_runtime     창, 렌더, 물리, 스크립트, 오디오를 묶어 한 프레임을 돌리는 앱
  │
  ├─ voltex_renderer_ecs   ECS 엔티티를 렌더러 입력으로 바꾸는 연결층   → ecs, renderer, asset
  ├─ voltex_editor         즉시 모드 UI, 에디터 패널, TTF                → ecs, renderer
  │
  ├─ voltex_renderer   wgpu 렌더러. PBR, 그림자, 디퍼드, 레이트레이싱  → platform, image
  ├─ voltex_physics    충돌, 강체, 레이캐스트                          → ecs
  ├─ voltex_ai         NavMesh, 경로 탐색, steering                    → ecs
  ├─ voltex_script     Lua 5.4와 엔진 API 바인딩                       → ecs, ai, audio, asset
  │
  ├─ voltex_platform   창, 입력, 타이머(winit)
  ├─ voltex_ecs        World, Entity, Component, 씬 저장·불러오기      → asset
  ├─ voltex_audio      WAV·OGG 디코더, 믹서, OS별 출력
  ├─ voltex_net        UDP, 신뢰 채널, 스냅샷
  │
  └─ voltex_math, voltex_asset, voltex_image   다른 voltex crate에 의존하지 않음
```

`voltex_math`는 `asset`, `image`를 뺀 거의 모든 crate가 쓴다. 그림에서는 생략했다. `voltex_net`은 `runtime`이 쓰지 않고 facade로만 노출된다.

도구의 의존은 이렇다.

- `voltex_cli`: `voltex_asset`, `voltex_ecs`. 엔진을 직접 링크하지 않고, 게임을 `cargo build`로 빌드해 `dist/`에 묶는다.
- `voltex_capture`: `voltex_renderer`, `voltex_renderer_ecs`, `voltex_editor` 등. 창 없이 GPU로 그려 PNG를 만든다.

## 엔진을 쓰는 두 방법

| 방법 | 쓰는 곳 | 특징 |
|---|---|---|
| `RuntimeApp`에 맡김 | 게임 3편, `runtime_demo`, `game_ready_demo`, `boids_demo`, `tutorial_minigame` | `main`은 설정과 콜백 등록만 한다. 창, 루프, 렌더는 runtime이 돈다 |
| 부품을 직접 조립 | 나머지 예제(`triangle`, 렌더링·에셋·오디오·에디터 예제, `survivor_game`) | winit 루프와 wgpu 파이프라인을 예제가 직접 짠다 |

새 게임은 첫 번째 방법으로 만든다. 두 번째는 한 기능을 떼어 보여주는 예제와, `RuntimeApp`이 생기기 전에 만든 `survivor_game`이 쓴다.

`RuntimeApp`에 맡기는 게임은 설정 빌더로 필요한 것만 끼운다.

```rust
RuntimeApp::new(
    RuntimeConfig::new()
        .with_window_title("My Game")
        .with_scene_builder(setup)        // 시작할 때 World 채우기
        .with_gpu_init(upload_meshes)     // GPU가 준비된 뒤 메시·재질 올리기
        .with_system_setup(add_systems),  // 매 프레임 돌 게임 로직 등록
)?
.run()
```

## 한 프레임

`RuntimeApp::run()`은 winit 이벤트 루프를 돌린다. 창이 다시 그려질 때마다 `engine/voltex_runtime/src/handler.rs`의 `redraw`가 한 프레임을 처리한다.

1. 시간: 타이머를 진행하고 이번 프레임의 `dt`를 정한다.
2. 카메라 조작: 마우스 오른쪽 버튼을 누르고 있을 때만 기본 자유 카메라가 움직인다.
3. 게임 로직(`RuntimeApp::tick_systems_only`, `app.rs`)
   1. 부모·자식 transform 전파
   2. 스켈레톤 애니메이션, 파티클, 스프라이트 애니메이션
   3. 물리 한 스텝
   4. 충돌 이벤트를 게임 콜백에 전달
   5. Lua 스크립트: 입력 상태를 넘기고 `on_start`, `on_update` 호출, 바뀐 스크립트 다시 불러오기
   6. 게임이 등록한 시스템
   7. 오디오: 엔티티의 소리 상태를 오디오 장치에 반영
4. GPU 자원 준비: 이번 프레임에 생긴 엔티티의 메시·재질을 GPU에 올리고, 씬 파일의 텍스처·메시 경로를 불러온다.
5. 게임 콜백: 카메라, 조명, 디버그 선을 게임이 정한 대로 갱신한다.
6. 입력 정리: 이번 프레임의 "막 눌림" 상태를 지운다. 게임 로직이 본 뒤라서 여기서 지운다.
7. 렌더: 렌더러가 World를 그리고, HUD를 켰으면 그 위에 UI를 그린 뒤 화면에 내보낸다.

## 코드 읽는 순서

1. `examples/runtime_demo/src/main.rs`: `RuntimeApp`을 쓰는 가장 짧은 프로그램
2. `engine/voltex_runtime/src/handler.rs`의 `redraw`: 위 "한 프레임"의 실제 코드
3. `engine/voltex_runtime/src/app.rs`의 `tick_systems_only`: 게임 로직 순서
4. 관심 있는 층의 crate. 각 crate 폴더의 `CODEMAP.md`에 책임, 공개 API, 고칠 때 주의할 점이 있다.
5. 게임: Lua로 만든 예는 `games/survivor`, Rust로 만든 예는 `games/depths`
