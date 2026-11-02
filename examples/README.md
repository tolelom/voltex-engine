# Examples

> 엔진 기능별 예제. 위에서부터 차례로 실행하면 렌더링, ECS, 에셋, 조명, 오디오, 에디터, 런타임 순으로 엔진을 한 바퀴 돈다.

```bash
cargo run -p <name>             # 디버그 빌드
cargo run -p <name> --release   # 릴리스 빌드. 렌더링 예제는 이쪽이 부드럽다
```

각 폴더의 README에 조작법과 코드에서 볼 곳이 있다.

| 예제 | 보여 주는 것 |
|---|---|
| [triangle](triangle) | wgpu 파이프라인으로 첫 삼각형 |
| [model_viewer](model_viewer) | 모델 로딩, FPS 카메라, Blinn-Phong 셰이딩 |
| [many_cubes](many_cubes) | ECS 쿼리로 큐브 여러 개 그리기 |
| [hierarchy_demo](hierarchy_demo) | 부모·자식 계층과 transform 전파(태양계) |
| [asset_demo](asset_demo) | 에셋 로더와 파일 변경 감지 핫 리로드 |
| [pbr_demo](pbr_demo) | PBR 재질, metallic·roughness 격자 |
| [multi_light_demo](multi_light_demo) | directional·point·spot 광원과 절두체 컬링 |
| [shadow_demo](shadow_demo) | 캐스케이드 그림자, point·spot 그림자 |
| [ibl_demo](ibl_demo) | 이미지 기반 조명과 절차적 하늘 |
| [deferred_demo](deferred_demo) | 디퍼드 렌더링, 레이트레이싱 그림자, Bloom, TAA |
| [audio_demo](audio_demo) | WAV·OGG 재생 |
| [editor_demo](editor_demo) | 에디터: 도킹 UI, 3D 뷰포트, Inspector, Lua, Play·Stop |
| [runtime_demo](runtime_demo) | `RuntimeApp`으로 짧은 `main`, HUD |
| [game_ready_demo](game_ready_demo) | 런타임 기능 모음: Lua 입력, 내비게이션, 충돌 이벤트, 3D 사운드, 스키닝, 파티클, 디버그 드로잉 |
| [boids_demo](boids_demo) | 선언형 렌더링으로 만든 군집 시뮬레이션 |
| [tutorial_minigame](tutorial_minigame) | [튜토리얼](../docs/tutorial.md) 완성본 |
| [survivor_game](survivor_game) | `games/voltex_survivor`의 초기 Rust 버전. 비교용으로 보관 |

완성된 게임은 [games/](../games)에 있다.

새 예제는 워크스페이스 `Cargo.toml`의 `members`와 이 표에 함께 추가한다.
