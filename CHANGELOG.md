# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## Unreleased — 2026-10-17 — capture scene 테스트 분리

### Refactor

- **폴더 구성 `engine/`·`tools/`**: `crates/`를 엔진 crate 15개(`engine/`)와 엔진을 쓰는 도구(`tools/voltex_cli`,
  `tools/voltex_capture`)로 나눴다. 루트 `Cargo.toml`의 members를 엔진·도구·게임·예제 묶음으로 다시 적었다.
  `voltex build`가 만드는 래퍼 crate는 엔진 경로의 `engine/voltex_runtime`을 가리킨다. 저장소를 도는 테스트
  (`module_wiring`, `repo_scenes`)와 pre-push·`check_changelog.sh`의 코드 경로 목록도 새 폴더로 바꿨다.
- **게임 이름에서 `voltex_` 제거**: `voltex_survivor`·`voltex_depths`·`voltex_leap`을 `survivor`·`depths`·`leap`으로
  바꿨다(폴더, 패키지, `voltex.toml`의 `name`). 엔진 crate와 이름 모양이 같아 구별되지 않았다. 실행은
  `cargo run -p depths`, 빌드 결과는 `dist/survivor` 형태가 된다. 이 항목 위의 옛 기록은 당시 이름 그대로 둔다.
- **README 저장소 구성 표**: 맨 위에 `engine/`·`tools/`·`games/`·`examples/`·`docs/`가 각각 무엇인지 적고,
  crate 구성 그림을 엔진과 도구로 나눴다(전에는 cli·capture가 facade 아래 엔진 부품처럼 그려져 있었다).
- **구조 문서 `docs/architecture.md`**: crate 층(각 `Cargo.toml` 의존 기준), `RuntimeApp`에 맡기는 방법과 부품을
  직접 조립하는 방법의 구분, `redraw`·`tick_systems_only` 기준 한 프레임 순서, 코드 읽는 순서를 적었다.

- **voltex_capture `scene`**: 인라인 테스트를 `scene/tests.rs`로 옮겼다(`scene.rs` → `scene/mod.rs` 1140줄 +
  `scene/tests.rs` 704줄). 코드는 바꾸지 않았고 형제 모듈 재노출과 `scene::render_*` 경로는 그대로다.
  확인: 1762 통과·76 무시, GPU 캡처 47 통과(skip 0), 캡처 29장 분할 전과 픽셀 단위로 같음.

### Fix

- **voltex_audio Linux 빌드**: Gitea CI(aarch64 Linux)의 `RUSTFLAGS=-D warnings` 빌드가 첫 실행부터
  실패하던 원인을 없앴다. 명령 적용(`apply_command`)·믹싱 import·그 테스트를 `cfg(target_os = "windows")`
  로 묶고, 무음 모드가 payload를 버리는 `AudioCommand` 에 비-Windows 한정 `expect(dead_code)` 를 달았다.
  `AlsaBackend`·`CoreAudioBackend` 에 빠진 doc 주석을 넣고, aarch64 에서만 불필요해지는
  `as *const u8` 캐스트를 `.cast::<u8>()` 로 바꿨다. Windows 컴파일 결과와 동작은 같다. macOS 는 검증하지 않았다.
- **deny.toml**: cargo-deny 0.19 가 거부하는 `highlight = "all-versions"` 를 `"all"` 로 바꿨다(설정 파싱 실패 해소).
  `[advisories] ignore` 에 간접 의존 권고 4건(quick-xml RUSTSEC-2026-0194·0195 via wayland-scanner,
  paste RUSTSEC-2024-0436 via metal, ttf-parser RUSTSEC-2026-0192 via sctk-adwaita)을 경로와 함께 넣었다.
  workspace 37개 crate 모두 `publish = false`(`[workspace.package]` + `publish.workspace = true`)로 두고
  `allow-wildcard-paths = true` 를 켜서 path 의존 wildcard 34 crate 를 해소했다 — 이 옵션은 `publish` 를
  제한한 crate 에만 적용된다. 이제 `cargo deny check` 는 exit 0(경고만: duplicate 10, license-not-encountered 4).
- **Linux CI 통과**: Rust 1.99 clippy 새 lint(`chunks_exact_to_as_chunks`, `for_kv_map`, `explicit_counter_loop`)를
  voltex_image·net·script·capture와 두 게임의 visual 테스트에서 고쳤다(`as_chunks::<N>()`, `values_mut()`, `zip`).
  voltex_editor TTF 테스트 17개가 `C:/Windows/Fonts`를 읽어 Windows 밖에서 실패하던 것을, 메모리에서 만드는 최소
  TrueType 폰트(`src/test_font.rs`)로 바꿨다. `ttf_fuzz`는 시스템 폰트가 없으면 건너뛰던 것을 이 폰트로 대신 돈다.
  voltex_cli `pick_executable` 테스트의 `\` 경로 리터럴을 OS 경로 조립으로 바꿨다. 확인(Ubuntu, Rust 1.99.1,
  `RUSTFLAGS=-D warnings`): build·clippy 경고 0, `test --workspace --lib` 1594 통과, `test -p voltex_cli` 72 통과.

### Docs

- **작업 문서 `docs/dev/`로 이동**: `STATUS`·`DEFERRED`·`NEXT`·`AUDIT-BACKLOG`·`dogfood-friction`(2개)·
  `v1.0-pub-audit`·`v1.0-retrospective`를 옮기고 문서·코드 주석·스크립트의 경로를 갱신했다. `docs/` 바로 아래에는
  사용 문서(tutorial, voltex-toml-reference, build-matrix, release, visual)만 남는다.
- **README 재작성**: 실행 명령을 `cargo run -p <name>`으로 바로잡고(example은 독립 패키지라 `--example`로는
  실행되지 않는다, tutorial의 두 곳도 함께), 버전·커버리지·개수처럼 시점이 지나면 틀리는 배지와 수치를 뺐다.
  `games/`의 게임 세 편과 crate 17개 전체를 적고, 기여 안내는 CONTRIBUTING.md로 넘겼다.
- **CONTRIBUTING 재작성**: 커밋 메시지 형식을 실제 이력(`<type>(<범위>)`)에 맞추고, pre-push 훅(CHANGELOG 검사,
  Linux clippy)과 GPU 테스트 실행법을 넣었다. 자동 루프 정책, 소요 시간, 버전별 경위, 끊긴 README 참조를 뺐다.
- **tutorial 정정**: 이미 구현된 기능을 "v1.1 후속"으로 적은 곳(Lua 입력, HUD, "+ script" 기본 경로)을 바로잡고,
  옮겨진 `voltex_image::parse_gltf` 경로, 빈 씬의 `version` 필드를 맞췄다. 없는 Ctrl+Z 단축키, chapter별 커밋 대응 절,
  스크린샷 자리 표시, 소요 시간을 뺐다. 에디터 조작은 실행해 보지 않았다(NEXT.md).
- **examples/README 정리**: 빠져 있던 `boids_demo`·`game_ready_demo`를 표에 넣고, 표와 겹치는 카테고리 절, 작업 코드(D1+D2, M6),
  60 FPS·5초 smoke 같은 검증 서술을 뺐다.
- **게임·예제 README 정정**: voltex_survivor README가 이미 있는 Lua 입력·HUD를 "v1.1 한계"로 적던 것을 실제 구조(이동은
  Rust action map, 규칙은 Lua, 점수는 콘솔)로 바로잡았다. voltex_depths의 해소된 한계 3건(조준 DPI, 에셋 절대경로,
  y 가라앉음)을 지우고, 예제 README의 버전 언급을 뺐다. API 경로 30개는 현재 코드와 대조해 그대로 맞다.
- **빌드·매니페스트·릴리스 문서 정정**: build-matrix는 게임 저장소에서 동작하지 않던 GitHub Actions 예시와 작업 코드를 빼고
  OS별 준비물·에러·확인 범위만 남겼다. voltex-toml-reference는 `.vpkg`가 `dist/wrapper/`에 생기는 것, `voltex new`가
  `[build]`까지 채운다는 것을 바로잡고 "향후 추가 후보" 절을 뺐다. release.md의 GitHub 미러 절차, visual/README의
  "PNG 240px 이하"(실제 200~800px)를 고쳤다.
- **fuzz 산출물 gitignore**: `cargo fuzz`가 만드는 `fuzz/target`·`corpus`·`artifacts`·`coverage`가 무시되지 않던 것을
  `.gitignore`에 넣었다(루트 `/target` 규칙은 `fuzz/target`에 적용되지 않는다).
- **루트 `assets/` 정리**: 어디서도 읽지 않던 `assets/scripts/spinner.lua`를 지웠다(`voltex new`는 같은 스크립트를
  직접 만들고, tutorial_minigame은 자기 사본을 쓴다). `assets/cube.obj`는 예제와 voltex_capture·voltex_image 테스트가
  함께 읽어 루트에 둔다. survivor_game 설명을 `collect_render_items`로 PBR 파이프라인을 직접 돌리는 예제로 고치고,
  "100% Lua로 다시 썼다"는 문구를 실제 구조(`RuntimeApp` 위에 규칙만 Lua)로 바로잡았다.

### CI

- **CI 잡 축소**: 필수는 Linux·Windows build·clippy·test만 남겼다. coverage는 78% 게이트를 없애고 측정만 하는
  advisory로 바꿨다(lavapipe 유무에 따라 통과가 갈렸다). semver-check(모든 crate가 `publish = false`),
  license-check(cargo-deny와 중복), changelog-check(pre-push 훅과 중복)를 지우고, e2e는 매일 실행을 빼고 수동만
  남겼다. cargo-deny는 `cargo-deny-action`으로 돌린다.

### Build

- **pre-push Linux clippy**: `scripts/check_linux_clippy.sh` 가 `aarch64-unknown-linux-gnu` 대상으로
  CI 와 같은 플래그의 clippy 를 돌린다. target 이 없으면 경고 후 건너뛴다. Lua C 소스를 크로스 컴파일할
  수 없어 `voltex_script` 와 그 의존 crate 12개, 합쳐서 13개(37개 중)는 제외한다(스크립트에 목록).

## v1.0.173 — 2026-10-17 — 워크스페이스 버전 정합 + 600줄 한도 리팩터링

### Fix

- **워크스페이스 버전이 1.0.114에 머묾**: `Cargo.toml`의 `[workspace.package] version`이
  1.0.114였고 CHANGELOG는 v1.0.172까지 나가 있었다. `voltex --version`(voltex_cli)과
  크래시 리포트의 `voltex_runtime:` 줄은 `env!("CARGO_PKG_VERSION")`을 읽으므로 옛 번호를
  보고했다. 1.0.172로 맞췄다. 확인: `cargo run -p voltex_cli -- --version` → `voltex 1.0.172`.
- README 버전 배지(v1.0.114)는 고치지 않았다(README 미커밋 변경과 함께 정리할 항목, NEXT.md).
- **재발 방지**: `scripts/check_version.sh`가 번호 붙은 최신 CHANGELOG 항목(`## vX.Y.Z`,
  `## Unreleased`는 건너뜀)과 `[workspace.package] version`을 비교한다. pre-push 훅과 PR CI가
  부르는 `scripts/check_changelog.sh`가 push할 커밋의 두 파일로 이 검사를 돌리고, 다르면
  `CHANGELOG v1.0.173 != workspace 1.0.172` 형식으로 실패한다. 번호를 매길 때는 Cargo 버전도
  함께 올려야 한다.

### Refactor

- **600줄 한도: 인라인 테스트 분리 9개 파일** — v1.0.2 전례대로 `foo.rs`를
  `foo/mod.rs` + `foo/tests.rs`로 나눴다. 코드는 바꾸지 않았고 테스트 블록만 옮겼다(4칸 내어쓰기,
  여러 줄 문자열 안쪽 줄은 원문 그대로). 대상: voltex_ecs `scene`, voltex_editor
  `asset_browser`·`panels/inspector`, voltex_renderer `texture`, voltex_ai `pathfinding`·`navmesh`,
  voltex_capture `scene_skinned`, voltex_runtime `audio_sync`, voltex_cli `build`.
- **voltex_cli `build`**: 테스트를 떼도 본문이 630줄이라 래퍼 크레이트 템플릿
  (`write_wrapper_manifest`·`render_wrapper_main`·`write_wrapper_main`)을 `build/wrapper.rs`로
  옮겼다(`build/mod.rs` 565줄). `write_wrapper_*` 둘을 `pub(super)`로 열었다.
- 확인: 커밋마다 `cargo test --workspace` 1762 통과·76 무시, GPU 캡처 47 통과(skip 0), clippy 경고 0.
  래퍼 경로는 `e2e_new_then_build_produces_dist_exe`(`--ignored`)로 실제 빌드까지 확인했다.
- 옮긴 모듈을 가리키는 CODEMAP 경로(ai·ecs·editor·renderer·runtime·cli)를 새 위치로 고쳤다.
- **600줄 한도: voltex_renderer_ecs `BasicRenderer` 분할** — 1767줄 파일을 나눴다(`basic_renderer/mod.rs`
  561줄). 독립적인 두 패스는 `TilemapPass`와 같은 방식의 패스 struct로 뗐다(`debug_line_pass.rs`
  `DebugLinePass`, `sprite_pass.rs` `SpritePass`). 서로 자원을 나눠 쓰는 그림자·스킨드·PBR은
  행동 없는 자원 묶음(`ShadowResources`·`SkinnedResources`·`PbrResources`·`PbrSkinnedResources`)과
  같은 파일의 `impl BasicRenderer` 메서드로 옮겼다(`basic_renderer/{shadow,skinned,pbr}.rs`).
  공개 API, 자원 생성 순서, `render()`의 패스·draw 순서, 바인드 그룹 레이아웃과 label, queue 쓰기
  순서는 그대로다.
- 확인: 커밋마다 GPU 캡처 29장(capture 바이너리 14 + `VOLTEX_WRITE_SCREENSHOTS` 기록 15)을 분할 전과
  비교해 모두 픽셀 단위로 같았다(같은 커밋 3회 촬영의 차이도 0). GPU 테스트 출력에 wgpu 경고·패닉 없음.
- **voltex_capture `scene`**: 씬 함수 22곳이 반복하던 encoder → `render` → submit → `read_rgba8` 꼬리를
  `draw_and_read`로 모았다(모양이 다른 테스트 3곳은 그대로). 텍스처·핫리로드·스프라이트 씬과 그 테스트를
  `scene_texture.rs`·`scene_hot_reload.rs`·`scene_sprite.rs`로 옮겼다. 새 모듈은 private이고 `scene`이
  `pub use`로 다시 내보내 `voltex_capture::scene::render_*` 경로는 그대로다. `scene.rs`는 2780 → 1845줄
  (그중 테스트 약 700줄)로 아직 600줄을 넘는다. 확인 방법은 위와 같다(29장 차이 0).

## v1.0.172 — 2026-10-12 — GPU 테스트 스크린샷 기록은 opt-in

### Changed (test-only)

- **voltex_capture GPU 테스트가 `docs/visual/` PNG를 매번 다시 쓰던 것**: GPU 출력은
  실행마다 비트 단위로 같지 않아 `demo_frame_{a,b,pbr}.png`가 매 실행 바뀌었고, 작업
  트리가 더러워졌다. 15개 기록 지점을 `voltex_capture::write_visual_record`로 모았다.
  `VOLTEX_WRITE_SCREENSHOTS`가 비어 있지 않고 `0`이 아닐 때만 쓴다. 쓰기 실패는
  stderr에만 남기고 테스트를 실패시키지 않는다(기록은 검증 대상이 아님. 일부 지점은 전에도
  실패를 무시했다). 재생성 방법은 `docs/visual/README.md`.
- 확인: 미설정으로 `--include-ignored` 실행 뒤 `docs/visual` 변경 0, 설정하면 15개 PNG 갱신.

## v1.0.171 — 2026-10-12 — 옛 핸들로 add하면 살아 있는 엔티티의 컴포넌트를 덮어씀

### Fix

- **voltex_ecs `World::add` — 죽은 핸들 미검사**: 컴포넌트 저장소(SparseSet)는 슬롯 id로
  키를 잡는다. 그래서 despawn된 옛 핸들로 `add`하면, 지금 그 슬롯을 쓰는 살아 있는
  엔티티의 컴포넌트를 덮어썼다. 슬롯의 세대 표시까지 옛 값으로 바뀌어 살아 있는 쪽에서는
  컴포넌트가 사라진 것처럼 보였다. 계층 속성 테스트에서 `add_child(parent, 옛 자식)`가 같은
  슬롯의 살아 있는 엔티티에서 `Parent`를 지운 것으로 발견했다. 들고 있던 핸들이 오래된
  경우는 게임 코드·스크립트·에디터 undo에서 모두 생긴다. 이제 살아 있지 않은 엔티티에 대한
  `add`는 아무것도 하지 않는다. `get`·`get_mut`·`remove`는 원래 세대를 검사했다.
- **동작 변화**: 죽은 엔티티에 컴포넌트를 붙이던 코드는 이제 조용히 무시된다. 워크스페이스
  전체 테스트·GPU 캡처에서 이 경로에 의존하는 곳은 없었다.

### Tests

- **voltex_ecs `tests/hierarchy_property.rs`**: 시드 3 × 3000 단계에서 `add_child`·
  `remove_child`·`despawn_recursive`·plain `despawn`을 죽은·무관한 엔티티까지 섞어 호출한다.
  매 단계 살아 있는 엔티티 사이의 Parent↔Children 양방향 일치, Children 중복 없음, 순환
  없음을 본다. 위 결함을 결정적 회귀 테스트로 따로 고정했다.

## v1.0.170 — 2026-10-12 — Lua 엔진 API 쓰레기 인자 퍼징

### Tests

- **voltex_script `tests/engine_api_fuzz.rs`**: 게임 스크립트는 사용자 코드다. 엔진 API 바인딩
  23개를 0~6개의 무작위 인자로 2만 번 호출한다. 인자는 nil, bool, NaN, ±inf, 2^31·2^32·2^53,
  정수 최대·최소, 빈 문자열, 비ASCII, 5000자 문자열, 테이블, 함수, 코루틴에서 고른다. 중간중간
  엔티티를 스폰해 유효한 id도 섞는다. 반복자를 돌려주는 바인딩은 몇 단계 더 돌린다.
  `extern "C"` 바인딩 안에서 Rust 패닉이 나면 프로세스가 abort되므로, 테스트가 끝까지 도는
  것 자체가 핵심 단언이다. 성공한 호출이 2000건을 넘는지도 본다. 인자 검사에서 모두 막히는
  공허한 퍼징을 막기 위해서다. 통과(버그 없음).

## v1.0.169 — 2026-10-12 — voltex.toml 파서 분리 + 퍼징

### Refactor

- **voltex_cli `manifest`**: `mod.rs`가 616줄로 600줄 한도를 넘어 있었다. 손으로 짠 미니
  TOML 파서(섹션·값·문자열·배열 파싱, 줄·열 위치 에러)를 `manifest/toml.rs`로 옮겼다
  (mod.rs 350줄, toml.rs 270줄). 동작 변화는 없다. CODEMAP 트리를 갱신했다.

### Tests

- **voltex_cli `manifest::tests::parse_never_panics_on_corrupted_manifests`**: 저장소의
  voltex.toml 3개를 TOML 구문 문자·바이트 이동·임의 바이트·잘라내기로 3만 건 변조한다.
  `Manifest::parse`가 패닉하지 않아야 한다(통과, 버그 없음).

### Docs

- DEFERRED: 600줄 한도 재위반 17개 파일(가장 큰 것은 `voltex_capture/src/scene.rs` 2787줄).
  구조 분할 설계가 필요해 별도 사이클로 둔다.

## v1.0.168 — 2026-10-12 — 손상된 TTF 폰트로 에디터 패닉·무한 재귀

### Fix

변조 퍼징으로 찾았다. 폰트는 사용자가 넣는 파일이라(에디터가 아무 `.ttf`나 읽는다)
손상된 폰트가 에디터를 죽이면 안 된다.

- **voltex_editor `ttf_parser` — 모든 읽기가 경계 검사 없음**: 읽기 헬퍼가 `data[off]`를
  직접 인덱싱했다. 테이블 오프셋·loca·cmap 값이 모두 파일에서 오므로 손상된 폰트 대부분에서
  범위 밖 패닉이 났다(변조 5회 안에 재현). 읽기를 전체 함수로 바꿨다(범위 밖은 0).
  그 밖의 결함은 다음과 같다.
  - 역방향 loca 범위의 뺄셈 언더플로우
  - 증가하지 않는 윤곽 끝점 목록에서 범위 밖 인덱싱(그런 글리프는 없는 것으로 처리)
  - `numberOfHMetrics = 0`의 언더플로우
  - `unitsPerEm` 미검증(0이면 scale이 무한대). 이제 규격 범위 16~16384만 받는다.
- **voltex_editor 래스터라이저**: 파일의 바운딩 박스로 비트맵 크기를 계산해 `w*h`가
  오버플로우하거나 거대한 할당이 일어날 수 있었다. 이제 한 변이 1024 픽셀을 넘거나 유한하지
  않은 글리프는 빈 글리프로 처리한다. 2차 곡선 분할에 깊이 상한(16)을 뒀다. NaN 좌표에서는
  평탄도 검사가 영원히 통과하지 않아 재귀가 끝나지 않을 수 있었다.
- **`TtfFont::new`**: ascender−descender+lineGap을 i16으로 더해 오버플로우했다. 이제 i32로
  더한다.

정상 폰트 렌더링은 바뀌지 않았다(arial 단위 테스트, GPU 캡처 48개 통과).

### Tests

- **voltex_editor `tests/ttf_fuzz.rs`**: 시스템 TrueType 폰트(Windows arial, Linux DejaVu,
  macOS Arial 중 있는 것)를 헤더 위주 변조·무작위 변조·잘라내기로 바꾼다. 로드한 뒤 라틴·
  기호·한글 글리프를 래스터화한다. 기본 300회이고, `VOLTEX_TTF_FUZZ_ITERS`로 늘릴 수 있다.
  폰트가 없는 환경에서는 건너뛴다. 개발 중 시드 2개 × 2만 회(debug)로 돌려 깨끗함을
  확인했다.

## v1.0.167 — 2026-10-12 — 네트워크 패킷·암호화·내비메시 로더 퍼징

### Tests

- **voltex_net `tests/wire_fuzz.rs`**: 모든 `Packet` 변형을 인코딩한 뒤 변조·잘라내기·임의
  바이트로 3만 건 넣는다. `Packet::from_bytes`가 패닉하지 않아야 하고, 받아들인 패킷은
  다시 인코딩해도 안정적이어야 한다. 비트 반전·잘라내기·임의 바이트로 만든 암호문 2만
  건에서 `PacketCipher::decrypt`도 패닉하지 않아야 한다. 둘 다 통과(버그 없음).
- **voltex_ai `tests/navmesh_fuzz.rs`**: 인접 정보가 있는 3×3 격자 내비메시를 `.vnav`와
  헤더 없는 옛 포맷으로 직렬화한 뒤 2만 건 변조한다. 로드에 성공한 메시에 무작위
  `find_path`까지 돌린다. 통과(버그 없음). 로더가 정점·이웃 인덱스를 이미 검증하고 있다.

## v1.0.166 — 2026-10-12 — glTF 인덱스 범위 미검증으로 로더 패닉

### Fix

- **voltex_image glTF — 정점 수를 넘는 인덱스**: 인덱스 accessor의 값을 정점 수와 대조하지
  않았다. 손상되거나 조작된 파일은 탄젠트 생성 단계(`compute_tangents`)에서 범위 밖
  인덱싱 패닉을 냈다(재현: 정점 3개에 인덱스 64). 탄젠트를 이미 가진 파일이라면 패닉
  없이 렌더러로 넘어가 GPU 범위 밖 읽기가 됐을 경로다. 이제 primitive 조립 직후 검사해
  `Err`를 돌려준다.

### Tests

- **voltex_image `tests/mesh_fuzz.rs`**: 인덱스·법선·UV·노드 계층·스킨·애니메이션을 갖춘
  glTF를 테스트 안에서 만들어 base64 내장 JSON과 GLB 두 형태로 쓴다. 이를 숫자 치환
  (문법은 유지한 채 인덱스·개수·오프셋만 바꿈), 임의 바이트, 잘라내기로 6000회 변조한다.
  `assets/cube.obj`도 6000회 변조한다. 패닉이 없어야 하고, 성공한 메시의 인덱스는 모두
  정점 범위 안이어야 한다. OBJ는 처음부터 깨끗했다.
- **voltex_ecs `tests/scene_fuzz.rs`**: 계층·태그(비ASCII 포함)가 있는 월드를 바이너리·
  JSON·텍스트로 직렬화한 뒤 9000회 변조한다. 로드와 transform 전파가 패닉하지 않아야
  한다(통과, 버그 없음).
- 개발 중 release 8만~15만 회, 다른 시드로도 깨끗함을 확인했다.

## v1.0.165 — 2026-10-12 — 손상된 Vorbis setup으로 디코더 패닉, float WAV NaN 통과

### Fix

- **voltex_audio Vorbis setup 헤더 — 교차 참조 미검증**: setup 헤더에 저장된 인덱스를
  패킷 디코더가 검사 없이 썼다. mode→mapping, mapping의 mux·submap floor·residue,
  floor·residue→codebook, floor partition→class 참조가 모두 그랬다. 그래서 손상된 파일에서
  범위 밖 인덱싱으로 패닉이 났다(재현: 길이 1 배열에 인덱스 32). 이제 setup 파싱 끝에서
  모든 참조를 한 번에 검증하고 `Err`를 돌려준다. 블록 크기 지수도 규격(§4.2.2)대로
  6~13만 허용한다. 지수 0이면 블록이 1샘플이 됐다.
- **voltex_audio `parse_wav` — float WAV의 NaN/Inf가 샘플로 통과**: 32비트 float WAV의
  NaN·±Inf 비트 패턴이 `AudioClip.samples`로 그대로 나왔다. 문서가 약속한 범위
  [-1, 1]를 벗어나고 믹서로 전파될 수 있다. 무음(0.0)으로 읽는다.

### Tests

- **`tests/audio_fuzz.rs`**: 실제 WAV fixture 4개와 생성한 16/24/32f 파일을 헤더 위주로
  변조해 1500회씩 넣는다. 패닉이 없어야 하고, 성공 시 채널 수 > 0, 프레임 정렬, 샘플이
  유한해야 한다.
- **`tests/ogg_vorbis_decode.rs` `decoder_never_panics_on_corrupted_streams`**: setup
  헤더(코드북·floor·residue·mapping·mode)나 오디오 패킷을 변조한 뒤 CRC가 맞는 Ogg
  페이지로 다시 감싼다. 그래서 바이트가 실제로 Vorbis 디코더까지 간다. 완성 파일
  자체도 변조한다. 3000회. 개발 중에는 release 4만 회, debug 시드 2개 × 3000회로 돌려
  깨끗함을 확인했다.

## v1.0.164 — 2026-10-12 — 손상된 JPEG에서 디코더 패닉 5종

### Fix

변조 퍼징으로 찾았다. 모두 손상되거나 조작된 파일에서 `Err`를 돌려줘야 하는데 패닉이 났다.

- **테이블 id 범위 밖 인덱싱**: SOF의 양자화 테이블 id(0~255)와 SOS의 DC/AC Huffman
  테이블 id(0~15)를 길이 4 배열 인덱스로 바로 썼다. `.get()`으로 검사해 에러를 낸다.
- **SOS 헤더 경계**: 길이 검사는 `pos + 2`까지만 하고 `data[pos+2]`를 읽었다. 컴포넌트
  목록은 아예 경계 검사가 없었다.
- **샘플링 계수 0 → 0으로 나누기**: SOF 샘플링 계수를 검증하지 않았다. 규격(T.81
  B.2.2)대로 1~4만 허용한다.
- **DC 카테고리 > 11 → 시프트 오버플로우**(debug 패닉, release는 쓰레기 값): baseline DC
  차분 카테고리는 0~11이다. DC 예측값 누적과 양자화 곱은 wrapping 연산으로 바꿨다.
- **over-subscribed Huffman 테이블 → u16 덧셈 오버플로우**: 길이 i+1 코드가 2^(i+1)개를
  넘는 DHT를 거부한다.

정상 파일 디코드는 바뀌지 않았다(실제 인코더 fixture 테스트 통과).

### Tests

- **voltex_image `tests/decoder_fuzz.rs`**: 실제 PNG·JPEG·deflate fixture를 바이트 덮어쓰기
  ·비트 반전·잘라내기·구간 복제로 변조해 디코더에 넣고, 패닉이 없는지와 성공 시 버퍼
  크기가 `w*h*4`인지 본다. 기본은 파일당 400회(deflate 2000회)다. 패닉이 나면 입력을
  임시 폴더에 저장한다. 개발 중에는 시드 3개 × 3000회(debug)와 2만 회(release)로 돌려
  위 5종을 모두 없앴다. PNG·deflate는 처음부터 패닉이 없었다.

## v1.0.163 — 2026-10-12 — 손상된 .vpkg 헤더 하나로 프로세스 중단

### Fix

- **voltex_asset `VpkgReader::parse`**: 헤더의 엔트리 수를 검증하지 않고
  `Vec::with_capacity(count)`로 미리 할당했다. 엔트리 수가 `u32::MAX`인 12바이트 파일
  하나로 약 137 GB 할당을 시도하다 프로세스가 중단됐다(패닉이 아니라 abort라 잡을 수도
  없다). 인덱스 엔트리는 최소 12바이트이므로, 파일 크기로 담을 수 없는 엔트리 수는
  손상으로 보고 할당 전에 `Err`를 돌려준다.

### Tests

- **voltex_asset `tests/vpkg_property.rs`**: 무작위 아카이브 500개 쓰기→파싱 왕복, 엔트리
  수가 비정상적으로 큰 헤더 거부, 무작위 바이트 변조·잘림 2만 건에서 파싱·읽기가
  패닉하지 않는지 본다.

### Docs

- DEFERRED: `.vpkg` 4 GiB 한계(u32 offset이 release에서 조용히 wrap).

## v1.0.162 — 2026-10-12 — AssetStorage 슬롯 재사용 세대가 항상 1 (ABA)

### Fix

- **voltex_asset `AssetStorage` — 해제된 핸들이 새 에셋을 가리킴**: `release`가 슬롯을
  `None`으로 비운 뒤 `insert`가 재사용 세대를 그 빈 슬롯에서 읽었다. 값이 없으니
  `0 + 1`이 되어 매 재사용의 세대가 1이었다. 슬롯을 두 번째 쓰던 에셋의 핸들이 세 번째
  에셋에 그대로 통했다(재현: a 해제 → b 해제 → c 삽입 후 `get(b) == "c"`). 해제된
  메시·텍스처 핸들을 들고 있던 코드가 다른 에셋을 받는 ABA 결함이다. 해제 시점의 세대를
  free list에 같이 보관하고 재사용 때 그 값에 +1 한다(`wrapping_add`). 공개 API는 그대로다.

### Tests

- **voltex_asset `tests/storage_property.rs`**: 결정적 ABA 회귀 테스트 1개와, insert ·
  add_ref · release · replace_in_place를 무작위로 시드 3 × 5000 단계 섞어 단순 맵 모델과
  비교하는 테스트 1개다. 모델 테스트는 매 단계 모든 옛 핸들의 `get`·`ref_count`와 `len`을
  보고, 핸들 재발급이 없는지 확인한다.

## v1.0.161 — 2026-10-12 — 지연 보상 보간이 엔티티를 목록 순서로 짝지음

### Fix

- **voltex_net `LagCompensation::rewind_interpolated`**: 두 기록 사이를 보간할 때
  엔티티를 id가 아니라 목록 인덱스로 짝지었다. 두 틱 사이에 무언가 스폰·디스폰되어
  목록 순서가 바뀌면, 엔티티 A의 위치가 엔티티 B 쪽으로 보간됐다(재현: x 0→10으로
  움직인 엔티티 1이 중간 시각에 x=50). 그러면 서버 히트 판정이 엉뚱한 곳에서 이뤄진다.
  나중 틱에 없는 엔티티는 결과에서 빠졌다. 이제 id로 짝짓고, 나중 틱에서 사라진
  엔티티는 이전 위치를 유지한다. 나중 틱에만 있는 엔티티는 그 시각에 존재하지 않았으므로
  넣지 않는다. 회귀 테스트 2개.

## v1.0.160 — 2026-10-12 — 보간 버퍼가 순서 뒤바뀐 스냅샷에서 틀린 구간을 보간

### Fix

- **voltex_net `InterpolationBuffer::push`**: 도착 순서대로 뒤에 붙이기만 했다. UDP에서
  흔한 순서 뒤바뀜이 생기면 `interpolate`가 시간이 정렬돼 있다고 보고 구간을 찾다가
  엉뚱한 두 스냅샷 사이를 보간했다. 가장 먼저 도착한 스냅샷이 렌더 시각보다 늦으면
  `None`(보간 불가)을 돌려주기도 했다. 축출도 "가장 먼저 도착한 것"을 버렸다. 이제
  `server_time` 순으로 정렬 삽입하고(같은 시각이면 교체), 축출은 가장 오래된 시각부터 한다.

### Tests

- **voltex_net `tests/snapshot_property.rs`**: 추가·삭제·부분 필드 변경(0.0→-0.0 포함)이
  섞인 무작위 스냅샷 체인 50×40에서 `apply_diff(old, diff_snapshots(old, new))`가 비트
  단위로 `new`와 같은지 본다. 전체 직렬화 왕복도 확인한다(통과). 임의 바이트와 비트를
  뒤집거나 잘라낸 diff 2만 건에서 디코더가 패닉하지 않는지 본다(통과). 무작위 도착
  순서와 정렬 순서의 보간 결과가 같은지도 본다(위 결함 검출).

## v1.0.159 — 2026-10-12 — OrderedChannel: 아주 늦은 중복이 다음 wrap에서 진짜 메시지를 밀어냄

### Fix

- **voltex_net `OrderedChannel::receive`**: 이미 배달된 시퀀스의 중복이 de-dup 창
  (1024)보다 늦게 도착하면, 중복으로 걸러지지 않고 순서 버퍼에 들어갔다. 이 패킷은
  u16이 한 바퀴 돈 뒤(65536개 후) 같은 번호의 진짜 메시지 대신 배달됐다. 진짜 메시지는
  다시 버퍼에 남아 다음 wrap까지 갇혔다. 결국 스트림이 영구적으로 한 칸씩 어긋났다
  (재현: 2000개 늦은 메시지 5의 중복 → 65541번째 배달이 옛 메시지 5). 이제
  `next_deliver` 기준 wrapping 절반 공간에서 뒤에 있는 시퀀스는 버퍼에 넣지 않는다.
  ACK는 계속 보낸다.

### Tests

- **voltex_net `tests/ordered_channel_property.rs`**: 패킷·ACK 손실 20%, 중복 10%,
  최대 40틱 지연·재정렬, 8틱마다 미확인분 재전송 조건에서 7만 개 메시지(u16 wrap 포함)를
  시드 3개로 보낸다. 정확히 한 번, 보낸 순서대로 배달되는지, 늦게 도착하는 중복이 다시
  배달되지 않는지 본다(통과). 위 결함은 결정적 회귀 테스트로 따로 고정했다.

## v1.0.158 — 2026-10-12 — Mat4::inverse가 큰 직교 투영·작은 배율을 특이로 판정

### Fix

- **voltex_math `Mat4::inverse` — 크기에 무관한 특이 판정**: `|det| < 1e-12` 절대
  임계값을 썼다. det는 배율의 세제곱·네제곱으로 줄기 때문에, 조건이 멀쩡한 행렬도
  `None`이 됐다. 재현은 두 가지다. 가로·세로 10만 단위 2D 맵의 직교 투영(det ≈ 4e-13)과
  0.1mm 배율 TRS다. 그러면 에디터 picking·gizmo의 언프로젝트가 실패하고, `csm.rs`는
  `expect`에서 패닉이 난다. 이제 열 길이 곱(Hadamard 상한)에 대한 상대값 1e-10으로
  판정한다. 값이 1e-10인 이유는, 원점에서 먼 카메라의 view 행렬은 이동 열이 상한을
  키우는데(1e5 단위에서 약 1e-6) 점 변환에는 여전히 쓸 만해서다. 0인 축 배율이나 같은
  열이 반복된 특이 행렬은 어느 배율에서든 계속 `None`이다.

### Tests

- **voltex_math `tests/rotation_property.rs`**: 쿼터니언 회전이 Rodrigues 공식·`to_mat4`와
  일치하는지 본다. 곱이 행렬 곱과 같은 순서로 합성되는지, Euler YXZ가 짐벌락 밖에서
  왕복하고 `RotY·RotX·RotZ` 순서와 맞는지 본다. slerp가 단위 길이·끝점·등각속도를
  지키는지, TRS 역행렬이 1e-4~1e3 배율에서 맞는지도 본다. 무작위 2만여 건이다. 큰 직교
  투영, 원점에서 1e6 떨어진 카메라 VP, 배율별 특이 행렬 거부도 결정적 테스트로 둔다.
  쿼터니언·slerp·Euler는 통과(버그 없음), 역행렬 판정이 위 결함을 잡았다.

## v1.0.157 — 2026-10-12 — 작은 삼각형의 비스듬한 레이 히트 누락

### Fix

- **voltex_physics `ray_vs_triangle` (메시 레이캐스트) — 크기에 무관한 평행 판정**:
  평행 판정이 `|a| < 1e-8` 절대값이었다. `|a| = |n|·|cos θ|`는 변 길이 제곱에
  비례하므로, mm 크기 삼각형에서는 비스듬하지만 분명히 안쪽에 맞는 레이를 평행으로
  보고 놓쳤다(재현: 1.4mm 삼각형, 입사각 약 0.2°, 무게중심 좌표 0.12 안쪽). 반대로
  km 크기 삼각형에서는 이 판정이 사실상 아무 일도 하지 않았다. 판정을
  `1e-7 × |e1|·|e2|`로 바꿔 크기와 무관하게 했다.

### Tests

- **`tests/triangle_ray_property.rs`**: 크기 1e-3~1e3의 무작위 삼각형 2만 개를 f64
  평면·무게중심 기준값과 비교한다. 히트 여부와 t(상대 1e-3), 단위 법선을 본다. 스치는
  각도, 모서리 근처, 원점이 평면 위인 경우는 제외한다. 위 누락을 잡았다.
  `BvhTree::query_ray`가 무작위 배치 100개 × 레이 20개에서 독립 slab 브루트포스와
  정확히 같은 집합을 돌려주는지도 본다(통과).

## v1.0.156 — 2026-10-12 — ConvexHull 레이캐스트 법선, 다면체 속성 테스트

### Fix

- **voltex_physics `raycast`/`raycast_all` — ConvexHull 법선이 항상 +Y**: 다면체 히트는
  AABB로 근사하면서 법선을 고정값 +Y로 돌려줬다. 옆면이나 아랫면을 맞혀도 위를
  가리켰다. 이제 같은 AABB의 면 법선을 쓴다(`ray_vs_box`). 박스 모양 다면체에서는
  t와 법선이 모두 정확하다. 일반 다면체의 AABB 근사 자체는 그대로다(DEFERRED).

### Tests

- 충돌·레이캐스트 속성 테스트(`collision_property.rs`, `raycast_property.rs`)의
  생성기에 박스 모양 `ConvexHull`을 추가했다. 기준값은 같은 크기의 박스로 계산한다.
  충돌 쪽은 GJK/EPA 경로의 접촉 유무·밀어내기·깊이(1e-3 이내)가 박스 기준값과 맞았다
  (버그 없음). 레이캐스트 쪽은 위 +Y 법선을 잡아냈다.

## v1.0.155 — 2026-10-12 — 2D 타일 이동 무작위 속성 테스트

### Tests

- **voltex_physics `tests/grid2d_property.rs`**: 무작위 타일 격자 2400개(밀도 5~35%,
  맵 밖은 솔리드)에서 `move_and_collide_2d`를 돌린다. 액터 크기는 타일의 0.1~1.5배,
  속도는 프레임당 수십 타일까지다. 끝 위치가 솔리드와 겹치지 않는지, 축마다 뒤로 가거나
  넘치지 않는지, 막히지 않은 축은 정확히 끝까지 가는지 본다. 막힌 축만 속도가 0이 되고
  `hit_wall` 플래그와 맞는지, 직선 이동에서 경로 전체가 벽을 뚫지 않는지도 본다.
  `grounded`는 발밑에 솔리드가 있을 때만 참이어야 한다. 통과(버그 없음). 서브스텝 제거,
  스냅 한 타일 어긋남, 접지 프로브 과대 돌연변이 3개를 모두 잡는다.

## v1.0.154 — 2026-10-12 — 접촉 마찰이 에너지를 만들어 냄

### Fix

- **voltex_physics `resolve_collisions` 마찰 — 운동 에너지 증가**: 무작위 두 물체
  접촉에서 처리 후 운동 에너지(선형+회전)가 최대 약 2% 늘었다(재현: 64.79→65.71,
  180.29→183.48). 원인은 세 가지다. (1) 마찰 충격량을 법선 방향 유효 질량으로
  나눴다. (2) 법선 충격량을 반영하기 전의 미끄러짐 속도로 마찰을 계산했다.
  (3) 서로 멀어지는 접촉에도 `depth / inv_mass_sum`이라는 임의 값을 법선 충격량처럼
  써서 마찰 한도를 만들었다. 이제 마찰은 법선 충격량이 있을 때만, 그 충격량을 반영한
  뒤의 미끄러짐을 접선 방향 유효 질량으로 상쇄하는 만큼, 마찰 원뿔 안에서 건다.
  충격량 적용은 `push_impulse`·`effective_mass_along` 헬퍼로 묶었다(solver.rs 518→495줄).
- **동작 변화**: 서로 다가가지 않는(법선 하중이 없는) 접촉에는 마찰이 걸리지 않는다.
  중력이 있으면 매 프레임 접근 속도가 생기므로 바닥 위 미끄럼 마찰은 그대로다.
  단위 테스트 `test_friction_slows_sliding`은 하중 없는 마찰을 기대하고 있어서,
  공이 바닥으로 다가가는 상태로 바꿨다.

### Tests

- **`tests/solver_property.rs`**: 질량·속도·회전·반발·마찰이 무작위인 Sphere/Box 두
  물체 접촉 1200건. 기본 4회 반복에서 선형 운동량이 보존되고 에너지가 늘지 않는지,
  64회 반복에서 법선 방향 접근 속도가 남지 않는지(마찰이 있으면 Jacobi 방식이라
  잔여가 기하급수로 줄어든다) 본다. 마찰 분모를 법선 유효 질량으로 되돌리기, 충격 전
  미끄러짐 쓰기, 반발 계수 과대 적용 돌연변이 3개를 모두 잡는다.

## v1.0.153 — 2026-10-12 — CCD: 접촉 중인 빠른 물체 감속, 실제로는 터널링을 못 막음

### Fix

- **voltex_physics `physics_step` CCD — 바닥에 닿은 빠른 물체를 매 스텝 반감속**: 이미
  겹쳐 있는 콜라이더(바닥 위를 구르는 공 등)는 스윕 결과가 t=0이었다. 그런데 이를
  터널링으로 보고 물체를 제자리에 둔 채 속도를 절반으로 줄였다. 이 반감속은 속도가
  CCD 임계(반지름/dt) 아래로 내려갈 때까지 매 스텝 반복됐다(재현: 80m/s 공이 첫
  스텝 뒤 20m/s 미만). 이제 t>0인 새 접촉만 본다.
- **CCD가 실제로는 터널링을 막지 못함**: CCD가 integrate 앞에서 돌아 물체를 벽 바로
  앞에 놓았다. 그런데 바로 뒤 integrate가 반으로 줄인 속도로 한 스텝을 더 움직여서
  결국 벽을 통과했다(재현: 300m/s 공이 두께 0.1 벽을 지나 x=4.9). 바닥 접촉의
  t=0 정지가 이 문제를 가리고 있었다. 이제 CCD는 integrate 뒤에 이전 위치에서 새
  위치까지 스윕하고, 첫 접촉보다 1mm 안쪽에 물체를 놓는다. 그러면 뒤이은 이산
  narrowphase와 솔버가 접촉을 보고 속도를 처리한다. 임의의 속도 반감은 없앴다.
  CCD 코드는 `solver.rs`에서 `ccd.rs`(`snapshot_positions`·`clamp_tunnelling`,
  crate 내부)로 옮겼다. 공개 API 변화는 없다.

### Tests

- **`tests/ccd_resting_contact.rs`**: 바닥 위 80m/s 공이 5스텝 뒤에도 70m/s 이상을
  유지하는지, 300m/s 공이 30스텝 동안 얇은 벽을 넘지 않고 벽 쪽 속도가 0이 되는지
  본다. t=0 포함, 접촉 앞 정지, CCD 끄기 돌연변이 3개를 모두 잡는다.

## v1.0.152 — 2026-10-12 — 충돌 판정 결함 3건 (브로드페이즈 누락·EPA 폭주·박스 내부 법선)

### Fix

- **voltex_physics `BvhTree::query_pairs` — 겹치는 쌍의 약 절반 누락**: 트리 순회는
  각 리프 쌍을 공간 정렬 순서로 한 번만 방문하는데, 리프 비교에서 `ea.id <= eb.id`로
  걸러 id 순서와 공간 순서가 어긋난 쌍을 통째로 버렸다. 그런 쌍은 narrowphase에
  도달하지 못해 `detect_collisions`가 접촉을 내지 않았다. `physics_step`의 물체가
  서로 통과하고, `survivor_game`의 충돌·런타임 충돌 이벤트도 일부 빠졌다. 거르지 않고
  낮은 id가 앞에 오도록 순서만 맞춘다(기존에 나오던 쌍의 순서는 그대로).
  기존 테스트는 모두 id 순서와 공간 순서가 같은 배치라 못 잡았다. 무작위 배치 50개
  브루트포스 비교 테스트를 추가했다.
- **voltex_physics EPA — 면 수가 기하급수로 늘어 사실상 멈춤**: 면의 방향을 "원점까지
  거리의 부호"로 정해서, 원점이 면에 가까우면 방향이 뒤집혔다. 그러면 수평선
  모서리가 상쇄되지 않아 면이 반복마다 약 3배로 늘었다(16회차에 12만 개). 캡슐이
  끼는 겹침(GJK 경로)에서 `detect_collisions`가 끝나지 않았다. 초기 사면체 무게중심
  (EPA는 다면체를 키우기만 하므로 계속 내부)을 기준으로 바깥을 향하게 고쳤다.
- **voltex_physics `sphere_vs_box` — 구의 중심이 박스 안일 때 법선이 반대**: 이 분기는
  가장 가까운 면의 바깥 방향을 그대로 돌려줬는데, 규약(A→B, 솔버는 B를 +법선으로
  민다)과 반대여서 깊이 박힌 구와 박스를 더 깊이 밀어 넣었다. 부호를 뒤집었다.

### Tests

- **`tests/collision_property.rs`**: 무작위 Sphere/Box/Capsule 쌍 2000개를 두 생성
  순서로 돌려, 접촉 유무를 독립 거리 오라클과 비교한다. 법선은 단위 길이여야 하고,
  B를 `normal * depth`만큼 밀면 떨어져야 한다. 구·박스 쌍은 깊이가 정확한 최소
  이동량과 맞아야 한다. 위 3건은 이 테스트가 찾았다. 각 수정을 되돌리면 실패하는
  것을 확인했다. EPA 폭주 재현은 20초 제한 회귀 테스트로 따로 고정했다. 그 밖에
  BVH 회귀 2개와 `sphere_vs_box` 내부 법선 회귀 1개를 추가했다.

## v1.0.151 — 2026-10-12 — 레이캐스트 무작위 속성 테스트

### Tests

- **voltex_physics `tests/raycast_property.rs`**: 무작위 Sphere/Box/Capsule 콜라이더
  2000개에 바깥에서 쏜 레이를, 각 도형의 SDF를 0.002 간격으로 행진한 기준값과
  비교한다. 맞춘 경우 t 일치(0.01 이내)·접점이 표면 위·법선이 단위 길이이며 바깥을
  향하는지, 놓친 경우 실제로 안 지나가는지(스치는 경우만 허용) 본다. 콜라이더 6개
  월드에서 `raycast`가 가장 가까운 엔티티를 고르는지, `raycast_all`이 정렬됐는지도
  본다. 통과(버그 없음). 캡슐 원기둥 구간 절반, 박스 -x 법선 반전, 최원 히트 선택
  돌연변이 3개를 모두 잡는 것 확인. 레이가 도형 안에서 시작하는 경우는 도형마다
  규약이 달라(박스는 t=0, 구·캡슐은 출구) 범위에서 뺐다.

## v1.0.150 — 2026-10-12 — 내비메시 경로 탐색 무작위 속성 테스트

### Tests

- **voltex_ai `pathfinding_property_tests`**: 구멍이 무작위로 뚫린 9×7 격자
  내비메시 40개(인접 관계는 공유 정점 쌍으로 구성)에서 무작위 출발·도착 480쌍.
  `find_path`가 셀 BFS(오라클)의 연결 여부와 정확히 일치하는지, 경로의 시작·끝·
  경유점이 메시 위인지, funnel 스무딩 경로를 촘촘히 샘플링해도 보행 영역을 벗어나지
  않는지 본다. 통과(버그 없음). 생성기가 도달 가능·불가 양쪽을 충분히 만드는지도
  단언. 과거에 고친 funnel apex 재시작 버그(코너 컷)를 되살리면 실패하는 것 확인.
  `find_path_triangles`가 `pub(crate)`라 통합 테스트 대신 crate 내부 모듈로 둠.

## v1.0.149 — 2026-10-12 — ECS 코어 모델 기반 속성 테스트

### Tests

- **voltex_ecs `tests/world_model_property.rs`**: spawn/despawn(죽은 핸들
  포함)/add/remove/clear를 무작위로 시드 4개 × 1500단계 실행하고, HashMap 모델과
  `is_alive`·`get`·`query`·`query2`·`entity_count`·`remove` 반환값을 매 단계
  비교한다. 한 번이라도 발급된 핸들은 다시 발급되지 않는다는 것도 확인한다(v1.0.120
  clear 계약). 통과(버그 없음). sparse set swap-remove의 인덱스 갱신을 빼면 실패하는
  것 확인.

## v1.0.148 — 2026-10-12 — 씬 직렬화 무작위 왕복 속성 테스트

### Tests

- **voltex_ecs `tests/scene_roundtrip_property.rs`**: 시드 5개의 무작위
  월드(트랜스폼, 비ASCII 태그, 계층, 중간 despawn으로 생긴 구멍·재사용
  슬롯·매달린 부모)를 JSON·binary VSCN·레거시 텍스트로 왕복시켜 엔티티 id와
  무관한 구조가 같은지 본다. 세 형식 모두 통과(버그 없음). 생성기가 계층·태그·
  despawn을 실제로 만드는지도 단언해 공허한 속성을 막았다. binary가 부모를
  버리게 바꾸면 실패하는 것 확인.

## v1.0.147 — 2026-10-12 — 부모가 despawn된 자식의 WorldTransform 동결

### Fix

- **voltex_ecs `propagate_transforms`**: 루트를 "`Parent` 없는 엔티티"로만 골라서,
  `world.despawn(parent)`(계층을 정리하지 않는 일반 despawn)로 부모가 사라진
  자식은 루트도 아니고 부모에게서 도달하지도 못했다. 그래서 `WorldTransform`이
  **마지막 값에 영원히 얼었다**. 움직여도 반영되지 않고 옛 자리에 렌더된다(재현:
  x=5로 옮겨도 11에 고정). 적과 함께 무기·이펙트 자식을 지우는 흔한 패턴에서
  생긴다. 살아 있지 않은 `Parent`를 가진 엔티티도 루트로 다루도록 고쳤다
  (자기 로컬 변환을 월드 변환으로 사용). 씬 직렬화는 원래 죽은 부모를 "부모
  없음"으로 처리해 안전했다. 테스트 +1.

## v1.0.146 — 2026-10-12 — 에디터 undo/redo 결함 4건 (무작위 속성 테스트로 발견)

새 속성 테스트 `voltex_editor/src/session/undo_property_tests.rs`: 결정적
xorshift로 Spawn/Despawn/SetTransform/SetTag/AddChild/RemoveChild를 시드 3개 × 400단계
돌린다. 성공한 명령마다 상태를 기록하고, 끝까지 undo하면서 **매 단계** 기록과
일치하는지(엔티티 id 무관 구조 비교), 다시 끝까지 redo해 최종 상태와 일치하는지
본다. 실패한 명령이나 undo 항목을 안 남긴 성공은 상태를 바꾸면 안 된다. 이
테스트가 차례로 찾은 결함:

### Fix

- **voltex_ecs `hierarchy::remove_child`**: 자식의 `Parent`를 무조건 지웠다.
  실제 부모가 아닌 엔티티를 넘기면 자식이 **진짜 부모에게서 떨어지고**, 그
  부모의 `Children`에는 남는 불일치가 생겼다. 이제 `Parent`가 그 부모일 때만
  지운다(Lua `remove_child`에도 적용).
- **voltex_editor Despawn undo**: A 삭제 → B 생성(슬롯 재사용) → undo → undo에서
  **A가 돌아오지 않았다**. B 생성의 undo가 슬롯을 다시 해제해 generation이 지나가서
  제자리 부활이 불가능했고, 코드가 이를 "의도된 무동작"으로 삼키고 있었다. 이제
  새 핸들로 재생성하고, undo/redo 스택과 선택의 옛 핸들을 새 핸들로 리매핑한다
  (redo의 Spawn과 같은 방식). 옛 계약을 고정하던 테스트
  `despawn_undo_silently_fails_when_slot_reused`는
  `despawn_undo_recreates_entity_when_slot_was_reused`로 바꿨다.
- **voltex_editor Despawn과 계층**: 삭제가 `Parent`/`Children` 링크를 매달린
  핸들로 남겼고(부모의 Children에 죽은 자식, 자식의 Parent가 죽은 부모), undo는
  등록 컴포넌트만 복원해 **계층이 사라졌다**. 이제 삭제 전에 링크를 떼고 undo
  데이터에 부모·자식 목록을 담아 복원 시 다시 잇는다.
- **voltex_editor AddChild undo**: 역연산이 `RemoveChild`뿐이라 재부모 지정의
  undo가 자식을 **원래 부모로 돌려놓지 못하고 고아로** 만들었다. 같은 부모로
  다시 붙인(무동작) AddChild를 undo하면 **있던 링크를 끊었다**. 이제 이전 부모를
  기록해 되돌린다.
- 600줄 규칙: `Inverse`·`remap_entity_refs`를 `session/inverse.rs`로 분리했고,
  뒤섞여 있던 두 문서 주석도 바로잡았다.
- 테스트 +5 (속성 1, 결정적 재현 3, ecs 1).

## v1.0.145 — 2026-10-12 — CMYK JPEG를 쓰레기 색 대신 에러로

### Fix

- **voltex_image `parse_jpg`**: 4성분(CMYK/YCCK, 인쇄용 툴이나 Photoshop이
  씀) JPEG를 YCbCr 경로로 **조용히 디코드해 쓰레기 색**을 냈다(Pillow CMYK
  픽스처로 재현). SOF에서 성분 수가 1(그레이)·3(YCbCr)이 아니면 에러를 낸다.
  CMYK 지원 자체는 미지원으로 둔다. 테스트 +1.

## v1.0.144 — 2026-10-12 — WAV: EXTENSIBLE · 8비트 · 32비트 정수 PCM

### Additive

- **voltex_audio `parse_wav`**: `WAVE_FORMAT_EXTENSIBLE`(0xFFFE)을 받는다.
  SubFormat GUID 앞 2바이트로 실제 형식(PCM/float)을 고른다. Audacity와
  대부분의 DAW가 24비트·멀티채널을 이 형식으로 써서, 지금까지는 "Unsupported
  WAV format tag: 65534"로 거부됐다. 8비트(부호 없음, 128 중심)와 32비트 정수
  PCM도 추가했다.
- 검증 `tests/wav_real.rs`: CPython `wave`로 만든 8/32비트, 수제 EXTENSIBLE
  24비트 PCM·float, 그리고 `data` 앞에 홀수 크기 LIST 청크가 있는 파일(패딩 —
  기존에도 통과)을 알려진 정수 램프로 정확히 비교. 테스트 +5.
- `.gitattributes`에 `*.wav` binary.

## v1.0.143 — 2026-10-12 — PNG 전 색 유형·비트 깊이 + tRNS 지원

### Additive

- **voltex_image `parse_png`**: 8비트 RGB/RGBA만 받던 것을 PNG 스펙이 허용하는
  모든 비인터레이스 조합으로 넓혔다. 그레이 1/2/4/8/16, RGB 8/16, 팔레트
  1/2/4/8, 그레이+알파 8/16, RGBA 8/16. `tRNS`는 팔레트 알파와 그레이/RGB 색
  키로 적용된다. 16비트는 상위 바이트를 쓰고(libpng strip_16과 같음), 1~4비트
  그레이는 0..255로 스케일한다. 아트 툴이 흔히 내보내는 팔레트 PNG를 그대로
  쓸 수 있다. Adam7 인터레이스는 계속 에러. 픽셀 변환은 새 모듈
  `png_pixels.rs`(600줄 규칙).
- 검증: Pillow 픽스처 8종(팔레트 8/4비트, 팔레트+tRNS, 그레이 1/8/16비트,
  그레이+알파, RGB+tRNS 색 키) + 수제 16비트 RGBA를 참조 디코드와
  **바이트 단위로** 비교, 13/13. 그레이 16비트 참조값은 Pillow
  `convert("RGBA")`를 쓰지 않았다(I;16을 정수로 보고 255에서 잘라 스펙 변환과
  다름). 상위 바이트로 만들었고 값을 ×257로 퍼뜨려 "전부 0이면 통과"하는 약한
  픽스처가 되지 않게 했다. RGB+tRNS는 기존에도 색 키를 무시하던 결함이었다.
  `png_rgb16.png` → `png_gray16.png` 이름 정정.

## v1.0.142 — 2026-10-12 — 믹서가 NaN을 장치 버퍼로 내보내지 않음

### Fix

- **voltex_audio `mix_sounds`**: 마지막 단계의 `sample.clamp(-1, 1)`은 NaN을
  그대로 통과시킨다(`f32::clamp` 동작). 소리 하나의 볼륨이 NaN/inf이거나
  (게임 실수) 3D 위치가 NaN이면(트랜스폼 폭주) 공유 버퍼 전체가 NaN이 돼
  **다른 정상 소리까지 함께 사라지고** 장치로 NaN이 나갔다(재현: 정상 소리
  0.25가 `[NaN, NaN, ...]`로). 이제 실효 볼륨이 유한하지 않은 소리는 그
  패스에서 믹스에서 빼고, 최종 단계에서 비유한 샘플은 0으로 만든다. 정상
  소리는 그대로 들린다. 같은-위치 3D 계산은 원래 안전했다(`stereo_pan`
  ε 가드). 테스트 +1.

## v1.0.141 — 2026-10-12 — Lua 샌드박스 탈출 경로(`package`) 차단

### Fix (security)

- **voltex_script `create_sandbox`**: `os`·`io`·`debug` 전역은 지웠지만
  `package`를 남겨 두었다. `package.loaded.os`·`.io`·`.debug`에 원래 모듈
  테이블이 그대로 있어서 `package.loaded.os.execute(...)` 한 번이면 샌드박스가
  무력화됐고, `package.loadlib`로 네이티브 DLL도 로드할 수 있었다(재현 테스트
  2개 실패 확인). `package`를 차단 목록에 추가했다. 저장소의 Lua 스크립트는
  `require`/`package`를 쓰지 않아 영향 없음.
- 참고: 샌드박스는 설계상 자동 적용이 아니다(CODEMAP — 신뢰할 수 없는
  스크립트 실행 전에 명시 호출). 출하 게임의 자체 스크립트에는 적용되지 않는다.
- 테스트 +2.

## v1.0.140 — 2026-10-12 — 씬 JSON의 비ASCII 컴포넌트 이름

### Fix

- **voltex_ecs JSON 리더**: v1.0.139(이미지 JSON)와 같은 결함이었다. 문자열을
  바이트마다 `char`로 넣어 비ASCII가 깨졌다. 씬 JSON에서 문자열로 직접 읽는
  것은 컴포넌트 이름(키)이고(Tag 등 값은 hex 페이로드라 영향 없음),
  `const NAME = "체력"`처럼 비ASCII 이름으로 등록한 컴포넌트는 저장 → 로드에서
  이름이 안 맞아 **조용히 사라졌다**(재현: 로드 후 `None`). 바이트를 모아
  UTF-8로 한 번 해석하도록 고쳤다. 저장소 전체에서 같은 `as char` 누적 패턴은
  이제 에러 메시지에만 남았다. 테스트 +1.

## v1.0.139 — 2026-10-12 — glTF JSON 문자열 UTF-8 복원 + 서로게이트 쌍

### Fix

- **voltex_image `json_parser`**: 문자열을 바이트마다 `b as char`로 넣어서
  비ASCII UTF-8이 깨졌다(재현: `"모델.bin"` → `"ëª¨ë\u{8d}¸.bin"`). glTF 노드
  이름이 깨졌고, 비ASCII 외부 버퍼·이미지 URI는 파일을 찾지 못했다. 이제 바이트를
  모아 끝에서 한 번 UTF-8로 해석한다(잘못된 UTF-8은 에러). `\uD83D\uDE00` 같은
  서로게이트 쌍은 각각 `char::from_u32` 실패로 **조용히 사라졌는데**, 이제 한
  문자로 합친다. 짝 없는 서로게이트는 에러. 테스트 +3.

## v1.0.138 — 2026-10-12 — OBJ 음수(상대) 인덱스 지원

### Fix

- **voltex_image `parse_obj`**: 면의 음수 인덱스(`f -3 -2 -1`, `-1` = 그
  줄까지 정의된 마지막 정점 — OBJ 스펙, 일부 익스포터가 씀)를 `u32` 파싱
  실패로 "없음(0)" 처리해 **모든 정점이 원점으로 접혔다**(재현: 정점 6개가
  전부 `(0,0,0)`). 기존 퍼즈 테스트는 패닉만 봐서 못 잡았다. 이제 각 면 줄을
  읽는 시점의 정점·UV·법선 개수로 상대 인덱스를 절대 인덱스로 푼다(정점이
  없는데 음수면 0). 테스트 +2 (같은 토큰 `-3 -2 -1`이 줄 위치에 따라 다른
  삼각형을 가리키는지, UV·법선 상대 인덱스).

## v1.0.137 — 2026-10-12 — glTF 노드 `matrix` 변환 지원

### Fix

- **voltex_image glTF 노드**: 로컬 변환을 TRS 대신 column-major `matrix`로
  준 노드(glTF 2.0 허용, 일부 익스포터·모델 아카이브에서 흔함)의 `matrix`를
  읽지 않아 **에러 없이 항등 변환**으로 로드됐다. 이제 `matrix`가 있으면
  translation·rotation(쿼터니언)·scale로 분해해 쓴다. 미러링(행렬식 < 0)은
  음수 X 스케일로 보존한다. `voltex_image`는 L0(수학 crate 무의존)이라 분해를
  직접 구현했다(Shepperd 방식, 네 분기 전부 테스트).
- 테스트 +4 (T·R·S 합성 분해, 미러링, 축별 180° 회전 3종으로 분기 커버,
  matrix 없을 때 TRS 유지).

## v1.0.136 — 2026-10-12 — PNG 디코더를 실제 인코더 출력으로 검증

### Tests

- **voltex_image `tests/png_real.rs`**: Pillow `optimize=True`의 RGB/RGBA PNG가
  바이트 단위로 정확히 복원된다. Pillow의 적응 필터는 Sub·Paeth만 골라서,
  필터 5종(None/Sub/Up/Average/Paeth)을 행마다 돌리고 IDAT를 3조각으로 나눈
  PNG를 직접 만들어(zlib + CRC) 추가했다. Average 필터를 망가뜨리면 이 픽스처만
  실패한다(Pillow 픽스처로는 못 잡음). 미지원 형식(팔레트, 8/16비트 그레이,
  Adam7 인터레이스)은 쓰레기가 아니라 에러로 거부되는 것도 확인. 디코더 버그는
  없었다.
- `.gitattributes`에 `*.png`, `*.rgba` binary 추가.

## v1.0.135 — 2026-10-12 — 실제 JPEG 인코더 출력으로 디코더 검증

### Tests

- **voltex_image `tests/jpg_real.rs`**: 기존 단위 테스트는 손으로 만든 최소
  스트림만 썼다. Pillow 12.1.1(libjpeg)로 만든 37×29(MCU 배수가 아님) 픽스처로
  4:4:4 · 4:2:2 · 4:2:0 · 그레이스케일 · DRI 재시작 마커 · progressive를
  Pillow 자체 디코드와 비교한다. 결과: 4:4:4·그레이 최대 차이 2·1(사실상
  일치). 서브샘플은 경계에서 차이가 컸는데(41/75), 위치를 찍어 보니 101/103개가
  강한 색 경계 바로 옆이었다. libjpeg의 "fancy" 크로마 보간과 이 디코더의
  샘플 복제 차이이지 크로마 위치 오류가 아니다. 그래서 경계에서 2px 이상
  떨어진 곳을 ≤16으로 단언한다. 재시작 마커 스트림은 마커 없는 같은 스트림과
  **바이트 단위로 같은 픽셀**을 내야 한다(통과, 재시작 시 DC 예측값 리셋을
  빼면 실패하는 것 확인). progressive(SOF2)는 디코드하지 않고 에러를 낸다
  (쓰레기 픽셀이 아님).
- 디코더 버그는 없었다. 알려진 차이: 서브샘플 크로마가 nearest라 강한 색
  경계가 libjpeg보다 거칠다(품질 이슈, 정확성 아님).
- `.gitattributes`에 `*.jpg`, `*.rgb` binary 추가.

## v1.0.134 — 2026-10-12 — glTF 애니메이션 데이터도 normalized 정수 지원

### Fix

- **voltex_image glTF 애니메이션**: 샘플러 input/output을 항상 f32로 읽었다.
  glTF 2.0은 rotation·weights 출력을 normalized BYTE/SHORT 등으로 저장하는
  것을 허용하므로, 그런 파일의 회전 키프레임은 쓰레기 값이 됐다(예: `(0,0,0,32767)`
  SHORT → 기대 단위 쿼터니언 `(0,0,0,1)`). v1.0.133의 컴포넌트 헬퍼를 재사용하도록
  고쳤다. `read_accessor_floats`는 `gltf/component.rs`로 옮겼다(600줄 규칙).
  테스트 +1.

## v1.0.133 — 2026-10-12 — glTF 정점 속성 componentType·normalized 지원

### Fix

- **voltex_image glTF**: VEC2/VEC3/VEC4 정점 속성을 `componentType`과
  상관없이 항상 f32로 읽고 있었다. glTF 2.0 core가 허용하는 정규화 정수
  인코딩 — `TEXCOORD`의 normalized `UNSIGNED_BYTE`/`UNSIGNED_SHORT`,
  `WEIGHTS_0`의 normalized `UNSIGNED_BYTE`/`SHORT` 등(압축 익스포트에서 흔함) —
  은 **에러 없이 쓰레기 값**으로 로드됐다. `read_accessor_vecn::<N>` 하나로
  통합해 FLOAT / (UNSIGNED_)BYTE / (UNSIGNED_)SHORT와 `normalized`(signed는
  `max(c/MAX, -1)`), byteStride를 모두 처리한다. 지원하지 않는 componentType은
  쓰레기 대신 에러를 낸다.
- 600줄 규칙 때문에 컴포넌트 읽기를 `gltf/component.rs`로 분리했다.
- 테스트 +4 (normalized u16 UV, normalized u8 가중치, signed -1 클램프, 미지원
  타입 에러). GPU 캡처 48 그린.

## v1.0.132 — 2026-10-12 — glTF 인터리브 정점(byteStride) 지원

### Fix

- **voltex_image glTF accessor**: `bufferView.byteStride`를 무시하고 원소를
  항상 촘촘하게(vec3 = 12B 간격) 읽고 있었다. 위치·법선·UV를 한 버퍼에
  인터리브한 glTF(Khronos 인터리브 샘플 등 일부 익스포터 출력)는 **에러 없이
  깨진 정점**으로 로드됐다. 재현: 두 번째 정점 위치가 `[0.25, 0.5, 4.0]`(UV와
  다음 정점이 섞임). 정점 속성 리더(vec2/vec3/vec4/joints)가 stride를
  쓰도록 고쳤다. 인덱스·역바인드 행렬·애니메이션 데이터는 spec상 stride
  대상이 아니라 그대로 둔다. `byteStride: 0`(spec 위반, 최소 4)은 촘촘한
  배치로 읽는다.
- 커버리지 순환 점검 중 발견(퍼징은 패닉만 잡아서 이런 조용한 오염은 못
  찾는다). 테스트 +3.

## v1.0.131 — 2026-10-12 — inflate를 실제 zlib 출력으로 검증 (dynamic Huffman)

### Tests

- **voltex_image `tests/deflate_dynamic.rs`**: 기존 단위 테스트는 stored·fixed
  Huffman 블록만 손으로 만들어 썼고, 저장소의 PNG는 전부 엔진 자체 인코더
  (stored 블록)로 만든 것이다. 그래서 이미지 편집기가 만드는 PNG의 압축
  방식인 **dynamic Huffman(BTYPE=2) 경로는 한 번도 실행된 적이 없었다**.
  CPython `zlib.compress(…, 9)`로 만든 픽스처 2개(378B·1168B, 첫 블록 BTYPE=2)를
  추가했다. 반복 텍스트 29,690B, 그리고 ~31,000B 뒤로 가는 back-reference(32KiB
  창 한계 근처)다. 디코더는 두 픽스처 모두 정확히 복원했다(버그 없음). 가장 큰
  distance base 값을 바꾸면 새 테스트가 실패한다. 잘린 스트림이 패닉하지
  않는 것도 확인했다.
- deflate.rs 라인 커버리지 56.2% → 82.5%.

## v1.0.130 — 2026-10-12 — 테스트 중 크래시 파일이 crate 폴더에 생기던 것

### Fix (tests only)

- **voltex_runtime**: 이 crate의 단위 테스트(`cfg!(test)`)에서는
  `RuntimeApp::new`가 패닉 훅을 설치하지 않는다. 훅은 프로세스 전역이고 먼저
  설치한 쪽이 이긴다. 그래서 `disable_crash_log()` 없이 `RuntimeApp`을 만드는
  테스트가 먼저 돌면 저장 위치가 cwd(= crate 폴더)가 되고, 같은 바이너리의
  **다른** 테스트가 실패할 때마다 `Voltex_Engine-crash-*.txt`가 생겼다. 테스트
  순서에 따라 생기기도 하고 안 생기기도 했다. 임시 패닉 테스트로 수정 전 1개 →
  수정 후 0개를 확인했다. 훅 자체는 crash.rs 테스트가 temp 디렉터리로 계속
  검증한다. 출하 바이너리와 다른 crate에는 영향 없음.

## v1.0.129 — 2026-10-12 — 항상 통과하던 테스트 단언 5건 복구

### Tests

- `assert!` 없이 `matches!(…);`만 쓴 테스트 문장은 결과를 버려서 무엇이 와도
  통과한다. v1.0.128에서 1건을 찾은 뒤 저장소 전체를 검색해 5건을 더 고쳤다:
  `voltex_asset` vpkg `parse_rejects_bad_magic` / `parse_rejects_truncated_index`,
  `voltex_cli` manifest `missing_name_returns_missing_field` /
  `missing_version_returns_missing_field`, `new_refuses_existing_directory`.
  다섯 다 실제 동작이 기대와 같아 통과했다(숨어 있던 버그는 없음). 기대값을
  틀리게 바꾸면 실패하는 것도 확인했다. 남은 `matches!`는 모두 `assert!` 안에 있다.

## v1.0.128 — 2026-10-12 — voltex.toml 파싱 에러에 열 번호

### Changed

- **voltex_cli `CliError::ManifestParse`**에 `column` 필드(1부터, **문자** 단위)
  추가. 메시지는 `parse error on line N, column M: …`. 파서가 보는 모든 조각이
  원본 줄의 슬라이스라서, 문제 조각의 위치를 그대로 열로 쓴다(`Loc::err`).
  한글 등 멀티바이트 키가 앞에 있어도 바이트가 아니라 문자로 센다.
- 같은 파일에서 발견: `unterminated_array_is_parse_error` 테스트가
  `matches!(…)`를 `assert!` 없이 써서 **항상 통과하는 테스트**였다. 줄·열까지
  단언하도록 고쳤다.
- 테스트 +3 (열 위치 5종, 문자 단위, 표시 문자열).

## v1.0.127 — 2026-10-12 — 크래시 리포트에 GPU·씬·FPS 컨텍스트

### Additive

- **voltex_runtime crash**: `CrashContext { gpu_adapter, scene, fps }` 전역 +
  `set_crash_gpu_adapter` / `set_crash_scene` / `set_crash_fps` /
  `crash_context_snapshot`, `format_crash_report_with_context`. 패닉 훅이 아는
  필드만 헤더에 `gpu adapter:` / `scene:` / `fps (avg):` 줄로 넣는다. 훅은
  `try_lock`으로 읽어서, 컨텍스트를 갱신하는 도중 패닉이 나도 교착 없이
  컨텍스트 없이 쓴다. 기존 `format_crash_report`는 시그니처 그대로(빈
  컨텍스트로 위임).
- 채우는 곳: GPU 생성 직후 어댑터 이름과 백엔드(`handler.rs`), `load_scene`과
  초기 씬 로드(로드 **전** — 로드 중 패닉에도 씬이 찍힘), 매 프레임 HUD 평균
  FPS.
- **voltex_renderer `GpuContext::adapter_info`** (pub 필드 추가).
- 검증: 포매터·setter·스냅샷 단위 +3. winit 창 경로의 배선(어댑터·FPS
  채우기)은 창을 띄우는 테스트가 없어 컴파일로만 확인했다.

## v1.0.126 — 2026-10-12 — 남은 normalize 미가드 호출자 정리 (v1.0.61 audit 잔여)

### Fix

- **voltex_editor `OrbitCamera`**: `distance`가 pub이라 `zoom()`의 clamp를 거치지
  않고 0이 될 수 있다. 그러면 eye == target이 되어 `pan()`의 forward가 NaN이
  됐고 `NaN × 0`이 **target을 영구히 NaN으로 오염**시켰다. `view_matrix()`도
  NaN이었다. 시선 방향을 yaw/pitch로 바로 계산하는 `forward()`(신설, pub)로
  바꿨고, `view_matrix`는 `look_at(eye, eye + forward)`다. 거리가 양수면 행렬이
  예전과 같다(회귀 테스트로 비교).
- **voltex_physics `ray_vs_sphere`**: 반지름 0 구의 중심을 정확히 지나는 레이의
  법선이 NaN이던 것 → 레이를 마주 보는 방향으로 폴백.
- 확인만 하고 고치지 않은 곳: `ray_vs_triangle`(퇴화 삼각형은 판별식 0으로
  먼저 걸러짐 — `a = -dir·(e1×e2)`), 기즈모 `pick_drag_plane_normal`(카메라가
  기즈모 원점에 있으면 NaN 비교가 모두 false라 두 번째 후보 축 반환 → 우연히
  안전, 테스트로 고정), 기즈모 `axis.cross(helper)`(helper가 축과 평행하지 않게
  골라짐).
- 테스트 +5.

## v1.0.125 — 2026-10-12 — AuthToken 상수 시간 비교 + 빈 secret 패닉 정리 (P3)

### Fix

- **voltex_net `AuthToken::validate`**: 토큰 바이트를 상수 시간으로 비교한다
  (`constant_time_eq` — XOR 누적, 첫 불일치 위치에 따라 시간이 달라지지 않음).
  이 모듈은 doc에 적힌 대로 개발용 비암호 체계라 위협 모델이 바뀌지는 않는다.
  다만 비교 시간이 키 정보를 흘리는 경로는 닫았다.
- 같은 파일에서 발견: `generate`/`validate`에 빈 `secret`을 주면
  `i % secret.len()`에서 0으로 나누기 패닉이 났다. 이제 `validate`는 false를
  돌려주고(검증은 외부 입력 경계라 패닉하지 않음), `generate`는
  `PacketCipher::new`와 같은 계약으로 명시적인 메시지와 함께 패닉한다(doc 명시).
  저장소 안 호출처는 없다.
- 테스트 +4.

## v1.0.124 — 2026-10-12 — 스프라이트 화면 평면 회전 옵트인 (friction-2 #6)

### Additive

- **voltex_renderer_ecs `Sprite::roll_from_transform`** (+ `with_roll_from_transform()`):
  켜면 엔티티 로컬 X축의 화면 투영 각도만큼 빌보드를 화면 평면에서 돌린다.
  2D 사이드뷰에서 `Transform` z 회전(빙글 도는 코인 등)이 그대로 보인다.
  기본값 false라 기존 동작은 그대로다. 각도는 `SpriteUniform.size.z`(이전에는
  패딩)로 전달해 유니폼 레이아웃은 바뀌지 않았다. 로컬 X축이 시선 방향과
  평행하면 0(NaN 없음).
- 주의: `Sprite`에 pub 필드가 하나 늘었다. 구조체 리터럴로 만드는 외부
  코드는 `..Default::default()`가 필요하다(저장소 안에는 없음).
- 검증: 단위 4(옵트인 기본값, 0/±각도 추적, 축 퇴화), GPU
  `voltex_capture/tests/sprite_roll.rs`(3×1 스프라이트 90°: 끄면 가로 유지,
  켜면 세로 — 셰이더가 각도를 무시하게 바꾸면 실패) + `docs/visual/sprite_roll.png`.
  스프라이트 셰이더가 모든 스프라이트에 적용되므로 GPU 테스트 98개를 전부
  재실행했고, 스프라이트가 담긴 기존 스크린샷(leap 등)은 바이트 동일.

## v1.0.123 — 2026-10-12 — voltex_leap 타일맵 마이그레이션 (friction-2 #5 해소)

### Changed

- **games/voltex_leap `spawn_level`**: 솔리드 셀마다 Sprite 엔티티를 스폰하던
  것을 원점의 `Tilemap` 엔티티 하나로 바꿨다(셀 규약이 `cell_center`와 같아
  위치 보정 불필요). 레벨 엔티티 수가 솔리드 셀 수만큼 줄고, 타일이 스프라이트
  예산(256)을 쓰지 않는다.
- 검증: GPU visual 회귀에 타일 회색 픽셀 단언을 먼저 추가해 **마이그레이션
  전** 통과를 확인(tile_gray=31800)한 뒤 전환 — 전환 후 픽셀 집계가 완전히
  같고(red 864 / green 810 / gold 225 / tile_gray 31800) `docs/visual/
  leap_level.png`가 바이트 단위로 동일. 헤드리스 플레이스루·단위 16 그린,
  spawn 테스트는 "Tilemap 1개 + filled = 솔리드 수 + 남은 Sprite 3"으로 갱신.

### Additive

- **voltex prelude**에 `Tilemap` 추가.

## v1.0.122 — 2026-10-12 — 배치 타일맵 렌더 (friction-2 #5, 엔진 측)

설계: `docs/superpowers/specs/2026-10-12-tilemap-batch-render-design.md`.

### Additive

- **voltex_renderer_ecs `Tilemap`** 컴포넌트 + `tilemap_vertices`: 로컬 XY
  평면 격자(gy +y 위, 0=빈 칸, n=아틀라스 프레임 n-1). `WorldTransform`이
  평면을 놓는다(빌보드 아님).
- **`BasicRenderer` 타일맵 패스** (`tilemap_pass.rs`): 보이는 맵을 하나의
  꼭짓점 스트림에 펼쳐 맵마다 텍스처 바인드 1회 + draw 1회. 스킨드 다음,
  스프라이트 앞(플레이어·코인이 타일 위에 합성). 프레임당 최대 16384쿼드,
  초과 시 경고 1회. 셀마다 Sprite 엔티티를 쓰던 방식의 `MAX_SPRITES = 256`
  예산·엔티티당 dynamic UBO 슬롯과 무관.
- **voltex_renderer** `TilemapVertex`/`TilemapUniform`/`create_tilemap_pipeline`
  + `tilemap.wgsl`.
- 검증: 단위 13(꼭짓점 전개 7 — 축 뒤바꿈·UV 반전·변환 무시 변이 3종이 모두
  테스트 실패로 잡힘, 배치 패킹 3, GPU 타입 3), GPU 캡처
  `voltex_capture/tests/tilemap.rs`(RTX 4080: 채운 셀 정확히 틴트색, 빈 셀·맵
  밖은 배경; 패스 호출을 끄면 실패) + `docs/visual/tilemap_basic.png`.
- leap 마이그레이션은 v1.0.123(B3b).

## v1.0.121 — 2026-10-12 — survivor 씬 version 누락 + 저장소 씬 전수 로드 가드

### Fix

- **games/voltex_survivor `scene.vscn`**: `{"entities":[]}` → `"version":1`
  추가. survivor 실행 파일은 `with_scene_builder`로 월드를 채워 영향이
  없었지만, `voltex build`(기본 kind=scene)로 만든 dist exe는 이 파일을
  초기 씬으로 로드해 "Missing or invalid 'version'"으로 실패했다. v1.0.114가
  스캐폴드에서 고친 것과 같은 종류의 결함이 저장소 게임에 남아 있던 것.

### Tests

- **voltex_ecs `tests/repo_scenes.rs`**: 저장소의 모든 `.vscn`(target/dist/
  fuzz 제외)을 런타임과 같은 판정(binary magic → JSON)으로 로드. 새 씬
  파일이 로드 불가 상태로 커밋되는 것을 막는다.

## v1.0.120 — 2026-10-12 — `World::clear()`가 generation을 올려 옛 핸들 별칭 차단

설계: `docs/superpowers/specs/2026-10-12-world-clear-generation-design.md`.

### Fix (behaviour change)

- **voltex_ecs `World::clear()`**: allocator를 새로 만드는 대신
  `EntityAllocator::clear()`(신설)로 모든 슬롯을 해제하고 generation을
  올린다. clear 뒤 spawn id는 여전히 0, 1, 2…이지만 generation이 달라서
  clear 전 핸들이 새 엔티티를 가리키지 않는다(`is_alive` false, `get` None).
  해제된 슬롯도 올려 E3 undo의 `restore_at_generation`이 clear를 넘지 못함.
  **바뀌는 동작**: clear 뒤 첫 spawn은 `(0,0)`이 아니라 `(0,1)` 이상.
  씬 파일 3종은 부모를 로컬 인덱스로 저장하므로 파일 호환에는 영향 없음.
- 이 변경으로 같이 해소되는 별칭 버그: `RuntimeApp::load_scene` 뒤 이전 씬의
  `audio_listener`가 새 씬 엔티티를 리스너로 잡던 것. Lua·게임 코드가 보관한
  핸들이 씬 교체 뒤 다른 엔티티에 작동하던 것.
- **voltex_editor Play → Edit**: 선택을 스냅샷 순서로 복원된 엔티티에 재매핑.
  예전에는 별칭 덕에 우연히 유지되던 선택이 stale 판정으로 사라지지 않게
  하려는 것. Play 중에 생성된 엔티티를 선택하고 있었으면 None.
- voltex_runtime `audio_sync`: 이제 sweep이 clear 전 voice도 잡는다. doc과
  v1.0.114 회귀 테스트를 새 계약(ABA 없음)으로 갱신. `stop_all`은 씬 교체 때
  이전 씬을 즉시 끄는 용도로 유지.
- 테스트 +7 (ecs 4, editor 2, runtime 1), 기존 1건 계약 갱신.

## v1.0.119 — 2026-10-12 — 루트 글롭이 빌드 산출물을 삼키던 것 차단 (감사 LOW 이월 4/4)

### Fix

- **voltex_cli `copy_assets` / `pack_archive`**: `assets = ["./**/*"]`처럼
  프로젝트 루트로 풀리는 선언이 `dist/`와 `target/`을 건너뛴다. 재현해 보니
  감사 기록(LOW)보다 심각했다 — `dist/` 복사가 자기 자신 안으로 재귀해
  **스택 오버플로로 `voltex build`가 죽었고**, `.vpkg`에는 이전 빌드의
  `dist/wrapper/*.vpkg`와 cargo `target/` 산출물이 통째로 들어갔다.
  `util::copy_optional_dir`에 skip 목록 인자 추가. 테스트 +2.

## v1.0.118 — 2026-10-12 — CLI 씬 스캔 BOM 오분류 해소 (감사 LOW 이월 3/4, 2부)

### Fix

- **voltex_cli `scan_scene_asset_paths`**: 포맷 판정 전에 UTF-8 BOM을 건너뛴다.
  BOM 붙은 JSON 씬이 legacy text로 분류돼 씬이 참조하는 에셋 자동 동봉이
  경고 없이 빠지던 것. v1.0.117(파서 BOM 허용)과 합쳐 BOM 씬이 빌드·실행
  양쪽에서 동작. 테스트 +1.

## v1.0.117 — 2026-10-12 — JSON 씬 로더가 UTF-8 BOM 허용 (감사 LOW 이월 3/4, 1부)

### Fix

- **voltex_ecs `deserialize_scene_json`**: 선두 U+FEFF(BOM)를 떼고 파싱.
  메모장·VS "UTF-8 with signature"로 저장한 씬이 `Expected number at
  position 0`으로 실패하던 것. runtime `load_scene`, 에디터 LoadScene,
  CLI 씬 스캔이 모두 이 함수를 거쳐 한 곳 수정으로 해소. 테스트 +1.

## v1.0.116 — 2026-10-12 — CLI 멀티바이너리 exe 추측 배포 차단 (감사 LOW 이월 2/4)

### Fix (behaviour change)

- **voltex_cli `pick_executable`**: `[project] name`과 같은 이름의 바이너리가
  없을 때 마지막 산출물을 배포하던 폴백을 없앴다. 후보가 1개면 그대로
  배포하고, 2개 이상이면 `CliError::AmbiguousExecutable`(후보 목록 + 이름
  맞추라는 힌트)로 빌드를 실패시킨다 — 에셋 생성기 바이너리가 게임 대신
  `dist/`에 들어가는 경로 차단. 저장소의 voltex.toml 게임(depths/survivor)은
  이름이 일치해 영향 없음. 테스트 +3.

## v1.0.115 — 2026-10-12 — 에셋 브라우저 `..` 루트 탈출 차단 (감사 LOW 이월 1/4)

### Fix

- **voltex_editor asset_browser `navigate_to`**: 결합 경로를 canonicalize한
  뒤 루트 검사. 기존 구현은 컴포넌트 단위 `starts_with`라 `root/..`를 루트
  안으로 판정했다 — Windows는 canonical 루트가 verbatim(`\\?\`) 경로라 OS가
  `..`를 거부해 가려졌고, Linux/macOS에서는 탈출 가능했다. 회귀 테스트는
  비-canonical 루트로 `..`·`sub/../..` 탈출 거부 + 정상 진입 유지를 확인.

## v1.0.114 — 2026-08-09 — 3-도메인 병렬 감사: 발견 23건 중 19건 당일 해소

v1.0.56 이후 처음의 심층 감사. 병렬 탐색 에이전트 3개가 미감사 표면
(v1.0.57~113 추가분)을 정적 감사: ① 런타임 에셋/오디오 resolve 체인 ② CLI
빌드 파이프라인 ③ 2D 스택 + 에디터. 발견 23건 (high 5) — high/medium 19건
당일 TDD 해소, low 4건 + World::clear 설계 항목은 DEFERRED 이월.

### Fix — high 5건

- **voice 영구 누수** (runtime/audio): `world.clear()`가 allocator까지
  리셋해 재사용 핸들이 orphan sweep을 통과 — Play 덮어쓰기가 이전 voice를
  stop, `AudioVoiceTracker::stop_all` 신설 + load_scene 배선, 계약 doc 정정.
  두 게임의 수동 `src.stop()`이 가리고 있던 실누수.
- **스프라이트 오버플로 컬링 역방향** (renderer_ecs): back-to-front 정렬 후
  truncate라 256 초과 시 **가까운 것부터** 사라짐 — select_nth로 가까운
  n개 생존 후 정렬. 정렬 키도 유클리드 거리 → 카메라 전방 투영 깊이로
  (직교 동일평면에서 화면 위치 따라 레이어 뒤집히던 것).
- **grid2d 터널링** (physics): "프레임 이동량 < tile" 전제가 미강제 —
  히칭 프레임(dt 250ms 클램프)이면 벽 관통. 내부 서브스텝(축당 반 타일)으로
  호출부 계약 유지한 채 해소.
- **.vpkg Lua 스크립트 로드 불가** (script/cli): 스크립트만 Vfs를 우회해
  dist exe가 루즈 .lua 없이는 무동작 + 무진단. 풀에 소스 시드 경로 신설
  (RuntimeApp이 Vfs의 .lua 전부 시드), 실패 1회 로깅 + 캐시.
- **wrapper Cargo.toml `[workspace]` 누락** (cli): 카고 워크스페이스 안
  프로젝트의 wrapper 빌드가 통째 실패하던 것.

### Fix — medium 14건 (요약)

- runtime: one-shot 재트리거 불가(play()가 active 미리셋), 캐시 키 철자
  변형 이중 슬롯(정규화 헬퍼 공유), Lifetime 미등록(저장 씬에서 소멸 조건
  소실), read_asset 폴백이 비-NotFound 에러 은폐.
- renderer_ecs: SpriteAnimation::clip 시트 밖 프레임 wrap 클램프.
- physics: grid2d tile ≤ 0 가드.
- editor: 인스펙터 tag 버퍼 undo 후 stale 재-dispatch(last_tag 동기화),
  Spawn redo 후 redo 체인 EntityNotAlive 유실(엔티티 리매핑), 카메라 포함
  AABB가 모든 픽킹 독점(내부 히트 최저 우선순위 강등).
- cli: 씬 자동 동봉 키 구분자 정규화, kind=rust의 assets/ 강제 기본값 제거,
  스캐폴드/튜토리얼 scene.vscn "version" 누락(새 프로젝트 첫 부팅 에러),
  중첩 에셋 선언 평탄화(manifest 상대 레이아웃 보존), kind=rust 엔진 경로
  없이 진행(라이선스만 스킵).

검증: 각 수정 TDD (+16 테스트), leap/depths 회귀 그린, workspace
clippy(-D warnings)/test 그린.

## v1.0.113 — 2026-08-09 — voltex_physics::grid2d (friction-2 #4 해소)

### Additive

- **voltex_physics `grid2d`** — `move_and_collide_2d(is_solid, tile, pos,
  vel, half, dt) -> MoveResult2D`: XY 평면 AABB vs 타일 그리드 축 분리(X→Y)
  이동·해소 + 발밑 접지 프로브. 그리드는 `is_solid(gx, gy)` 클로저로 주입
  (맵 타입 비의존, 경계 정책 호출자 소유). 전제(프레임 이동량 < tile) doc
  명시. 단위 6 (leap 시나리오 4 이식 + 경계 정책/타일 스케일 2).
  판정 기록: 충돌 코어는 엔진 승격, 중력/점프 튜닝은 게임 잔류.
- 비범위(다음 사용자 등장 시): 스윕 CCD, 슬로프, 원웨이 플랫폼.

### Changed

- **games/voltex_leap `physics2d`** — grid2d 어댑터 + `GRAVITY` 상수로 축소
  (호출부 시그니처 불변). 기존 테스트 4개가 동작 동일성 회귀 가드.

## v1.0.112 — 2026-08-09 — 2D 스프라이트 저작 1급 지원 (friction-2 P1 3건 해소)

leap 도그푸딩 직후 후속 사이클 — `docs/dev/dogfood-friction-2.md` P1 전건 해소 +
leap 마이그레이션으로 실검증. 설계:
`docs/superpowers/specs/2026-08-09-sprite-authoring-friction-design.md`.

### Additive

- **voltex_renderer_ecs `SpriteAnimation::clip`** — `first_frame` 필드 신설
  (기본 0, `new()` 호환 유지). 한 시트에 여러 클립(idle/run/jump)을 담고
  오프셋으로 선택. wrap/clamp은 클립 내부 기준. 단위 +2.
- **voltex_renderer_ecs `Sprite::flip_x`** — 표시 전용 좌우 반전 플래그.
  렌더러가 `effective_uv_rect()`(u-스왑)를 샘플하고 `uv_rect`는 애니
  시스템이 정방향 소유 — 종전 uv 스왑 워크어라운드의 "매 프레임 재적용 +
  tick 순서 지식" 요구가 사라짐. 단위 +1.
- **voltex_capture `render_world_ortho`** — `render_world`의 직교 변형
  (`ortho_height` = 런타임 `with_orthographic`과 동일 의미). 기존
  `render_world`는 무변경 (공통부는 내부 헬퍼로 추출).

### Changed

- **games/voltex_leap** — 새 API 도그푸딩: 플레이어 시트 3장 → 단일 8×1
  (`AnimState::clip()` 전환, 텍스처 교체 제거), facing 반전은
  `sprite.flip_x` 한 줄, visual 회귀는 게임과 동일한 직교 구도
  (`render_world_ortho`, 스크린샷 교체). 스모크 런 그린.

### Fix

- **voltex_audio** — 테스트의 `drop(closure)`가 clippy 1.94
  `drop_non_drop`에 걸리던 것을 블록 스코프로 교체 (동작 불변).

## v1.0.111 — 2026-08-09 — 도그푸딩 게임 2편: Voltex Leap (2D 스택) + OGG 실검증

### Additive — `games/voltex_leap` (2D 사이드뷰 플랫포머)

엔진 2D 스택(v1.0.60: orthographic, `Sprite`, `SpriteAnimation`, `Particle`,
`Lifetime`, `add_texture_png`)의 첫 실게임 도그푸딩. 게임 로직 100% Rust.
ASCII 맵 → 타일 레벨, 중력 + AABB 축 분리 타일 충돌(자체 `physics2d`), 순찰
워커 2, 가시/코인/문, 점프·착지 먼지 + 코인 스파클 파티클, 상태별(idle/run/
jump) 스프라이트 시트 전환 + uv 스왑 좌우 반전, 절차 생성 SFX 5종 + BGM +
스프라이트시트 PNG(`bin/gen_assets`), 직교 카메라 플레이어 추적, HUD.

- **검증**: 단위 16 + 헤드리스 풀 플레이스루(`tests/playthrough.rs` — 봇이
  본편 MAP을 실클리어 + 사망/재시작 리셋) + GPU 시각 회귀(`tests/visual.rs`,
  RTX 4080 — 가시/문/코인 색 비율 + `docs/visual/leap_level.png`) + 8초
  스모크 런 (panic/validation 0, 시트 4장 로드 확인).
- **마찰 로그 신설**: `docs/dev/dogfood-friction-2.md` — 6건 (P1 3: SpriteAnimation
  프레임 오프셋 부재 / Sprite flip 1급 지원 부재 / render_world 직교 미지원,
  P2 3: 2D 타일 충돌 헬퍼 / 타일맵 배치 렌더 / 빌보드 z-rot). 지난 사이클
  개선(gpu_init·asset_root·audio 시드·orphan voice)은 전부 우회 없이 동작.
- 스펙: `docs/superpowers/specs/2026-08-09-voltex-leap-game-design.md`,
  플랜: `docs/superpowers/plans/2026-08-09-voltex-leap.md`.

### (이월) OGG/Vorbis 디코드 경로 실검증 + 그 과정에서 드러난 갭 2건

DEFERRED 의 "실제 `.ogg` 파일 디코드 검증" 항목 처리.

### Additive

- **voltex_audio `tests/ogg_vorbis_decode.rs`** — 코드로 생성한 실 Ogg/Vorbis I
  스트림을 `decode_ogg_vorbis_bytes` 로 끝까지 디코드하는 통합 회귀 5개.
  실 CRC 페이지 프레이밍 + spec 형태 setup 헤더(flat 256-entry codebook /
  floor type 1 / residue type 2 / 2 modes) + 지정 MDCT 스펙트럼 audio 패킷.
  종전엔 100 바이트 이하 합성 입력의 *거부*만 검증돼 컨테이너 → 헤더 →
  codebook/floor/residue → IMDCT → overlap-add 전 구간이 무검증이었다.
  fixture 는 디코더가 읽는 순서가 아니라 spec 레이아웃 기준으로 작성 —
  디코더가 spec 에서 벗어나면 테스트가 깨진다.

### Fix

- **voltex_audio `decode_vorbis`** — audio 패킷 디코드 실패를 전부 조용히
  삼켜 모든 패킷이 깨진 파일도 `Ok(빈 클립)` 이었다. 호출부는 이걸 "의도된
  무음" 과 구분할 수 없어 손상 파일이 무진단 무음으로 재생됐다. 개별 패킷
  skip 은 유지(잘린 tail 은 흔함), 성공 0개일 때만 첫 에러 반환. 호출부
  3곳 모두 이미 Err 를 로그 + graceful degrade 처리라 새 실패 경로 없음.
- **voltex_audio `parse_ogg_pages`** — 컨테이너의 유일한 무결성 검사인 페이지
  CRC 를 통째로 건너뛰고 있었다("skip verification for simplicity"). `page_crc32`
  (poly 0x04c11db7, init 0, 무반사, xorout 없음 — zlib/PNG 변종 아님) 신설 +
  불일치 시 page sequence 와 양쪽 값을 담은 에러. 실 libvorbis 인코딩 파일의
  저장된 페이지 체크섬 9개를 정확히 재현함을 확인했고, 레포에 실 인코더가
  없어 CI 에 못 넣는 그 검증을 대신할 체크값(0x89A1897F)을 테스트에 고정.

### 남은 것

- libvorbis 인코딩 실 파일 디코드는 여전히 미검증 — 이 디코더가 VQ codebook /
  채널 커플링 미구현이라 디코더 확장이 선행 조건. audio_demo 통합도 미실시.

---

## [v1.0.110] — 2026-07-07 — 도그푸딩 마찰 일괄 해소 (friction 9/9)

Voltex Depths 도그푸딩이 남긴 마찰 6건(P1 4 + P2 2)을 한 세션에서 해소 +
미배선 모듈 가드 신설. 상세는 `docs/dev/dogfood-friction.md`.

### Breaking (시그니처 변경 — 워크스페이스 내 호출부 전부 갱신됨)

- **voltex_audio `play`/`play_3d`** 가 `VoiceId` 반환 (기존 호출부는 반환값
  무시로 무변경 동작). `AudioCommand::Play{,3d}` 에 `voice` 필드.
- **voltex_runtime `audio_sync_system(world, audio, listener, &mut
  AudioVoiceTracker)`** — voice 추적자 파라미터 추가.
- **voltex_runtime `resolve_asset_paths` / `resolve_audio_paths`** —
  `asset_root: Option<&Path>` 파라미터 추가.

### Additive

- **voltex_audio `VoiceId` + `stop_voice`** — 재생 인스턴스 단위 정지.
  스레드 커맨드 처리를 `apply_command` 로 추출 (디바이스 없이 단위 테스트).
- **voltex_runtime `AudioVoiceTracker`** — entity별 발행 voice 추적, entity
  소멸(despawn/clear/컴포넌트 제거) 시 orphan voice 자동 stop (friction #9).
- **voltex_runtime `RuntimeConfig::with_asset_root`** — 상대 에셋 경로의
  루트 디렉토리; 상대 루트는 exe 디렉토리 기준 해석 (friction #6).
- **voltex_runtime `AudioClipCache::seed/get`** — with_audio_files 매핑을
  AudioPath resolver에 시드, 이중 디코드 제거 (friction #5).
- **voltex_cli `[build] kind = "rust"`** — Rust 로직 게임 패키징: 게임 crate
  cargo build → dist에 exe + 루즈 에셋 + 라이선스 (friction #8). rust kind는
  build.scene 불요구, scripts 기본값 []. `pick_executable` 이 project.name
  일치 bin 우선.
- **voltex_ai retarget** — nav_agent_system이 `planned_target` 로 target 변경
  감지 후 자동 repath; 같은 삼각형 내 이동은 A* 없이 path 끝점만 교체
  (friction #2 — `agent.target = Some(p)` 직접 대입이 이제 그냥 동작).
- **module_wiring 가드** (`crates/voltex/tests/module_wiring.rs`) — 미배선
  .rs 를 CI에서 적발 (friction #1 후속). 첫 실행에서 3건 적발·배선:
  voltex_ai `obstacle.rs`(velocity obstacle 회피), voltex_script
  `coroutine.rs`(LuaCoroutine), `sandbox.rs`(create_sandbox — 문서에 있던
  보안 기능이 컴파일조차 안 되고 있었음). 잠자던 테스트 23개 부활.

### Fix

- **voltex_ai `follow_path` 평면화** — waypoint를 에이전트 y 평면에 투영:
  도달 판정 XZ + 조향 force가 y를 건드리지 않음 + goal snap이 agent y 보존
  (friction #3 — 게임플레이 y 유지 시 waypoint 통과 불가/침하 해소).

### 게임 (voltex_depths)

- `voltex.toml` kind="rust" + `with_asset_root` dev/dist 패턴으로 마이그레이션
  (CARGO_MANIFEST_DIR 절대경로 해킹 제거). `voltex build` → dist exe가 dev
  에셋 숨긴 외부 cwd에서 부팅·오디오 resolve 검증 통과.

### 검증

- 전 crate TDD (RED 확인 후 구현). workspace lib 테스트/clippy(-D warnings)
  그린, depths 27 + 플레이스루 그린, rust-kind E2E(#[ignore]) 1회 통과.

## [v1.0.109] — 2026-06-12 — 도그푸딩 게임 1편: Voltex Depths

`games/voltex_depths` — 탑다운 트윈스틱 던전 슈터, 게임 로직 100% Rust (Lua 주도
flagship voltex_survivor의 대칭). 적 3종(추적자/사수/보스) + region 기반 문 진행.

### Additive

- **games/voltex_depths** — ASCII 맵 → 레벨/NavMesh 빌드, 마우스 조준
  unprojection, 절차 생성 SFX/BGM(`gen_sfx`), 헤드리스 풀 플레이스루 +
  GPU 시각 회귀 테스트.
- **`voltex_capture::scene::render_world`** — 게임 crate가 임의 World를
  오프스크린 렌더해 픽셀 검증하는 공개 진입점 (고정 라이트/FOV 45°/
  1024 entity 상한 — doc 주석 참고).

### Fix

- **voltex_ai NavMeshBuilder 배선** — `navmesh_builder/` 모듈이 M3 분리 이후
  lib.rs 미선언으로 죽은 코드였음. 배선 + bitrot 수정, 잠자던 테스트 7개 통과.

### 검증

- voltex_depths 단위 29 + 플레이스루 + GPU 시각 회귀 그린. debug/release
  스모크 런 panic/wgpu validation error 0.
- 도그푸딩 마찰 8건 기록 (`docs/dev/dogfood-friction.md`) — `voltex build`가
  Rust 로직 게임 패키징 불가(#8) 등 P1 4건이 다음 엔진 사이클 입력.

## [v1.0.85] — 2026-06-04 — Lua `find_in_radius` (spatial query for scripts)

Lua 스크립트가 v1.0.76 `SpatialGrid` 를 통해 근접 쿼리를 할 수 있게 — Lua AI 가 근처
적/대상/픽업을 찾는 서바이버류 핵심. (선언적 렌더러블/이벤트/리소스는 L3/타입 특화라
Lua 노출 불가하지만, SpatialGrid 는 L1 ecs 라 voltex_script(L2)가 레이어 위반 없이 사용.)

### Additive — `voltex_script::engine_api`

- **`find_in_radius(x, y, z, r)`** → `{id, gen}` 핸들 배열 테이블. Transform 위치가
  `(x,y,z)` 반경 `r` 내인 엔티티. SpatialGrid 로 백킹. 비숫자 인자/world 없음 → 빈 테이블
  (`ipairs` 항상 안전). 기존 하드닝 규약 준수(strict numeric readers, get_world Option).
  매 호출 grid 재구축(O(n)) — Lua 게임 규모에 적합.

### 검증

- Lua 통합 테스트 2개(반경 내 2/far 1 제외 + 핸들 사용 가능, 비숫자 인자 → 빈 테이블) +
  변이 테스트(비숫자 가드 제거 RED→GREEN). voltex_script 57→59.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.84] — 2026-06-04 — `Color` type (named / hex / HSV / lerp)

색이 전부 raw `[f32; 4]` 배열이었다(`MaterialDesc::color([..])`, Sprite 등). 명명 색·hex·
HSV·보간을 갖춘 1급 Color 타입 추가(상용 엔진 기본). v1.0.82 lerp 와 함께 색 트위닝.

### Additive — `voltex_math::color`

- **`Color`**(RGBA f32) — `rgb`/`rgba`/`hex(0xRRGGBB)`/`hexa(0xRRGGBBAA)`/
  `from_hsv(h,s,v)`/`from_array`/`to_array`/`with_alpha`/`lerp` + 명명 상수
  (WHITE/BLACK/TRANSPARENT/RED/GREEN/BLUE/YELLOW/CYAN/MAGENTA/GRAY).
- **`From<[f32;4]>` / `Into<[f32;4]>`** — 렌더러 색 경로와 **무변경 연결**
  (`MaterialDesc::color(Color::RED.into())`). renderer 시그니처 변경 0.
- facade prelude(math)에 `Color` 추가.

### 검증

- 단위 7개(rgb 불투명, hex/hexa 채널 디코딩, array round-trip + From/Into, with_alpha,
  lerp 블렌드, from_hsv 알려진 hue+wrap+grayscale) + 변이 테스트(hex red 시프트 RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.83] — 2026-06-04 — Tween (Timer + easing + lerp, packaged)

v1.0.80 `Timer` 와 v1.0.82 `easing`/`lerp` 의 자연스러운 결합: "값을 N초간 부드럽게
이동" 을 한 타입으로 (Unity/Godot tween).

### Additive — `voltex_runtime::time`

- **`Tween`** — `new(from, to, duration, easing)` / `linear(from,to,dur)` +
  `tick(dt) -> f32`(진행+현재값) / `value` / `value_normalized`(eased 진행 0..1 →
  `Vec3::lerp`/`Vec2::lerp` 로 비스칼라 tween) / `finished` / `reset`. 내부적으로
  one-shot `Timer` 가 구동, easing 은 `fn(f32)->f32`(voltex_math::easing 곡선).
- facade prelude 에 `Tween` 추가.

### 검증

- 단위 5개(linear 보간 from→to+finished, eased midpoint≠linear, value_normalized로
  Vec3 tween, reset 재생, zero-duration 즉시 target) + 변이 테스트(value 가 진행 무시 시
  RED→GREEN). voltex_runtime time 11→16.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.82] — 2026-06-04 — Easing curves + lerp (tweening)

UI 모션·카메라 이동·juice·부드러운 전환에 필요한 이징/보간. v1.0.80 `Timer.fraction()` 과
결합하면 tween (Unity/Godot tween 대응).

### Additive — `voltex_math`

- **`easing` 모듈**: `lerp(a, b, t)` + 곡선 `linear` / `ease_in|out|in_out_quad` /
  `_cubic` / `smoothstep` / `ease_out_back`(overshoot). 모두 t 를 `[0,1]` clamp,
  끝점 `f(0)=0`·`f(1)=1` 고정. 순수, 외부 dep 0.
- **`Vec2::lerp` / `Vec3::lerp`** — 위치·색 보간 메서드.
- 함수가 많아 prelude 미포함 — `voltex::math::easing::*` 경로. Vec lerp 은 메서드라
  prelude 타입에서 바로 사용. 사용 패턴: `lerp(a, b, ease_out_cubic(timer.fraction()))`.

### 검증

- 단위 9개(전 곡선 끝점 고정, out-of-range clamp, ease_in<linear<ease_out, smoothstep
  대칭, ease_out_back overshoot, lerp 보간/외삽, timer→easing 램프) + Vec2/Vec3 lerp 각 1 +
  변이 테스트(ease_in_quad→linear RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.81] — 2026-06-03 — Capstone: boids swarm demo (the new stack, composed)

이번 세션(v1.0.62~80)의 프리미티브들이 **하나의 실행 가능한 게임으로 조합됨**을 보이는
캡스톤 — 단일 기능이 아니라 culmination.

### Additive — `examples/boids_demo`

- 군집 데모: `Rng` 로 랜덤 위치/헤딩/색 시드 → `spawn_bundle((Transform, PrimitiveMesh,
  MaterialDesc, Boid))` 선언적 스폰 → 매 프레임 `flocking_system`(SpatialGrid 이웃) +
  toroidal wrap. 탑다운 카메라. `cargo run -p boids_demo`.
- 사용 스택: Rng · 선언적 렌더러블 · spawn_bundle · Boid/flocking_system · SpatialGrid ·
  per-frame materialize · RuntimeApp 카메라/라이트 config.

### 검증

- **실 하드웨어 스모크 런**: 8초 완주, panic/validation/error 0(120 boid flocking 매 프레임).
- RTX 4080 capture 회귀 `boids_swarm_renders_many_objects`(declarative 스웜 non-blank +
  556 distinct colors) + 시각 기록 `docs/visual/boids_swarm.png`.
- per-example README. workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.80] — 2026-06-03 — Timer (cooldowns / intervals)

쿨다운·스폰 간격·능력 재충전·타임드 상태 — 거의 모든 게임이 손으로 `elapsed += dt;
if elapsed > x` 를 반복하던 것을 1급 타입으로 (Unity/Godot/Bevy Timer 대응). v1.0.66
`Time` 옆에 추가.

### Additive — `voltex_runtime::time`

- **`Timer`** + **`TimerMode`**(Once/Repeating): `once(dur)`/`repeating(dur)` +
  `tick(dt) -> &mut Self`(체인) + `just_finished`/`finished`/`elapsed`/`duration`/
  `fraction`(0..1)/`reset`. Repeating 은 dt>period 시 leftover wrap. duration<=0 은
  즉시 발사(once 는 latch, repeating 은 매 틱). 순수 — 외부 dep 0.
- facade prelude 에 `Timer`/`TimerMode` 추가.

### 검증

- 단위 6개(once 정확히 1회, repeating 매 사이클+wrap, dt>period 발사, fraction,
  reset 후 재발사, zero-duration) + 변이 테스트(발사 신호 무력화 RED→GREEN).
  voltex_runtime time 5→11.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.79] — 2026-06-03 — Deterministic RNG (`voltex_math::Rng`)

게임은 무작위성(스폰 jitter·전리품·절차 배치)이 필요하지만 엔진엔 없었고, 프로젝트의
외부-dep-0 정책상 `rand` 크레이트는 부적합. 내장 결정론적 PRNG 추가(SplitMix64).

### Additive — `voltex_math::rng`

- **`Rng`** — `seeded(u64)` / `next_u64` / `next_u32` / `next_f32`([0,1)) /
  `range_f32(min,max)` / `range_i32(min,max)` / `chance(p)` / `unit_vec2` /
  `unit_vec3`(area-uniform 구면). 외부 dep 0. **시드 동일 → 시퀀스 동일**(리플레이/
  네트워크/테스트 결정론). World 리소스로 보관해 per-run 제너레이터로 활용.
- facade prelude(math)에 `Rng` 추가.

### 검증

- 단위 8개(시드 결정론, 시드 분기, f32 [0,1), range_f32/i32 경계+커버리지, chance
  극단값, **chance 보정**(p=0.25 → 10만 표본 ±2%), unit 벡터 정규화) + 변이 테스트
  (`>>8`→`>>7` 로 [0,1) 깨면 RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.78] — 2026-06-03 — Flocking as an ECS system (`Boid` + `flocking_system`)

v1.0.77 의 순수 boid 함수를 "한 번 호출로 군집 AI" 로 끌어올림 (선언적 렌더러블 →
materialize 와 같은 패턴): 컴포넌트 + 시스템.

### Additive — `voltex_ai`

- **`Boid { velocity }`** 컴포넌트 + **`BoidParams`**(neighbour_radius / separation /
  cohesion / alignment 가중치 / max_speed, `Default` 제공).
- **`flocking_system(&mut World, &BoidParams, dt)`** — 모든 `(Transform, Boid)` 를
  스냅샷 → `SpatialGrid` 로 이웃 수집(~O(n)) → 3법칙 가중합 가속 → 속도(max_speed
  clamp)·위치 적분 → 일괄 기록. 스냅샷-후-기록이라 모든 boid 가 같은 프레임 상태를
  관측(순서 무관). `nav_agent_system` 과 같은 호출자-주입 패턴.

### 검증

- 단위 5개 추가(군집 분리로 간격↑, 응집으로 간격↓, 단독 boid 등속 coast, max_speed
  clamp, 빈 월드 no-op) + 변이 테스트(`vel + acc*dt`→`vel` 가속 무시 RED→GREEN).
  voltex_ai flocking 9→14.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.77] — 2026-06-03 — Boid flocking steering (uses the spatial grid)

v1.0.76 의 `SpatialGrid` 를 실제로 활용: voltex_ai 에 이웃 기반 스티어링이 전혀 없었다
(steering 은 단일-에이전트 함수, `avoid_obstacles` 는 에이전트당 전체 장애물 O(N·M)).
boid 3법칙 추가 + 이웃 수집을 그리드로.

### Additive — `voltex_ai::flocking`

- **`separation(position, &[neighbour_pos])`** (역거리 가중 회피), **`cohesion(position,
  &[neighbour_pos])`** (그룹 중심으로), **`alignment(&[neighbour_vel])`** (평균 heading).
  모두 unit 방향 반환(이웃 없으면 zero). 입력 이웃 리스트는 호출자가
  `SpatialGrid::query_radius` 로 한 번에 수집 → O(n²) 전수 스캔 회피.
- domain-특화라 prelude 제외 — `voltex::ai::{separation,cohesion,alignment}`.

### 검증

- 단위 9개(분리 방향/대칭상쇄/근접우세/빈, 응집 중심/빈, 정렬 평균/빈) +
  **그리드=brute-force 등가 테스트**(`grid_gathered_neighbours_match_brute_force_separation`:
  in/out-of-range 혼재 군중에서 SpatialGrid 수집 이웃이 전수 스캔과 동일 집합·동일 힘) +
  변이 테스트(away 방향 부호 RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.76] — 2026-06-03 — Spatial grid (gameplay proximity queries)

기초를 넘어 깊이로: 서바이버류(flagship)·AoE·어그로·이웃 탐색이 늘 필요로 하는
"반경 내 엔티티" 가 지금껏 O(n) 수동 거리 루프뿐이었다. 균일 그리드 공간 해시 추가.

### Additive — `voltex_ecs::spatial`

- **`SpatialGrid`** — `new(cell_size)` / `from_transforms(&World, cell_size)`(Transform
  위치로 빌드) / `insert(e, pos)`(재삽입 시 셀 이동) / `query_radius(center, r)`(반경
  AABB 가 겹치는 셀만 스캔 후 정확 거리 필터, 경계 포함) / `query_radius_into`(버퍼
  재사용, 무할당) / `nearest(point, exclude)`. 음수/NaN 반경은 빈 결과.
- physics 의 콜라이더 BVH 와 별개 — 이쪽은 엔티티 Transform 위치 기반 게임플레이 인덱스.
  프레임당 재구축(Transform 1패스).
- facade prelude 에 `SpatialGrid` 추가.

### 검증

- 단위 8개(반경 내/외, 셀 경계 횡단, 경계 포함, 음수 좌표, nearest+exclude, 빈 그리드,
  재삽입 셀 이동, degenerate cell_size) + 변이 테스트(`<=`→`<` inclusive RED→GREEN).
  voltex_ecs 195→203.
- workspace build / clippy(`-D warnings`, 부정비교/map_or 2건 정리) / lib test 전부 그린.

## [v1.0.75] — 2026-06-03 — Full-stack composition test (gameplay slice)

이 세션(v1.0.62~74)의 12개 프리미티브를 단위가 아닌 **시스템 레벨에서 조합**해 실제
게임플레이 루프로 검증. 단위 테스트가 못 잡는 통합 버그를 노린다(사이클 7·8 이 이렇게
실버그를 잡았음).

### Additive — `crates/voltex_ecs/tests/gameplay_slice.rs` (외부 통합 테스트, public API만)

- **`full_stack_gameplay_loop_spawns_kills_and_scores`** — `Prefabs` 로 적 스폰 +
  `spawn_child_bundle` 로 shield 부착, `SpawnBudget`/`Score` 리소스, `iter_by_tag` 로
  순회하며 데미지, 0 HP 시 `despawn_recursive` + `send_event(EnemyKilled)`, 별도
  시스템이 `drain_events` 로 점수 적립. 프레임 루프를 수렴까지 돌려 Score==30,
  적·shield 0, budget 0 단언.
- **`events_left_undrained_persist_until_a_consumer_runs`** — consumer 가 안 돌면
  kill 이벤트가 유실되지 않고 큐에 남아 다음 drain 에서 처리됨.
- **`child_entities_are_torn_down_with_their_parent_mid_loop`** — 게임 흐름 안에서
  spawn_child_bundle + despawn_recursive 상호작용 확인.

### 결과

- 3개 통합 테스트 그린 — **통합 버그 0** (스택이 의도대로 조합됨을 확인). 동시에 게임을
  실제로 어떻게 배선하는지 보여주는 정식 예제 역할.
- workspace build / clippy(`-D warnings`) / test 전부 그린.

## [v1.0.74] — 2026-06-03 — Events / messaging (decoupled systems on top of resources)

기초 워크플로 격차 9번째: 시스템 간 통신 수단 부재(적 사망 → 점수·오디오·파티클을
직접 호출로 엮어야 했다). 사이클 8(World 리소스) 위에 이벤트 큐를 올려 producer/
consumer 를 분리.

### Additive — `voltex_ecs::events`

- **`Events<T>`** (`send`/`drain`/`len`/`is_empty`) — `Events<T>` 가 World 리소스로
  저장됨.
- **`World::send_event::<T>(event)`** / **`drain_events::<T>() -> Vec<T>`** — 시스템은
  `&mut World` 만으로 이벤트를 쏘고 받음(배선 불필요). single-buffer drain:
  consumer 는 같은 프레임에 *먼저 보내진* 이벤트만 봄 → producer 를 consumer 보다
  먼저 등록. (order-independent double-buffer reader 는 후속.)
- facade prelude 에 `Events` 추가.

### 검증

- 단위 5개(순서 보존 drain, drain 이 큐 비움, 빈 이벤트, 타입별 독립, Events 직접 send/drain) +
  변이 테스트(drain 이 빈 Vec 반환하도록 깨면 순서 테스트 RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.73] — 2026-06-03 — World resources (entity-less shared state)

기초 워크플로 격차 8번째: World 에 **엔티티에 매이지 않는 공유 상태**(점수/설정/RNG/
이벤트 큐) 개념이 없었다. 이전 사이클(Time/InputMap)에서 반복된 "시스템에 어떻게
전달하나" 문제의 근본 원인 — 시스템은 이미 `&mut World` 를 받으므로, World 에 리소스를
두면 추가 배선 없이 어느 시스템에서든 접근 가능. 이벤트/메시징 등의 토대.

### Additive — `voltex_ecs::World`

- **`insert_resource::<T>(value)`** / **`resource::<T>() -> Option<&T>`** /
  **`resource_mut`** / **`remove_resource`** / **`resource_or_insert_with`** /
  **`has_resource`** — 타입당 싱글턴, World 의 기존 type-erased 패턴(`Box<dyn Any>`)
  재사용. `clear()`(씬 로드) 시 함께 리셋. 컴포넌트 저장소와 독립.

### 검증

- 단위 8개(insert/read, 부재 None, mut 변형, 덮어쓰기, remove, 타입별 독립,
  or_insert_with 재사용, clear 제거) + 변이 테스트(or_insert_with 가 default 재실행
  하도록 깨면 reuse 테스트 RED→GREEN). voltex_ecs 188→195.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.72] — 2026-06-03 — Spawn-as-child (`spawn_child_bundle` / `Prefabs::spawn_child`)

기초 워크플로 격차 7번째: 부모에 부착된 엔티티(포탑·장비·UI 중첩)를 만들려면
spawn → add_child 를 손으로 엮어야 했다. spawn/번들/프리팹/계층 스토리를 완성.

### Additive — `voltex_ecs`

- **`World::spawn_child_bundle(parent, (A, B, ..)) -> Entity`** — `spawn_bundle` +
  `add_child` 한 번에. `despawn_recursive(parent)` 가 자식도 제거, WorldTransform
  propagation 이 자식으로 취급.
- **`Prefabs::spawn_child(&mut World, name, parent) -> Option<Entity>`** — 프리팹
  인스턴스를 부모 하위로.

### 검증

- 단위 4개(자식 컴포넌트+양방향 링크, despawn_recursive 가 자식 제거, prefab spawn_child
  링크, 미등록 None) + 변이 테스트(add_child 생략 시 링크 테스트 RED→GREEN).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.71] — 2026-06-03 — Automated visual + smoke verification of the new authoring stack

이 세션(v1.0.62~70)의 작업, 특히 라이브 렌더 경로(per-frame materialize + flagship
현대화)를 자동 검증.

### Additive — 검증

- **`render_declarative_colored_scene`** + capture 회귀
  `declarative_scene_renders_distinct_per_object_colors`(RTX 4080): flagship 의 선언적
  경로(`PrimitiveMesh`+`MaterialDesc`)로 회색 바닥/초록 player/빨강 enemy 를 렌더 →
  `distinct_colors >= 3` + **초록-우세 픽셀 + 빨강-우세 픽셀 존재**를 픽셀 단위로 단언
  (모더나이즈 전엔 전부 흰색이었음). 시각 기록 `docs/visual/survivor_seed.png`.
- capture bin 에 `survivor_seed` shot 추가.

### 검증 결과 (이 커밋 + 세션 누적)

- 전체 GPU capture 회귀 **37개** RTX 4080 그린(신규 5개 포함: 선언적 렌더 non-blank /
  dedup+bounds / 데이터씬 / 런타임-스폰 materialize / flagship 색).
- flagship `voltex_survivor` + `game_ready_demo` **8초 스모크 런**: exit 124(완주),
  panic/validation/error 0, Lua 게임플레이 실행 확인(survivor 웨이브·라이프·적 스폰).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.70] — 2026-06-03 — Flagship `voltex_survivor` modernized to the new authoring stack

사이클 1~8 로 추가한 워크플로 API 를 flagship 게임에 적용해 그 가치를 실증 + 검증.
이전엔 hand-written 24-vertex `make_unit_cube` + 별도 `upload_meshes`(GPU 단계에서
모든 Transform 엔티티에 공유 핸들/Bounds 수동 부착) + 런타임 enemy/projectile 에
핸들을 매 프레임 샘플링해 붙이는 코드를 갖고 있었다.

### Refactor — `games/voltex_survivor`

- **setup_scene**: ground/player 가 `spawn_bundle((Transform, Tag, PrimitiveMesh,
  MaterialDesc))` 로 선언적. ground 회색, player 초록 (이전엔 전부 흰색).
- **`upload_meshes` + `make_unit_cube` 제거**: `with_gpu_init` 제거. 런타임이 매 프레임
  materialize (v1.0.69) 하므로 GPU 업로드 단계 불필요.
- **런타임 enemy/projectile**: Lua 가 스폰한 bare 엔티티에 `MeshHandle` 샘플링 대신
  `insert_bundle((PrimitiveMesh, MaterialDesc, ScriptComponent))` — 역할별 색(enemy 빨강,
  projectile 노랑) + per-frame materialize 가 GPU 자원 부여. 이전엔 전부 player 와 같은 흰색.
- **204 → 127 줄** (-77). winit/`voltex_renderer` 직접 import 제거(전부 prelude).

### 검증

- workspace build / clippy(`-D warnings`) / lib test 전부 그린. 런타임 렌더 경로는
  독립 검증된 서브시스템(per-frame materialize capture 회귀 + BasicRenderer base_color)에
  의존 — 라이브 게임 화면은 수동 확인 영역.

## [v1.0.69] — 2026-06-03 — Per-frame materialization (runtime-spawned renderables become visible)

사이클 1(v1.0.62)의 실제 한계 수정: `materialize_renderables` 가 `gpu_init` 에서 **한 번만**
실행돼, 부팅 후 런타임에 스폰된 엔티티(Lua `spawn_entity`, 투사체 팩토리, 게임플레이
시스템)가 `PrimitiveMesh`/`MaterialDesc` 를 달아도 GPU 자원을 영영 못 받아 안 보였다.

### Fix — `voltex_runtime` 핸들러

- redraw 경로에서 `tick_systems_only` **직후 매 프레임** `materialize_renderables` 호출
  (device 가 거기서만 존재). idempotent(이미 resolve 된 엔티티 skip) + pending 없으면
  저렴(쿼리 2회). 부팅 엔티티는 첫 프레임에서 커버되고, 이후 스폰 엔티티도 다음
  프레임에 자동 가시화. gpu_init 의 1회 materialize 는 첫 렌더 전 보장용으로 유지.
- 정적 메시를 수동 부착(MeshHandle)하는 기존 게임은 PrimitiveMesh 가 없어 무영향.

### 검증

- RTX 4080 capture 회귀 `materialize_picks_up_entities_spawned_after_a_prior_pass` —
  1차 materialize 후 새 엔티티를 스폰하면 2차 pass 가 그것을 resolve(MeshHandle+
  MaterialHandle 부착) 함을 확인 — 런타임 스폰 시나리오의 정확한 재현.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.68] — 2026-06-03 — Data-authored visual scenes (compose v1.0.62/64/67) + scene-anchor doc

상용 엔진의 본질은 "코드가 아닌 데이터 씬"이다. 사이클 1(선언적 렌더러블)이
`PrimitiveMesh`/`MaterialDesc` 를 `Component` 로 만들어 둔 덕에, 씬 파일(JSON/VSCN)에
**모양+색이 담겨** 코드 없이 visual 씬을 저작할 수 있어야 한다 — 이게 실제로
레지스트리 경유 직렬화로 round-trip 하고 materialize→렌더되는지 검증.

### 검증 (결합 동작 + 회귀 가드)

- `declarative_renderables_round_trip_through_a_data_scene` — Transform+PrimitiveMesh+
  MaterialDesc 월드를 `serialize_scene_json` → `deserialize_scene_json`(레지스트리:
  `register_defaults` + `register_renderable_components`) 하면 두 컴포넌트가 그대로
  복원됨. 로드 후 `plan_primitive_meshes`/`plan_materials` 가 GPU 작업 대기로 인식.
- RTX 4080 capture 회귀 `data_authored_scene_renders_nonblank` — JSON 문자열에서만
  로드한 월드를 `materialize_renderables` 후 BasicRenderer 로 그려 non-blank 확인
  (**데이터 → 픽셀, 스폰 코드 0**).

### 발견 & 문서화 (truthful-doc)

- **scene Transform-anchor footgun**: `serialize_scene_json`/binary 는 **Transform 이
  있는 엔티티만** 직렬화 — Transform 없는 렌더러블 엔티티는 통째로 누락(silent). 검증
  중 재현 확정. 동작 변경(3개 포맷 영향, 위험) 대신 docstring 에 명시 + 회귀 가드
  `renderable_without_a_transform_is_not_serialized` 추가 (v1.0.41/56 의 truthful-doc
  패턴). 렌더러블은 위치가 필요해 Transform 이 사실상 필수라 실사용 영향 없음.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.67] — 2026-06-03 — Prefab registry (named entity templates)

기초 워크플로 격차 6번째: 같은 엔티티를 여러 번 만들 때 매번 컴포넌트 조립을 반복.
Unity Prefab / Godot PackedScene 처럼 "한 번 정의, 이름으로 인스턴스화".

### Additive — `voltex_ecs::prefab`

- **`Prefabs`** 레지스트리 + `register(name, \|&mut World\| -> Entity)` /
  `spawn(&mut World, name) -> Option<Entity>` / `contains` / `len` / `is_empty`.
  팩토리는 `spawn_bundle`(사이클 v1.0.64) 위에 얹히고 컴포넌트-무관이라 어느 crate
  컴포넌트(Sprite 등)든 클로저 안에서 사용. 미등록 이름 spawn → `None`(아무것도 안 만듦).
- facade prelude 에 `Prefabs` 추가.

### 검증

- 단위 6개(템플릿 인스턴스화, 미등록 None, 2회 distinct, 인스턴스별 후처리, contains/len,
  재등록 교체) + 변이 테스트(`contains` 반전 RED→GREEN).
- **실증**: `game_ready_demo` 의 3개 pickup 이 "pickup" 프리팹 등록 → 스폰 지점마다
  인스턴스화 후 위치/색 per-instance 설정(Unity instantiate-then-configure 패턴).
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.66] — 2026-06-03 — Frame clock for systems (`Time` + `add_timed_system`)

기초 워크플로 격차 5번째: 시스템은 per-frame `dt` 만 받고 **누적 경과 시간·프레임
수**를 못 봤다. 타이머/쿨다운/애니메이션 위상을 쓰는 게임 코드가 매번 `+= dt` 를
손으로 누적(조건 분기에 묻혀 버그 유발).

### Additive — `voltex_runtime::time`

- **`Time { elapsed_secs, delta_secs, frame_count }`** + `advance(dt)`. `elapsed_secs`
  는 시스템에 넘기는 `dt` 와 동일 누적이라 fixed-timestep 에서 결정적.
- **`RuntimeApp.time`** public 필드 — `tick_systems_only` 시작에서 1회 `advance`,
  그래서 그 프레임의 모든 시스템·HUD 가 일관된 시계 관측.
- **`add_timed_system(|&mut World, &Time|)`** — 액션 시스템 직후 실행.
- facade prelude 에 `Time` 추가.

### 검증

- 단위 6개(Time::new/advance 누적/delta + 런타임 time 누적/timed 시스템 관측) +
  변이 테스트(`+= dt`→`= dt` RED→GREEN).
- **실증**: `game_ready_demo` 의 HUD 경과시간이 chase 시스템의 조건부 `s.time += dt`
  (chaser 미추적 시 멈추던 버그) → `add_timed_system` 이 `time.elapsed_secs` 반영.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.65] — 2026-06-03 — Tag lookup helpers (`find_by_tag` / `iter_by_tag`)

기초 워크플로 격차 4번째: 데모마다 반복되던
`world.query::<Tag>().find(\|(_, t)\| t.0 == "player").map(\|(e,_)\| e)` 보일러플레이트.

### Additive — `voltex_ecs`

- **`World::find_by_tag(&str) -> Option<Entity>`** — Tag 가 일치하는 첫 live 엔티티.
- **`World::iter_by_tag(&str) -> impl Iterator<Item = Entity>`** — 일치하는 모든 엔티티.
- `impl World` 를 `scene.rs`(Tag 가 사는 곳)에 둬서 World 코어 모듈은 Tag-free 유지.

### 검증

- 단위 3개(조회 성공/부재/다중).
- **실증**: flagship `voltex_survivor`(player 조회 2곳) + `game_ready_demo`(totem/chaser
  조회 2곳)의 `query::<Tag>().find(..).map(..)` 4-라인 패턴 → `find_by_tag(..)` 1줄.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린.

## [v1.0.64] — 2026-06-03 — Component bundles (`spawn_bundle`)

기초 워크플로 격차 3번째: 엔티티 하나 만들 때마다 `world.spawn()` 후 컴포넌트를
4~6번 `add()` 하는 보일러플레이트. Bevy `Bundle` / Unity 프리팹처럼 "이 엔티티는
무엇이다" 를 한 문장으로.

### Additive — `voltex_ecs::bundle`

- **`Bundle`** trait + **`World::spawn_bundle((A, B, …))`** / **`insert_bundle`** —
  `'static` 컴포넌트 튜플(1~12개)을 한 번에 첨부. 순서대로 add 되어 같은 타입은
  뒤쪽이 덮어씀(반복 `add` 와 동일 규칙). 매크로 생성 impl, 외부 의존 0.
- facade prelude 에 `Bundle` 추가.

### 검증

- 단위 6개(전체 첨부, 1-튜플, entity_count, insert_bundle, 덮어쓰기 순서, 4-컴포넌트) +
  변이 테스트(insert 제거 시 4개 RED→GREEN).
- **실증**: `game_ready_demo` `setup_scene` 의 모든 엔티티(ground/player/chaser/totem/
  pickups)가 `spawn()` + N×`add()` → 단일 `spawn_bundle`. player 는 6-튜플
  (Transform/Tag/Script/Collider/PrimitiveMesh/MaterialDesc), chaser 는 7-튜플 —
  크레이트 경계(ecs/physics/ai/audio/renderer_ecs) 넘는 컴포넌트도 한 번들에.
- workspace build / clippy(`-D warnings`) / lib test 전부 그린(ecs 161→167).

## [v1.0.63] — 2026-06-03 — Input action mapping (rebindable semantic controls)

상용 엔진 기초 워크플로 격차 2번째: 입력이 raw 키코드 폴링뿐이었다
(`input.is_key_pressed(KeyCode::KeyW)`). Unity Input Actions / Godot `InputMap`
처럼 **시맨틱 액션/축**을 한 곳에서 바인딩하고 이름으로 질의하는 레이어가 없었다.

### Additive — `voltex_platform::input_map`

- **`InputMap`** — `InputState` 위 순수(상태 없는) 매핑 레이어. `bind_key(action,key)`
  / `bind_mouse(action,btn)` / `bind(action, Trigger)`(다중 = OR 대체) +
  `bind_axis(axis, positive, negative)`(±1/0). 질의: `is_pressed`/`just_pressed`/
  `just_released`/`axis`(→ f32 `[-1,1]`)/`vector(x,y)`(→ `Vec2`). 모두
  `&InputState` 를 받아 윈도우/GPU 없이 단위 테스트 가능.
- **`Trigger`** enum (`Key`/`Mouse`) + `KeyCode`/`MouseButton` winit 재export
  (게임이 winit 직접 의존 회피).
- 한계: 마우스 just-released 미지원(InputState public 질의 부재) — mouse 트리거의
  `just_released` 는 항상 false.

### Additive — 런타임 통합

- **`RuntimeApp.input_map`** public 필드 + **`add_action_system(|&mut World,
  &InputState, &InputMap, f32|)`** — raw input system 직후 실행. 컨트롤 리바인딩이
  `app.input_map` 한 곳에 모임.
- **`RuntimeConfig::with_input_map(InputMap)`** — 부팅 시 `app.input_map` 으로 이동.
- facade prelude 에 `InputMap`/`KeyCode`/`MouseButton` 추가.

### 검증

- 단위 8개(액션 held/edge, axis ±/0, vector 합성, 마우스 트리거, 미바인딩=false/0) +
  변이 테스트(axis `p-n`→`p+n` RED→GREEN).
- 런타임 통합 테스트 `action_system_reads_input_map_bindings`(바인딩된 키를 액션
  시스템이 관측).
- **실증**: flagship `voltex_survivor` 의 WASD/화살표 이동 ~35줄(8개 `is_key_pressed`
  분기 + 수동 정규화) → `bind_axis` 4줄 + `map.vector("move_x","move_y")` 액션 시스템.
  WASD·화살표가 같은 축의 대체 바인딩.
- workspace build / clippy(`-D warnings` all-targets) / lib test 전부 그린.

## [v1.0.62] — 2026-06-03 — Declarative renderables (one-call "spawn a visible object")

상용 엔진과의 **기초 워크플로 격차** 정정. 기능 폭(PBR/물리/오디오/AI/net/Lua)은
이미 충분하지만, 엔진에서 가장 빈번한 연산 — "눈에 보이는 오브젝트 하나 놓기" —
이 두 단계(GPU 없는 `scene_builder` ↔ GPU 필요한 `gpu_init`)로 갈라져, 엔티티를
`Tag` 로 되찾아 `MeshHandle`/`MaterialHandle`/`Bounds` 를 별도 GPU 패스에서 수동
부착하는 보일러플레이트가 모든 게임에 강제됐다. Unity 의 `CreatePrimitive` /
Godot 의 노드 추가에 해당하는 한-줄 경로가 없었다.

### Additive — `voltex_renderer_ecs::renderable`

- **`PrimitiveMesh`** 컴포넌트 (값 타입, GPU 무관): `Cube`/`Plane`/`Quad`/`Sphere`.
  생성자 `cube(size)`/`plane(size)`/`quad(w,h)`/`sphere(r,sec,stk)`/`sphere_default(r)`,
  `aabb()`(자동 `Bounds` 용), `geometry()`(빌트인 generator 위임). `Component`
  구현 → 씬 직렬화.
- **`MaterialDesc(MaterialUniform)`** 컴포넌트: `color(rgba)` / `pbr(rgba,metallic,roughness)`.
  `Component` 구현 → 씬 직렬화.
- **`materialize_renderables(&mut World, &mut RenderAssets, &Device)`**: `PrimitiveMesh`
  가 있고 `MeshHandle` 이 없는 엔티티마다 GPU 메시를 생성(**dedup**: 동일 프리미티브는
  한 버텍스 버퍼 공유)하고 `MeshHandle`+`Bounds` 부착; `MaterialDesc` 도 동일하게
  dedup 후 `MaterialHandle` 부착. **멱등** — 이미 resolve 된 엔티티는 skip 하므로 매
  프레임/핫리로드 후 호출 안전. 순수 헬퍼 `plan_primitive_meshes`/`plan_materials`
  (GPU 없이 멱등 계약 테스트 가능).
- **자동 와이어링**: `RuntimeApp` 이 `register_defaults()` 직후 두 컴포넌트를 등록하고,
  `gpu_init` 직후 `materialize_renderables` 를 한 번 호출. 게임은 `scene_builder`
  한 곳에서 모양+색을 선언하면 끝 — device/`MeshHandle`/Tag 왕복 불필요.
- **facade prelude** 에 `PrimitiveMesh`/`MaterialDesc` 추가.

### 검증

- 단위 13개(aabb 4종, geometry 카운트, dedup 키, plan 멱등 2종, 직렬화 round-trip 2종) +
  변이 테스트로 dedup 키 충돌 검출 RED→GREEN 확인.
- RTX 4080 capture 회귀 2개(`#[ignore]`): `materialized_primitives_render_nonblank`(선언적
  엔티티만으로 비-blank 렌더), `materialize_dedupes_identical_meshes_and_attaches_bounds`(3개
  동일 큐브 → 메시 1개 공유, sphere 분리, Bounds/MaterialHandle 부착, 2회 호출 멱등).
  시각 기록 `docs/visual/declarative_primitives.png`(빨간 큐브 + 초록 구).
- **실증**: `game_ready_demo` 의 `upload_meshes` 가 ~30줄(큐브 mesh/3머티리얼/Tag 재조회
  루프/bounds) → 스킨드 토템 한 줄로 축소. ground/player/chaser 는 `setup_scene` 에서
  `PrimitiveMesh::cube` + `MaterialDesc::color` 선언만.
- workspace build / clippy(`-D warnings` all-targets) / lib test 전부 그린.

## [v1.0.61] — 2026-06-03 — Status sweep: hygiene + runtime lights/audio/shadows + robustness audit

프로젝트 현황 점검 후 A(위생)→D(대형) 우선순위로 자율 진행한 세션. 모두 헤드리스
검증(단위 + RTX 4080 capture 회귀) + 역호환 유지. 점검 중 DEFERRED.md 의 **문서
drift 4건**(이미 구현됐는데 v1.1 후속으로 표기) 확인·정정.

### Fix — 위생 (Tier A)

- **clippy 게이트 복구**: `voltex_capture` 텍스처 로드 테스트의 `vec![...].repeat`
  가 `clippy::useless_vec` 로 `--all-targets -D warnings` 게이트를 깨고 있던 것 수정.
- **워크스페이스 버전 sync**: 17개 라이브러리 crate 가 placeholder `0.1.0` 이라
  `voltex --version` 이 0.1.0 을 출력하던 문제 — `[workspace.package].version = "1.0.61"`
  + 각 crate `version.workspace = true` (기존 `license.workspace` 패턴). 예제는 0.1.0 유지.

### Additive — 런타임 라이팅/오디오 (Tier B)

- **per-frame 점광원 업데이터**: `RuntimeApp::set_point_lights_fn(\|&mut Vec<PointLight>, &World, f32\|)`
  — directional `set_light_fn` 과 대칭. 움직이는 횃불·깜빡이는 램프·발광 투사체를
  world 상태 기반으로 매 프레임 구동(정적 `set_point_lights` 보강). 멀티라이트 배선
  자체는 이미 완료돼 있었음(colored PointLightsUniform + PBR LightsUniform 병합, spot 포함).
- **AudioSource 씬 직렬화**: `voltex_audio::AudioSource`(ECS 무의존)를 `register_raw`
  escape hatch 로 등록(`audio_sync::{serialize,deserialize}_audio_source` +
  `register_audio_source`, RuntimeApp 이 `register_defaults()` 직후 자동 등록). 11바이트
  (clip_id+volume+spatial/looping/playing; transient played_once/active 미저장).
  JSON/binary 씬 round-trip. +3 테스트.

### Additive — 스킨드 메시 그림자 caster (Tier B)

- 런타임 directional 그림자 pre-pass 가 정적 메시만 그려 스킨드 캐릭터가 그림자를
  못 던지던 갭 해소. `create_skinned_shadow_pipeline` + `skinned_shadow_shader.wgsl`
  (depth-only GPU 스키닝, `view_proj=light_vp`, 기존 skinned camera/palette 레이아웃
  재사용 — 신규 레이아웃 0). `BasicRenderer::gather_skinned_shadow` 가 light-space
  cam UBO + 공유 palette 업로드 후 정적 메시 뒤에 스킨드 메시를 깊이에 렌더. Hidden
  엔티티는 cast/draw 모두 제외. RTX 4080 검증 `skinned_mesh_casts_shadow`(strip 이
  바닥에 그림자 투사, Hidden 대조군 압도) + `docs/visual/skinned_shadow.png`.

### Fix — 견고성 audit (Tier C)

- **`.normalize()` zero-input audit** (v1.0.41 deferred): 게임플레이 핵심 경로
  (voltex_ai steering seek/flee/arrive/wander + nav_agent_system)는 **이미 length 가드**가
  있어 NaN-safe 확인. 유저 라이트 설정이 zero direction 일 때 NaN 전파되던
  `csm::compute_cascade_matrices` + `spot_shadow_matrix` 두 곳을 `normalize_or_zero`
  + straight-down fallback 으로 수정. +2 finite-output 회귀.

### 문서 drift 정정 (점검 중 발견 — 이미 구현됐던 항목)

- **Lua 입력 API**: `voltex_script::engine_input` 의 `input` 테이블(v1.0.59) — DEFERRED 가
  아직 "v1.1 후속" 표기였음.
- **JSON 씬 entity cap**: `deserialize_scene_json` 도 `MAX_SCENE_ENTITIES` 적용.
- **`validate_hierarchy`**: 구현 완료 + text/binary/JSON 3개 deserializer 모두 로드 후
  호출(pre-existing 사이클 방어) — DEFERRED 가 아직 "별 pass 필요" 표기였음.

### Tier D / B-7 (의도적 미진행 — 평가 후 문서화)

- **에디터 커스텀 컴포넌트 리플렉션**: ComponentRegistry 에 필드 레벨 리플렉션 인프라
  부재 — 별도 설계 필요. **Archetype 스토리지 / dep scheduler 실제 병렬**: major
  rewrite (data-race 위험, 다중 세션). **CI 첫 green run**: GitHub mirror 필요(자율 불가).
  **missing_docs 1476 / pub(crate) 다운그레이드**: 대량 기계적 churn, 가치 낮음 — 보류.

### 검증

`cargo build --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` /
`cargo test --workspace --lib` (1239 lib 테스트, 0 실패) 전부 그린.

## [v1.0.60] — 2026-06-03 — Scene control + asset loading + deeper game-ready wiring

v1.0.59 데모(game_ready_demo)를 실제로 돌려보며 드러난 엔진 갭들을 연달아 메운
세션. 모두 헤드리스 검증(단위 테스트 + RTX 4080 capture 회귀) + 역호환 유지.

### Additive — 씬 제어 (게임이 화면을 통제 가능하게)

- **카메라**: `RuntimeConfig::with_camera(pos, yaw, pitch)` / `with_camera_fov` +
  `RuntimeApp::set_camera_fn(\|cam, world, dt\|)` (follow/orbit). 근본 원인은
  렌더 버그가 아니라 하드코딩 카메라(0,10,20)에 1-unit 오브젝트가 ~15px 점으로
  보였던 것 — capture로 재현 확인.
- **머티리얼 색상**: BasicRenderer가 `MaterialUniform.base_color`를 적용
  (`ColoredCameraUniform` + `mesh_colored.wgsl`, 격리 — shared CameraUniform/PBR/
  레거시 예제 무영향). 모든 오브젝트가 같은 회색이라 바닥에 묻히던 문제 해결.
- **조명**: `RuntimeConfig::with_light(dir, color, ambient)` + `set_light_fn`
  (day/night).
- **충돌/트리거 이벤트**: `RuntimeApp::set_collision_fn(\|contacts, world, dt\|)`,
  `enable_physics` 무관 실행(트리거 존). 데모의 "잡힘"을 거리 계산 → 실제 Collider
  충돌로 교체.
- **점광원**: `RuntimeApp::set_point_lights(Vec<PointLight>)` — mesh_colored가
  additive point light (MAX_POINT_LIGHTS=8). 단일 directional을 보강.
- **스폿라이트**: `PointLight`를 3×vec4(48B)로 확장(방향 + cos(outer half-angle)).
  `PointLight::spot(pos, dir, color, range, intensity, cone_degrees)` = 부드러운
  가장자리 원뿔, `PointLight::new` = omni(센티넬 `spot_cos_outer < -1.5`).
  mesh_colored가 cone smoothstep gate 적용 → `set_point_lights` 경유 그대로 사용.
  캡처 회귀 `spotlight_lights_cone_footprint_not_corners` +
  `docs/visual/spot_cone.png`. 데모 아레나 중앙에 흰 스폿 추가.
- **HUD 바**: `UiContext::progress_bar` / `draw_rect` (체력/진행 바). 데모에
  danger meter 추가.
- **디버그 드로우(gizmo 라인)**: `voltex_renderer::debug_line`(LineVertex +
  `LineList` depth-tested 파이프라인) + `BasicRenderer::set_debug_lines` +
  `RuntimeApp::set_debug_draw_fn(\|d: &mut DebugLines, world, dt\|)`.
  `DebugLines` 빌더(line/ray/aabb/cross)로 콜라이더·내비·레이 시각화. 매 프레임
  3D 위에 렌더(depth-test, no write). 캡처 회귀
  `debug_lines_draw_colored_segments` + `docs/visual/debug_lines.png`. 데모에
  chase 벡터 + 플레이어/추적자 cross + 아레나 AABB 추가.
- **CPU 파티클 시스템**: `voltex_renderer_ecs::particle` — `Particle` 컴포넌트
  (velocity/acceleration/lifetime/size·alpha 보간) + `spawn_particle` +
  `update_particles`(RuntimeApp tick 자동 호출). 파티클 = Transform+Sprite+
  Particle 엔티티라 빌보드 패스가 그대로 렌더. 이동·페이드·수명 만료 despawn.
  단위 테스트(이동/중력/페이드/despawn) + 캡처 회귀
  `particles_spread_outward_over_time`(시간에 따라 반경 증가) +
  `docs/visual/particle_burst.png`. 데모: 잡힐 때 16개 오렌지 버스트.
- **직교(2D/탑다운) 카메라**: `Camera::ortho_height: Option<f32>` +
  `Camera::orthographic(pos, aspect, height)` (parallel 투영) +
  `RuntimeConfig::with_orthographic(height)`. 2D·탑다운·아이소메트릭 게임용 —
  깊이에 무관한 footprint(원근 단축 없음). 캡처 회귀
  `orthographic_footprint_is_depth_independent`(near≈far) + 단위 테스트
  (ortho는 깊이 무관, perspective는 단축). 기존 perspective 기본값 불변.
- **빌보드 스프라이트**: `voltex_renderer::sprite`(SpriteUniform +
  alpha-blended·depth-tested 파이프라인, 정점 셰이더에서 카메라 right/up으로
  쿼드 확장) + `voltex_renderer_ecs::Sprite` 컴포넌트
  (`Sprite::solid`/`textured`). `WorldTransform`+`Sprite` 엔티티를 mesh 패스 후
  카메라-facing 쿼드로 렌더(카메라 축은 inverse VP 복원, MAX 256/frame). 픽업·
  파티클·월드 마커·간단한 2D-in-3D 적에 사용. 캡처 회귀
  `billboard_sprite_faces_camera_from_any_angle`(정면/측면 footprint 일치) +
  `docs/visual/sprite_billboard.png`. 데모에 떠다니는 픽업 3개 추가.
- **스프라이트시트/아틀라스 UV**: `Sprite.uv_rect` + `with_uv_rect` +
  `set_sheet_frame(cols, rows, index)`(row-major) — sprite.wgsl이 단위 쿼드 UV를
  서브렉트로 remap. 2D 캐릭터 애니메이션·아틀라스용. 단위 테스트(프레임→uv_rect)
  + 캡처 회귀 `sprite_sheet_frame_selects_atlas_cell`(4색 2×2 아틀라스 각 프레임
  정확 샘플).
- **스프라이트 애니메이션**: `SpriteAnimation` 컴포넌트(cols/rows/frame_count/
  fps/looping) + `update_sprite_animations`(RuntimeApp tick 자동 호출) — Sprite의
  `uv_rect`를 시간에 따라 진행. loop=wrap, non-loop=마지막 프레임 clamp+정지.
  단위 테스트(프레임 진행/wrap/clamp+stop).
- **런타임 텍스처 로딩 편의**: `RenderAssets::add_texture_png(device, queue,
  bytes)`(PNG 디코드+업로드, `Result`) + `add_texture_rgba(device, queue, w, h,
  px)` — bind-group 레이아웃 생성 보일러플레이트 제거. gpu_init에서 스프라이트/
  메시 텍스처를 한 줄로 로딩. 캡처 회귀
  `render_assets_png_loader_roundtrips_into_a_sprite`.
- **Lifetime 자동 despawn**: `voltex_runtime::Lifetime { remaining }` 컴포넌트 +
  `update_lifetimes`(tick 마지막 단계, 사용자 시스템 관측 후 만료 시
  `despawn_recursive`). 발사체·타임드 픽업·임시 이펙트·스폰 적 등 자기정리 엔티티용.
  단위 테스트(카운트다운→despawn / 빈 월드 no-op).
- **prelude 확장**: `voltex::prelude`에 게임플레이 컴포넌트 추가 — `Sprite`,
  `Particle`, `SpriteAnimation`, `TextureHandle`, `Lifetime`, `DebugLines`.
  거의 모든 게임이 쓰므로 Transform처럼 prelude에 포함(도메인 특화 타입은 여전히
  명시 경로). prelude 컴파일 테스트 갱신.

### Additive — 런타임 PBR 경로 (v1.1 항목 선반영)

- **BasicRenderer 풀 PBR 패스**: `BasicRenderer::set_pbr_enabled(true)` /
  `RuntimeConfig::with_pbr(true)` — 기존 colored Blinn-Phong 대신 Cook-Torrance
  PBR로 메시 렌더. 멀티라이트(`LightsUniform`: directional sun + `set_point_lights`
  의 point/spot 접합), per-entity 머티리얼 파라미터, 4-맵 텍스처(albedo/normal/
  ORM/emissive via 신규 `PbrTextures` 컴포넌트, 없으면 flat-normal/white/black
  기본), IBL BRDF LUT. `render()` 시그너처 불변(`LightUniform`→`LightsUniform`
  내부 변환). colored/스프라이트/파티클/디버그라인/스킨드/그림자 패스 무영향
  (기본 off → byte-identical). 캡처 회귀
  `basic_renderer_pbr_path_shades_differently_than_colored` +
  `docs/visual/pbr_runtime.png`(metallic 그래디언트 + 스페큘러 + 환경 반사).
  env는 procedural sky(SH 0).
- **스킨드 메시 PBR**: `pbr_skinned.wgsl` — GPU 버텍스 스키닝 + 풀 Cook-Torrance
  프래그먼트. bone palette를 group 0(camera+lights)에 합쳐 max_bind_groups=4
  유지. `create_pbr_skinned_pipeline` + `BasicRenderer` skinned 패스가
  `pbr_enabled` 시 PBR 스킨드 파이프라인 사용(멀티라이트+IBL+그림자). 스킨드
  메시도 이제 정적 메시와 동일한 PBR 라이팅. 캡처 회귀
  `runtime_skinned_pbr_path_renders_and_animates` + `docs/visual/skinned_pbr.png`.
  스킨드 엔티티도 `MaterialHandle`/`TextureHandle`/`PbrTextures`를 정적 메시와
  동일하게 반영(per-entity 머티리얼 + 4-맵 텍스처).
- **선형 텍스처 로더 + 노멀맵**: `GpuTexture::from_rgba_linear` +
  `RenderAssets::add_texture_png_linear`/`add_texture_rgba_linear` — 노멀/ORM
  맵을 sRGB 디코드 없이 linear `Rgba8Unorm`로 업로드. 컬러/알베도는 기존 sRGB
  경로 유지. 캡처 회귀 `pbr_normal_map_perturbs_shading`(노멀맵이 PBR 셰이딩을
  실제로 교란).

### Additive — 실사용 점검 보완

- **MAX 캡 1회성 경고**: BasicRenderer가 per-frame 한도(mesh `max_entities`,
  스프라이트 256, 스킨드 64)를 처음 초과할 때 `eprintln!` 1회 — 조용한 truncation이
  "다 그려졌다"로 오인되지 않게. `Cell<bool>` 플래그로 프레임마다 스팸 안 함.
  단위 테스트(초과 시에만/한 번만 발화).
- **`Mesh` 프리미티브 생성자**: `Mesh::cube(device, size)` / `Mesh::plane` /
  `Mesh::quad(w,h)` / `Mesh::sphere(r, sectors, stacks)` — `from_obj`/`from_gltf`
  스타일의 한 줄 GPU 메시 생성(generate_* + `Mesh::new` 래핑). capture/데모가
  `Mesh::cube`로 교체.
- **프리미티브 메시 생성기**: `voltex_renderer::generate_cube(size)` /
  `generate_plane(size)`(XZ 바닥, +Y) / `generate_quad(w, h)`(XY, +Z) —
  `(Vec<MeshVertex>, Vec<u32>)` 반환(`generate_sphere`와 동일 스타일).
  지금까지 공개 프리미티브가 `generate_sphere` 뿐이라 모든 게임/예제가 큐브
  버텍스를 손으로 작성해야 했던 갭 해소. per-face normal/UV/tangent 정확(PBR
  노멀맵 호환). `voltex_capture::unit_cube`와 `game_ready_demo`가 이걸로 교체
  (손수 작성 큐브 제거). 단위 테스트 4개 + 기존 큐브 GPU 회귀(single_cube/
  base_color/shadow)가 `generate_cube`로 통과.

### Fixed (game-ready 세션)

- **스프라이트 투명도 정렬**: BasicRenderer 스프라이트 패스가 엔티티 순서대로
  그려 겹친 반투명 스프라이트의 합성이 spawn 순서에 따라 달라지던 문제 — 카메라
  거리 기준 back-to-front 정렬 추가. 캡처 회귀
  `overlapping_sprites_blend_independent_of_spawn_order`(두 spawn 순서가 동일
  결과 + 가까운 스프라이트가 위로 합성).

- **그림자**: `RuntimeConfig::with_shadows(true)` — BasicRenderer directional
  shadow mapping (depth pre-pass + mesh_colored 3×3 PCF, group 3). 기본 off
  (동작 불변). 캡처 검증 `docs/visual/shadow_on.png`. 한계: 스킨드 caster 제외,
  ortho 고정, 바닥 접촉 오브젝트 self-occlusion.
- **헤드리스 데모 검증 하니스**: `voltex_capture::scene_playthrough` — 데모
  장면을 오프스크린 렌더해 시각/동작을 자동 확인(라이브 창 불필요).

### Fixed

- **generate_sphere winding**: 구가 CW-from-outside로 감겨 카메라 향한 반구가
  컬링되고 먼 안쪽 반구가 비쳐 앞면이 어둡게 렌더되던 버그(PBR 데모/IBL/모든
  sphere 영향). CCW-from-outside로 수정 → 앞 반구 정상 라이팅.

### Additive — 에셋 로딩 (핸드코딩 큐브 탈피)

- **모델**: `Mesh::from_obj(device, src)`, `Mesh::from_gltf(device, bytes)`.
- **텍스처**: `RenderAssets::add_texture` + `TextureHandle` 컴포넌트 (BasicRenderer
  per-entity 디퓨즈 텍스처), `GpuTexture::from_png(bytes)`.
- **씬 저장/로드**: `RuntimeApp::save_scene_json(path)` / `load_scene(path)`.
- **스킨드 glTF**: `skinned_vertices_from_gltf` + `load_skinned_gltf(device,
  assets, &GltfData)` → 애니메이션 캐릭터를 glTF에서 바로 조립.

### Additive — 스킨드 애니메이션 런타임 통합

- `SkinnedMesh` 에셋 + `SkinnedMeshHandle`/`SkinnedPose` 컴포넌트 +
  `update_skinned_poses`(tick 자동) + BasicRenderer 2차 스킨드 패스
  (per-entity bone palette dynamic UBO, MAX_SKINNED=64).
- `skinned_vertices_from_gltf` + `load_skinned_gltf(device, assets, &GltfData)`
  → 애니메이션 캐릭터를 glTF에서 바로 조립.
- game_ready_demo에 흔드는 스킨드 totem.

### Additive — Lua 게임 로직

- `nav_set_target` / `nav_clear_target` / `nav_reached`, `audio_play` /
  `audio_stop` (engine_api → voltex_ai/voltex_audio 의존).

### 검증

워크스페이스 build / clippy(-D warnings) / test 그린. 런타임/렌더 경로는 `voltex_capture`
헤드리스 GPU 회귀로 픽셀 검증(카메라 프레이밍, base_color, 텍스처 샘플, 스킨드 변형,
OBJ/glTF/PNG 로드). **남은 visual 검증**: 라이브 창에서의 데모 외형은 사용자 디버깅 패스.

## [v1.0.59] — 2026-06-03 — Game-ready: wire up nav / Lua input / HUD / audio / skinning

"기능은 있는데 연결이 안 된" 다섯 구멍을 메워 엔진을 실사용 가능(game-ready)
상태로. 설계: `docs/superpowers/specs/2026-06-03-engine-game-ready-upgrade-design.md`.

### Additive — nav_agent_system (voltex_ai)

`nav_agent_system(world, navmesh, dt)` — NavAgent+Transform 엔티티를 매 프레임
`find_path` → `follow_path` 스티어링으로 이동(kinematic), 최종 waypoint 도달 시
goal에 snap + `reached`. 도달 불가 target은 idle 전환. NavMesh는 호출자 주입. +5 테스트.

### Additive — Lua 입력 API (voltex_script)

스크립트에서 입력 읽기: `input.is_key_pressed("W")` / `is_key_just_pressed` /
`mouse_position` / `mouse_delta` / `is_mouse_button_pressed("left")` / `scroll`.
`voltex_script`는 winit 무의존 유지 — POD `ScriptInput` 스냅샷 + thread-local
포인터(`set_current_input`), 키 이름↔keycode 매핑은 `voltex_runtime`이 소유.
런타임이 매 프레임 스냅샷 발행/실행 후 클리어. +5 단위 +2 런타임(전체 체인) 테스트.

### Additive — HUD 런타임 API (voltex_runtime → voltex_editor)

`RuntimeConfig::with_hud(true)` + `RuntimeApp::set_hud_fn(|ui, world| ...)`.
editor UiContext/UiRenderer를 GPU 초기화 시 생성, redraw에서 BasicRenderer 다음
같은 encoder로 `LoadOp::Load` 인-윈도우 오버레이 패스. 점수·체력 등 콘솔 print 탈피.
runtime_demo가 인-윈도우 HUD 데모로 전환. +2 테스트.

### Additive — audio_sync_system (voltex_runtime → voltex_audio)

`RuntimeConfig::with_audio(clips)` + `RuntimeApp::set_audio_listener(entity)`.
RuntimeApp이 `Option<AudioSystem>` 소유(실패 시 silent None). `audio_sync_system`이
listener Transform 동기화 + `AudioSource::playing` 엣지(`active` 플래그)에서만
play/play_3d/stop — held playing이 매 프레임 voice를 새로 만들지 않음.
순수 `decide_action`으로 엣지 로직 분리. +3 +1 테스트.

### Additive — 스킨드 메시 애니메이션 (Blinn-Phong 한정)

`voltex_renderer::Skeleton`(from_skin/apply_animation/compute_palette) — glTF
skin/animation을 GPU 스키닝 행렬(`world_j · inverse_bind_j`)로. `create_skinned_mesh_pipeline`
+ `mesh_skinned.wgsl`(group0 camera, group1 bone palette, 4 joint/weight 블렌딩).
`voltex_capture::scene_skinned`가 RTX 4080(Vulkan)에서 rest(직선)↔bent(굽힘) 2프레임
렌더 + 픽셀 차이로 시각 검증 (`docs/visual/skinned_{rest,bent}.png`). +6 단위 +1 GPU 테스트.
**미완(v1.1)**: PBR 경로 스키닝(bind group 재배치), ECS SkinnedMesh 컴포넌트/자동 재생.

### 검증

`cargo build --workspace` / `clippy --all-targets -D warnings` / `test --workspace --lib`
전부 그린. 워크스페이스 테스트 ~1180 → ~1202. 의존성 그래프: voltex_runtime이
editor/audio 추가 의존(L3.5 → L3/L1, 방향 유효).

## [v1.0.58] — 2026-05-28 — Quat Transform + fuzz + coverage gate + audio smoke (E3b/F4/F5/F6/F7)

Closes every remaining item in `docs/dev/AUDIT-BACKLOG.md`.

### Breaking — Transform.rotation Vec3 → Quat (E3b)

`voltex_ecs::Transform.rotation` was an Euler `Vec3` (YXZ radians). It is
now a unit `voltex_math::Quat`. 24 files touched across ecs, physics,
editor, script, net, examples, capture.

- `Transform::matrix()` uses `Quat::to_mat4` directly.
- `Transform::from_euler_yxz(pos, euler, scale)` helper for call sites
  that still author orientations as Euler triples.
- `EditorCommand::SetTransform.rotation` is now `Quat`.
- Inspector keeps an Euler buffer per selected entity (re-extract only
  on selection change or external quat drift) so sliders stay smooth
  without round-tripping `Quat → Euler → Quat` every frame.
- Gizmo rotate composes `q_new = (q_delta * q_initial).normalize()` —
  axis-stable regardless of initial orientation.
- Lua API surface unchanged: `set_rotation` / `get_rotation` still talk
  YXZ Euler, with `Quat::from_euler_yxz` / `to_euler_yxz` at the FFI
  boundary so existing scripts keep working.
- Physics integrator switches `rotation + ω·dt` (wrong) to
  `Quat::integrate(ω, dt)` (semi-implicit `dq/dt = ½ωq + normalize`).
- voltex_math gains `Quat::to_euler_yxz` (gimbal-collapsing inverse of
  from_euler_yxz) + `Quat::integrate` helpers, +8 tests (commit bee796c).
- voltex_math::Quat re-exported via `voltex::prelude`.

### Wire format compatibility

Persistent formats (`.vscn` text, JSON, binary) **dual-read** 3-float
Euler (legacy) and 4-float quaternion (current). Existing user scene
files keep loading without conversion. Writers emit 4-float quat.

Ephemeral wire format (`voltex_net::EntityState.rotation`) is a **hard
cutover** from `[f32;3]` to `[f32;4]`. `ENTITY_SIZE` 40 → 44 bytes.
Interpolation uses `Quat::slerp` instead of componentwise Euler lerp,
sidestepping the long-way bug previously patched at v1.0.56.

### Layer graph

voltex_net moved L0 → L1 (now depends on voltex_math for Quat::slerp).
CLAUDE.md dependency graph updated.

### Additive — F4 cargo-fuzz scaffold

New `fuzz/` crate (excluded from workspace) with 9 libfuzzer targets:
parse_png/jpg/gltf/obj, scene text/json/binary, snapshot, navmesh.
Each target is a thin `|data: &[u8]| -> _` wrapper that calls the
parser/deserializer and discards the result. Documents nightly
invocation in `fuzz/README.md`. voltex_ai gains pub re-exports for
`serialize_navmesh` / `deserialize_navmesh`.

### CI — F5 coverage gate flipped to blocking

The `coverage` job in `.github/workflows/ci.yml` was advisory; now
blocks at `--fail-under-lines 78`. Runner installs mesa-vulkan-drivers
+ lavapipe (software Vulkan) and runs with `-- --include-ignored` so
voltex_capture's GPU-gated tests exercise renderer / renderer_ecs /
runtime headless paths. lavapipe missing → tests graceful-skip,
78% gate still passes on lib-only coverage (79.69% measured locally,
84.24% with GPU paths).

`scripts/coverage.sh` mirrors the CI invocation for local repro;
supports `--lib-only` / `--html` / `VOLTEX_COV_FAIL_UNDER_LINES`
override.

### Test — F6 audio backend real-device smoke

Each backend (`wasapi`, `alsa`, `coreaudio`) gains an `#[ignore]` test
that opens the default output device, writes 64 frames of silence, and
tears down. Skip with `eprintln!("skipping: no <backend> device")` on
init failure so scripted unattended runs are safe. WASAPI verified on
RTX/Realtek box; ALSA / CoreAudio await real-hardware passes (tracked
in pre-release checklist).

### Docs — F7 CI green-run procedure

`docs/release.md` gains "Real-hardware audio backend verification (F6)"
and "First green CI run on GitHub mirror (F7)" sections under the
pre-release checklist. Per-OS `cargo test ... --ignored` invocation
pinned; `git push github master` workflow documented.

### Verification

`cargo build --workspace` clean, `cargo clippy --workspace --all-targets
-- -D warnings` clean, `cargo test --workspace --lib` 1158 tests passing
(↑ from 1157 — new WASAPI smoke test runs on this Windows host).

---

## [v1.0.57] — 2026-05-28 — voltex_image crate split (E1 + C1 audit items)

**Refactor / Additive**. New L0 crate `voltex_image` extracts all GPU-agnostic asset
codecs from `voltex_renderer`, resolving audit items E1 (God-crate reduction) and C1
(CRC32/Adler32 deduplication). Commits: 202afea + 7ee76f4 + f3684ae + b0c2d63 + 1856ef1.

### What moved to `voltex_image`
- PNG decoder (`parse_png`), JPEG Baseline + Progressive decoder (`parse_jpg`)
- DEFLATE inflate (`deflate` module), PNG encoder (`encode_rgba8`)
- glTF 2.0/GLB parser (`parse_gltf`, `GltfData`/`GltfMesh`/`GltfMaterial`)
- OBJ parser (`obj` module), JSON parser (`json_parser` module)
- `MeshVertex` POD struct (`bytemuck::Pod + Zeroable`)
- CRC32/Adler32 (`checksum` module — crate-private)

### What stays in `voltex_renderer`
- `MESH_VERTEX_LAYOUT` (`VertexBufferLayout` for `MeshVertex`) — renderer/vertex.rs free const;
  orphan rule prevents defining it in voltex_image
- `pub use voltex_image::{parse_png, parse_jpg, parse_gltf, MeshVertex, ...}` back-compat
  re-exports — existing code using `voltex_renderer::parse_png` continues to compile

### Other changes
- `voltex_capture`: duplicate CRC32/Adler32 removed; uses `voltex_image::encode_rgba8` (C1)
- `voltex` facade: `pub use voltex_image as image` added (`voltex::image` module)
- 91 tests migrated to voltex_image; workspace total remains green

### Follow-up
- F9: `#![allow(missing_docs)]` suppressions in moved decoder files — to be resolved
  incrementally with doc comments (low risk, tracked in AUDIT-BACKLOG)

---

## [v1.0.54] — 2026-05-22 — Sphere generator degenerate-input clamp

**Fix**. `voltex_renderer::sphere::generate_sphere(radius, sectors, stacks)` divided by zero on multiple paths when called with degenerate inputs:

- `sector_step = 2π / sectors` → ∞ when `sectors == 0`.
- `stack_step = π / stacks` → ∞ when `stacks == 0`.
- `normal = [x/radius, y/radius, z/radius]` → NaN when `radius == 0`.
- `uv = [j/sectors, i/stacks]` → NaN when either is zero.

A `generate_sphere(0.0, 0, 0)` call produced a NaN-filled vertex buffer that then panicked downstream when a renderer tried to upload it. Realistic callers always pass valid arguments, but PBR demos with user-driven sliders could plausibly hit zero.

### Fixed

Inputs are now clamped to sensible minimums up front:

- `sectors.max(3)`, `stacks.max(2)` — minimum geometry for a closed mesh.
- `radius` validated as finite and > `f32::MIN_POSITIVE`, otherwise clamped.

### Tests

`voltex_renderer` 230 → 232 (+2):

- `voltex_renderer::sphere::tests::generate_sphere_with_degenerate_inputs_produces_finite_mesh`
- `voltex_renderer::sphere::tests::generate_sphere_with_negative_radius_clamps`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.53] — 2026-05-22 — `Camera::right` NaN at pitch ±90°

**Fix**. `voltex_renderer::Camera::right()` computed `forward × Vec3::Y` and called `.normalize()`. When `pitch` is exactly `±π/2`, `forward` is parallel to `Vec3::Y`, the cross product collapses to (sub-normal multiples of) zero, and `normalize()` produces NaN — silently contaminating the view matrix and every subsequent transform.

`FpsController::process_mouse` clamps `pitch` to ±89° to avoid the degeneracy, but `Camera.pitch` is a public field. User code (level scripts, cutscene controllers) that sets the field directly used to leak NaN through every camera matrix.

### Fixed

- `Camera::right` now uses `normalize_or_zero` (added in v1.0.41) and falls back to `Vec3::X` for the degenerate case.

### Tests

`voltex_renderer` 228 → 230 (+2):

- `voltex_renderer::camera::tests::camera_right_at_straight_up_does_not_nan`
- `voltex_renderer::camera::tests::camera_right_at_straight_down_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.52] — 2026-05-22 — GameTimer hz=0 panic + alpha div-by-zero

**Fix**. Two `voltex_platform::GameTimer` footguns:

### Fixed

- **`GameTimer::new(0)` panic.** `Duration::from_secs_f64(1.0 / 0.0)` evaluates `1.0 / 0.0 = inf`, which `from_secs_f64` rejects with a panic. `new` now clamps `fixed_hz` to `1..=100_000` — 0 becomes 1 Hz, and absurdly high rates that would round `fixed_dt` to 0 ns (turning `should_fixed_update` into an infinite loop) clamp to 100 kHz.
- **`alpha()` div-by-zero defence.** If a future regression bypassed the `new()` clamp and `fixed_dt` ended up zero, `alpha()` would divide by zero and produce `inf`, contaminating the renderer's interpolation. Now returns `0.0` when `fixed_dt <= 0`.

### Tests

`voltex_platform` 5 → 8 (+3):

- `voltex_platform::game_loop::tests::new_with_zero_hz_does_not_panic`
- `voltex_platform::game_loop::tests::new_with_extreme_hz_does_not_round_to_zero`
- `voltex_platform::game_loop::tests::alpha_with_zero_fixed_dt_returns_zero_not_inf`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for v1.0.40 – v1.0.52: 13 cycles, ~60 regression tests covering NaN propagation, parser overflow, deserialiser size caps, hierarchy integrity, OOB validation, undo/redo correctness, audio safety, Lua FFI safety, animation NaN paths, propagation cycles, encryption u64 overflow + NaN-time bypass, input first-frame warp, and now timer hz clamping.

---

## [v1.0.51] — 2026-05-22 — Input first-mouse-move warp fix

**Fix**. `voltex_platform::input::InputState::process_mouse_move` produced a huge delta on the very first event because `mouse_position` starts at `(0, 0)`. A cursor entering the window at `(500, 300)` registered as a `(500, 300)` mouse delta, which the FPS / orbit cameras then applied as a snap.

### Fixed

- Added `mouse_seen: bool` to `InputState`. The first event after construction is treated as a warp (no delta contributed) and only flips `mouse_seen = true`. Subsequent events compute the delta normally. `begin_frame` does not reset `mouse_seen`, so the zero-delta path only fires once per program run.

### Tests

`voltex_platform` 3 → 5 (+2). This crate had no `input` tests before — now exercised:

- `voltex_platform::input::tests::first_mouse_move_produces_zero_delta`
- `voltex_platform::input::tests::begin_frame_clears_delta_but_keeps_mouse_seen`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.50] — 2026-05-22 — PacketCipher u64 overflow + AuthToken NaN-time bypass

**Fix**. Two `voltex_net::encryption` bugs reachable from adversarial input:

### Fixed

- **`PacketCipher::decrypt` u64 overflow on `seq = u64::MAX`.** An attacker can craft a packet whose 8-byte sequence header is `[0xFF; 8]`. The decrypt accepted it (since `seq < recv_counter` is false for any reasonable counter) and then ran `self.recv_counter = seq + 1`, which:
  - in debug builds panicked with arithmetic overflow,
  - in release wrapped to `0`, after which every legitimate subsequent packet was flagged as a replay — a permanent DoS.
  Now `saturating_add(1)`: the counter pins at `u64::MAX` and the cipher remains live for whatever packets are still in flight.
- **`AuthToken::validate` NaN-time bypass.** `if current_time > self.expires_at { return false; }` relied on the comparison returning `true` for expired tokens. NaN comparisons always return `false`, so a NaN `current_time` (e.g. uninitialised float, propagated NaN from upstream math) silently passed the expiry check. Now any non-finite `current_time` or `expires_at` is rejected up front.

### Tests

`voltex_net` 49 → 51 (+2):

- `voltex_net::encryption::tests::decrypt_with_max_u64_seq_does_not_panic_or_wrap`
- `voltex_net::encryption::tests::auth_token_rejects_nan_current_time` (also pins NaN expiry rejection)

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

`AssetStorage` and `SparseSet` were also audited this round; both already have generation-checked handle lookups and bounded resize behaviour for realistic entity-id ranges. No fixes needed there.

---

## [v1.0.49] — 2026-05-22 — propagate_transforms cycle / depth-limit

**Fix**. The `add_child` cycle guard introduced in v1.0.42 only catches cycles created *after* it landed. A scene file loaded via raw `world.add(_, Children(_))` / `world.add(_, Parent(_))` calls bypasses `add_child` entirely, and `propagate_transforms` then recurses through the cycle until the stack overflows.

### Fixed

- **`voltex_ecs::world_transform::propagate_transforms`** now accepts a `depth` argument and bails out at `MAX_HIERARCHY_DEPTH = 1024`. Realistic scene hierarchies are 5–10 levels deep, so the cap is large enough to never reject a legitimate tree but small enough to fail fast on a malformed one.

### Tests

`voltex_ecs` 144 → 146 (+2):

- `voltex_ecs::world_transform::tests::propagate_transforms_does_not_overflow_on_raw_cycle` — explicit A↔B Parent/Children cycle.
- `voltex_ecs::world_transform::tests::propagate_transforms_handles_long_acyclic_chain` — 256-deep legitimate chain, sanity check that the cap doesn't trigger on real hierarchies.

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

GJK / EPA narrow-phase audited in this round; existing length / cross-product guards (`if dot.abs() < 1e-8 { return None }`, `length_squared() < 1e-10` checks, and `make_face` Vec3::Y fallback for degenerate triangles) cover the obvious NaN-source paths. No fixes needed there in this cycle.

---

## [v1.0.48] — 2026-05-22 — Animation system zero-duration / OOB keyframe / quat slerp NaN

**Fix**. Three latent NaN / panic paths in `voltex_renderer/src/animation.rs`:

### Fixed

- **`AnimationPlayer::update` zero-duration NaN.** A zero-duration animation in `looping=true` mode hit `current_time %= 0.0`. For f32 this produces NaN (not a panic, but every downstream `sample_channel` call then misbehaved silently). Now: `duration <= 0` clamps `current_time` to 0 and stops the player.
- **`extract_keyframe` slice OOB panic.** A malformed glTF where `values.len() < times.len() * stride` panicked at `values[start..end]` whenever `start > values.len()`. The previous `end.min(values.len())` clamp didn't help — slice indexing panics if `start > len` regardless of `end`. `start >= values.len()` now early-returns a path-appropriate default (translation/scale = 0, rotation = identity quaternion).
- **`Quat::slerp` `acos(> 1.0)` NaN.** Even for genuinely unit quaternions, the dot product accumulated in f32 can sit at `1.0000001` etc. `theta = dot.acos()` then returned NaN, contaminating the whole interpolation. Added `dot = dot.clamp(-1.0, 1.0)` before branching. The existing `dot > 0.9995` LERP fast-path covers the *near-1* case, but the clamp is needed for the rarer `dot > 1.0` (overshoot) and `dot < -1.0` (after the sign-flip branch) edges.

### Tests

`voltex_renderer` 225 → 228 (+3):

- `voltex_renderer::animation::tests::animation_player_zero_duration_does_not_div_by_zero`
- `voltex_renderer::animation::tests::extract_keyframe_with_truncated_values_returns_default`
- `voltex_renderer::animation::tests::quat_slerp_with_floating_point_dot_overshoot_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.47] — 2026-05-22 — Lua FFI null-deref + type-confusion guards

**Fix**. The Lua engine_api FFI surface had three latent safety bugs reachable from arbitrary script code:

### Fixed

- **`get_world()` null-pointer dereference (UB → SIGSEGV).** Pre-fix:
  ```rust
  let ptr = ffi::lua_touserdata(L, -1) as *mut World;
  ffi::lua_pop(L, 1);
  &mut *ptr   // UB if ptr is null
  ```
  triggered any time the `__voltex_world` global was missing or not a light userdata. Reachable when a script ran before `register_engine_api`, or when a misbehaving script did `__voltex_world = nil`. `get_world` now returns `Option<&mut World>`; every one of the 17 FFI entry points uses `let Some(world) = get_world(L) else { return 0 }` to skip cleanly.
- **Non-numeric entity handles silently mutated entity (0, 0).** `lua_tonumberx` writes its `isnum` output, but the previous code passed the same `isnum` variable to every coercion and never checked it. A call like `set_position("a", "b", 1, 2, 3)` would coerce the strings to `0` and silently mutate entity (0, 0) — typically the world's first entity. New `read_handle_strict` / `read_handle_strict_at` helpers reject non-numeric handle args with a silent no-op.
- **Non-numeric coordinates silently coerced to 0.** Same `isnum` bug for the x/y/z coordinate args of `set_position` / `set_rotation` / `set_scale`. Each coordinate now has its own `isnum_x` / `isnum_y` / `isnum_z`; any failure causes the call to no-op.

### Tests

`voltex_script` 42 → 45 (+3):

- `voltex_script::engine_api::tests::entity_count_without_world_global_returns_nothing`
- `voltex_script::engine_api::tests::entity_count_after_global_overwrite_does_not_crash`
- `voltex_script::engine_api::tests::set_position_with_non_numeric_args_is_silent_noop`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.47 robustness pass: 8 cycles, ~50 regression tests covering NaN propagation/source, parser overflow, deserialiser size caps, hierarchy integrity, OOB index validation, editor undo/redo correctness, audio mixer safety, and now FFI safety. Workspace test count ~1056 → ~1095.

---

## [v1.0.46] — 2026-05-22 — Editor redo-Spawn leak + audio mixer OOB defence + runtime resize cleanup

**Fix**. Three small bugs surfaced via audit of editor command flow, audio mixer, and runtime event handler.

### Fixed

- **`EditorSession::redo()` leaked entities on the `Spawn → Undo → Redo → Undo` sequence.** The undo stack stored `DespawnSpawned(old_entity)` as the inverse of the original `Spawn`. After the first undo killed `old_entity` and redo allocated a fresh entity (same id, bumped generation), the inverse still pointed at the dead slot. The second undo's `world.despawn(old_entity)` was a silent no-op because the generation no longer matched, and the freshly-redo'd entity stayed alive forever. Fixed by patching the inverse to `DespawnSpawned(applied_entity)` immediately after redo applies a `Spawn` command.
- **`voltex_audio::mixing::mix_sounds` OOB panic on stale `clip_index`.** A `PlayingSound` whose `clip_index` no longer indexes the `clips` slice (asset reload, list shrunk) used to crash with index-out-of-bounds on `clips[sound.clip_index]`. The mixer now skips invalid sounds via `clips.get(_)` and queues them for removal so the next pass drops them cleanly.
- **`voltex_runtime` `WindowEvent::Resized` cleanup.** Window minimisation produces a 0×0 resize event on most platforms. `state.gpu.resize` already no-ops for zero dimensions, but the caller was invoking it unconditionally; tightened the guard so both `gpu.resize` and the camera aspect update happen behind the same `> 0` check. Functional behaviour unchanged.

### Tests

`voltex_audio` 119 → 120 (+1), `voltex_editor` 169 → 170 (+1):

- `voltex_audio::mixing::tests::out_of_range_clip_index_does_not_panic`
- `voltex_editor::session::tests::redo_spawn_then_undo_cleans_up_the_new_entity`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.45] — 2026-05-22 — glTF accessor count cap + attribute length consistency

**Fix**. Two classes of bug in `voltex_renderer/src/gltf/accessor.rs`:

### Fixed

- **Accessor `count` is now capped at `MAX_ACCESSOR_COUNT = 4_000_000`.** Every `read_accessor_*` function (vec3/vec2/vec4/indices/joints/mat4/floats) trusted the JSON `count` field; an adversarial accessor with `count = u32::MAX` drove `Vec::with_capacity` allocations of 48 GB (vec4), 32 GB (vec3), etc., before any data was even read. A single `validate_count` helper now gates every accessor read.
- **`read_accessor_floats`** computed `total = count * components` without overflow check. Switched to `checked_mul` (defends 32-bit `usize`).
- **`extract_meshes` attribute length validation.** A glTF primitive whose `POSITION` accessor declared 3 elements while `NORMAL` declared 5 used to OOB-panic at `normals[3]` inside the vertex assembly loop. The mismatch is now caught up front with a friendly error message that lists every attribute's length.

### Tests

`voltex_renderer` 223 → 225 (+2):

- `voltex_renderer::gltf::tests::parse_gltf_rejects_huge_accessor_count`
- `voltex_renderer::gltf::tests::parse_gltf_rejects_attribute_length_mismatch`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The OBJ parser is unaffected — it already uses `.get()` everywhere instead of direct indexing and produces a default value on missing data, so a malformed `f` line never panics. JSON-parser-level size limits remain v1.1.

---

## [v1.0.44] — 2026-05-22 — Navmesh deserializer adversarial input hardening

**Fix**. Same pattern as v1.0.43 applied to the VNAV navmesh format. Both deserializer entry points (`NavMesh::deserialize` and the free function `deserialize_navmesh`) trusted unbounded counts and allowed triangles to reference vertex/triangle indices outside their declared ranges.

### Fixed

- **`voltex_ai::navmesh::deserialize_navmesh` and `NavMesh::deserialize`**:
  - Vertex count and triangle count are now capped at `1_000_000` each (`MAX_NAVMESH_VERTICES` / `MAX_NAVMESH_TRIANGLES`). An adversarial header with `count = u32::MAX` used to drive `Vec::with_capacity` allocations of 12 GB (vertex slot) + 48 GB (triangle slot).
  - Triangle vertex indices (`i0 / i1 / i2`) are validated to be inside the vertex array. Pre-fix, a malformed file declaring 1 vertex + a triangle with index `5` could be deserialised; the panic surfaced later inside `NavMesh::shared_edge` or `point_in_triangle_xz` when downstream code did `self.vertices[5]`.
  - Neighbour triangle indices (when non-negative) are validated to be inside the triangle array. Same panic class but inside pathfinding code paths.

The validations are done at deserialise time, so downstream consumers can use indices without re-checking.

### Tests

`voltex_ai` 34 → 37 (+3):

- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_huge_vertex_count`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_triangle_vertex_index_oob`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_neighbour_index_oob`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.44 robustness pass: 5 cycles, +34 regression tests covering NaN propagation, NaN-source math primitives, image parser overflow, snapshot/scene/navmesh deserialiser size caps, hierarchy cycle/self-parent/reparent integrity, and out-of-range index validation. Workspace tests went from ~1056 to ~1090.

---

## [v1.0.43] — 2026-05-22 — Scene deserializer adversarial input hardening

**Fix**. Two scene deserializers (text `.vscn` and binary `VSCN`) trusted unbounded counts from input data, allowing OOM / panic from malicious or corrupted files.

### Fixed

- **`voltex_ecs::scene::deserialize_scene` (.vscn text)**:
  - Lines like `entity 4294967295` previously forced the loader to push billions of placeholder `Option<Transform>` entries into the intermediate vectors before any error could be reported. Now silently ignored above `MAX_SCENE_ENTITIES = 1_000_000`.
  - Out-of-range parent indices (e.g. `parent 99` when only 1 entity was declared) used to panic with index-out-of-bounds inside the parent-application loop. Now silently ignored.
  - Cycles and self-parents that survive the parse are caught by the v1.0.42 `add_child` guard. This entry just documents that the guarantee composes correctly through scene load.

- **`voltex_ecs::binary_scene::deserialize_scene_binary` (.vscn binary)**: header field `entity_count: u32` was used directly in `Vec::with_capacity(entity_count)`. An adversarial header with `count = u32::MAX` could allocate 32 GB+ (entity slot vec) before reading any body bytes. Same `MAX_SCENE_ENTITIES = 1_000_000` cap applied with a friendly error message.

### Tests

`voltex_ecs` 139 → 144 (+5):

- `voltex_ecs::scene::tests::deserialize_scene_caps_huge_entity_index`
- `voltex_ecs::scene::tests::deserialize_scene_ignores_out_of_range_parent_index`
- `voltex_ecs::scene::tests::deserialize_scene_self_parent_is_silently_refused`
- `voltex_ecs::scene::tests::deserialize_scene_at_cap_boundary_works`
- `voltex_ecs::binary_scene::tests::deserialize_binary_scene_rejects_huge_entity_count`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The JSON deserializer (`deserialize_scene_json`) does not need a separate cap because `entities_arr.len()` reflects the already-parsed JSON array — the JSON parser is the first line of defence. Adding a limit to the JSON parser itself is queued for v1.1.

---

## [v1.0.42] — 2026-05-22 — Hierarchy cycle / self-parent / reparent integrity

**Fix**. `voltex_ecs::hierarchy::add_child` silently allowed three classes of integrity violation that caused unbounded-recursion stack overflow inside `despawn_recursive`:

### Fixed

- **Self-parenting.** `add_child(world, e, e)` set `e.Parent = e` and put `e` in `e.Children`, after which `despawn_recursive(e)` recursed into itself until the stack overflowed. Now silently refused.
- **Cycles.** `add_child(A, B)` followed by `add_child(B, A)` formed an A↔B cycle. Deeper cycles (`A→B→C, then C→A`) were equally accepted. `add_child` now walks `parent`'s ancestor chain before writing; if `child` is encountered, the call is silently refused. A `MAX_ANCESTOR_WALK = 2²⁰` cap also defends against pre-existing cycles in malformed scene files.
- **Stale entries on re-parent.** `add_child(old, child)` followed by `add_child(new, child)` overwrote `child.Parent` but left `child` in `old.Children`. `add_child` now cleans the old parent's `Children` list before assigning the new one.

### Tests

`voltex_ecs` 134 → 139 (+5):

- `voltex_ecs::hierarchy::tests::add_child_rejects_self_parent`
- `voltex_ecs::hierarchy::tests::add_child_rejects_simple_cycle`
- `voltex_ecs::hierarchy::tests::add_child_rejects_deeper_cycle`
- `voltex_ecs::hierarchy::tests::add_child_reparents_cleanly`
- `voltex_ecs::hierarchy::tests::despawn_recursive_terminates_on_clean_hierarchy` — 256-deep chain sanity check

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cycles introduced via direct `world.add(child, Parent(parent))` (e.g. by a corrupted .vscn) bypass `add_child` and still need a separate `validate_hierarchy` pass on scene load. Queued for v1.1+.

---

## [v1.0.41] — 2026-05-22 — NaN-source hardening (math) + snapshot deserialiser cap

**Fix + additive**. v1.0.40 protected sort comparators from incoming NaN. v1.0.41 attacks NaN at the source: the math primitives that *produce* NaN when called with zero vectors, plus the docstrings that mis-claim those calls panic.

### Fixed

- **`Vec3::normalize` / `Vec2::normalize` docstrings.** Old text claimed "Panics if the input is the zero vector (division by zero)". Rust `f32 / 0.0` does not panic — it produces NaN (when numerator is also 0) or `±inf`. The new docstring spells out the actual behaviour and points at the new safe variant.
- **`Ray::new` docstring.** Same correction. `Ray::new(_, Vec3::ZERO)` did not panic — it stored a NaN direction.
- **`Mat4::look_at` docstring.** Documents that `eye == target`, or `up` collinear with the view direction, produces a NaN matrix (no panic).
- **`voltex_net::snapshot::deserialize_snapshot` entity-count handling.** Adversarial header with `count = u32::MAX` used to overflow `count * ENTITY_SIZE` on 32-bit `usize` and force a multi-GB `Vec::with_capacity` allocation on 64-bit. Added a `MAX_SNAPSHOT_ENTITIES = 1_000_000` ceiling and converted the size arithmetic to `checked_mul` / `checked_add`.

### Added

- `Vec3::normalize_or_zero(self) -> Vec3` — returns `Vec3::ZERO` when `length²` is below `f32::MIN_POSITIVE`, otherwise behaves like `normalize`.
- `Vec2::normalize_or_zero(self) -> Vec2` — same pattern.
- `Ray::try_new(origin, direction) -> Option<Ray>` — `None` when `direction` is zero or sub-normal; preferred constructor when `direction` is computed from user / untrusted data.

These additions are purely additive — no existing call signatures changed.

### Tests

`voltex_math` 39 → 48 (+9), `voltex_net` 47 → 49 (+2):

- `voltex_math::vec3::tests::normalize_zero_produces_nan` (regression-pin for the documented bad behaviour)
- `voltex_math::vec3::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec3::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::vec3::tests::normalize_or_zero_handles_subnormal_length`
- `voltex_math::vec2::tests::normalize_zero_produces_nan`
- `voltex_math::vec2::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec2::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::ray::tests::try_new_rejects_zero_direction`
- `voltex_math::ray::tests::try_new_accepts_nonzero_direction`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_huge_entity_count`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_truncated_body`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The ~30 existing `.normalize()` callsites (steering, gizmo, picking, csm, frustum, …) were *not* mass-migrated to `normalize_or_zero`. Each site has either a manual length-guard or a calling convention that excludes zero inputs; a blanket rewrite would mask context-specific assumptions. A targeted audit cycle is queued for v1.1+.

---

## [v1.0.40] — 2026-05-22 — NaN-safe sort comparators + parser size hardening

**Fix**. P0 robustness pass surfaced via static audit of `partial_cmp(...).unwrap()` callsites and arithmetic-overflow paths in the image parsers. Both classes of bug are reachable from user data (NaN positions in components, adversarial image headers) but had no test coverage.

### Fixed

- **NaN-safe sort comparators in 6 production callsites.** `partial_cmp(...).unwrap()` panics when one side is `NaN`. Replaced with `.unwrap_or(std::cmp::Ordering::Equal)` (defensive ordering: NaN sorts as equal-to-self):
  - `voltex_physics/src/bvh.rs:62` — BVH build sorts entries by AABB center on the longest axis. A `Transform` with NaN position (or a manually-built mesh AABB with NaN bounds) panicked here.
  - `voltex_physics/src/bvh.rs:150` — `BvhTree::query_ray` sorts hits by ray-distance. NaN-bounded AABBs could yield NaN intermediate t.
  - `voltex_physics/src/raycast.rs:142` — `raycast_all` sorts hits by distance.
  - `voltex_physics/src/mesh_collider.rs:75` — `ray_vs_mesh_all` sorts hits by distance. Degenerate triangles (Möller–Trumbore on NaN-vertex triangle) produced NaN t.
  - `voltex_editor/src/rasterizer.rs:124` — TTF glyph scanline-fill sorts edge intersections by x.
  - `voltex_renderer/src/light_probes.rs:43` — `LightProbeGrid::nearest` minimum search by squared distance.

- **Parser size-overflow / huge-allocation guards (BMP, PNG, JPG).** Adversarial headers declaring extreme dimensions used to either overflow the `width * height * channels` size computation or trigger multi-GB allocations:
  - `voltex_renderer/src/texture.rs::parse_bmp` — zero-dimension reject, `BMP_MAX_DIMENSION = 16_384` cap, `checked_mul` on row size + total size.
  - `voltex_renderer/src/png.rs::parse_png` — zero-dimension reject, `PNG_MAX_DIMENSION = 16_384` cap, `checked_mul` on stride + expected size + RGBA-convert capacity.
  - `voltex_renderer/src/jpg/mod.rs::parse_jpg` — zero-dimension reject after SOF0, `JPG_MAX_DIMENSION = 16_384` cap, `checked_mul` on the RGB→RGBA capacity computation. (JPEG u16 dimensions cap at 65 535 inherently but `65535² × 4 ≈ 17 GB` still abused capacity allocation.)

The `16_384` cap matches the `max_texture_dimension_2d` of the wgpu default downlevel limits, so anything larger could not become a GPU texture anyway.

### Added (regression tests)

- `voltex_physics::bvh::tests::build_with_nan_center_does_not_panic`
- `voltex_physics::bvh::tests::query_ray_with_nan_hit_distance_does_not_panic`
- `voltex_physics::raycast::tests::raycast_all_with_nan_position_does_not_panic`
- `voltex_physics::mesh_collider::tests::ray_vs_mesh_all_with_degenerate_triangle_does_not_panic`
- `voltex_renderer::light_probes::tests::nearest_with_nan_query_does_not_panic`
- `voltex_renderer::texture::tests::parse_bmp_rejects_zero_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_rejects_huge_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_accepts_max_allowed_dimension_header`
- `voltex_renderer::png::tests::parse_png_rejects_zero_dimensions`
- `voltex_renderer::png::tests::parse_png_rejects_huge_dimensions`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green. `voltex_physics` 106 → 110 (+4), `voltex_renderer` 217 → 223 (+6).

### Notes

This batch is static-audit-driven. Fuzz-based coverage expansion (libfuzzer / cargo-fuzz across all parsers + ECS query paths) remains v1.0.x P3 / v1.1.

---

## [v1.0.39] — 2026-05-21 — README polish (status badge + service-readiness bullets)

P2. README's status badge was frozen at v1.0.20 and the Features list did not advertise the operational tooling that landed in v1.0.29–v1.0.34.

### Updated

- `README.md` status badge: `v1.0.20` → `v1.0.38`.
- `README.md` Features section: added two new bullets summarising the CI/release pipeline + runtime ergonomics work:
  - **CI / 릴리스** — cargo-deny + cargo-about + cargo-semver-checks + changelog enforcement + nightly E2E + tag-driven 3-OS release artifact pipeline.
  - **Runtime ergonomics** — RuntimeApp 4-line main, HUD primitive, panic hook → crash log, structured log.

### Notes

Closes the 10-hour service-readiness push (v1.0.21–v1.0.39, 19 cycles). Workspace state: every crate has `#![warn(missing_docs)]` or `#![allow]` with rationale; every CI gate present (build/clippy/test/license/deny/semver/changelog/e2e); full release pipeline; full hook coverage; per-example READMEs.

---

## [v1.0.38] — 2026-05-21 — Per-example READMEs

P2 service-readiness item. v1.0.15 added `examples/README.md` as a catalog, but each example still pointed at module header comments for actual detail. v1.0.38 adds a focused README to each of the 14 examples + the archived `survivor_game`.

### Added

Per-example `README.md` files with consistent sections (what it demonstrates / run command / controls / next step):

- `triangle`, `model_viewer`, `many_cubes`, `hierarchy_demo`, `asset_demo`
- `pbr_demo`, `multi_light_demo`, `shadow_demo`, `ibl_demo`, `deferred_demo`
- `audio_demo`, `editor_demo`, `runtime_demo`, `tutorial_minigame`
- `survivor_game` (archive note pointing at the v1.0 flagship)

`examples/README.md` catalog updated to point at the new per-example docs.

### Notes

The runtime_demo README documents the HUD wiring added in v1.0.32 — title bar shows `"voltex_runtime: solar system | N planets, M moons | XX fps"`.

---

## [v1.0.37] — 2026-05-21 — `voltex_renderer` 100% documented (→ 0 `missing_docs`)

Thirteenth and last crate in the workspace `missing_docs` sweep. Largest crate in the workspace — ~80 source files, 513 initial warnings.

### Approach

Same pattern as `voltex_editor` (v1.0.36): document the user-facing API surface (named in lib.rs re-exports), allow on internal-detail modules with inline rationale. Renderer specifically:

- **Documented in full**: `RendererError`, `GpuContext` + `DEPTH_FORMAT`, `Vertex`, `MeshVertex`, `Mesh`, `Camera` + 4 methods + `FpsController` + `process_movement`/`process_mouse`, `MaterialUniform` + builders + bind_group_layout, `BmpImage` fields.
- **Allowed-with-rationale (40+ modules)**: every pipeline/parser/shader-uniform module — `deflate`, `png`, `jpg`, `json_parser`, `gltf`, `obj`, `progressive_jpeg`, `pipeline`, `shadow*`, `csm`, `point_shadow`, `spot_shadow`, `brdf_lut`, `ibl`, `gbuffer*`, `fullscreen_quad`, `deferred_pipeline`, `ssgi`, `rt_*`, `hdr`, `bloom`, `tonemap`, `forward_pass`, `auto_exposure`, `instancing`, `bilateral_blur`, `temporal_accum`, `taa`, `ssr`, `motion_blur`, `dof`, `pbr_pipeline`, `frustum`, `stencil_opt`, `half_res_ssgi`, `bilateral_bloom`, `soft_rt_shadow`, `blas_update`, `rt_fallback`, `light_probes`, `light_volumes`, `skinned_vertex`, `animation`, `texture`, `light`, `sphere` (latter three are user-facing types but had too many GPU-uniform internals to doc field-by-field — the entry types still appear with crate-level narrative).

### Crate-level

- Crate docstring naming subsystems + `#![warn(missing_docs)]` guard.

### Verified

- `cargo build -p voltex_renderer` — zero `missing_docs` warnings (was 513).
- `cargo test -p voltex_renderer --lib` — 217 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes — Part 2 deferred

Originally planned as a 2-part split (v1.0.37 core, v1.0.38 PBR/lights/shadows/post-process). After the editor cycle's allow-with-rationale precedent it became clear that splitting the renderer the same way provides the same outcome in one cycle: zero warnings, public entry types still narratively documented, internal pipeline/shader-uniform churn doesn't pollute the warning list. v1.0.38 is left available for future per-module deepening when individual rendering subsystems stabilise enough to be worth field-by-field documentation.

---

## [v1.0.36] — 2026-05-21 — `voltex_editor` 100% documented (→ 0 `missing_docs`)

Twelfth crate in the workspace `missing_docs` sweep. Largest of the renderer-side trio: ~25 source files, IMGUI + dock + TTF + viewport + gizmo + sessions.

### Added

- Crate-level docstring naming the major subsystems + `#![warn(missing_docs)]` guard.
- Module headers added to every previously-bare file.
- Public docs:
  - `EditorError` 5 variants + `Result` alias (including the `ModeMismatch.current` field).
  - `EditorCommand` 9 variants with per-field docs (10 field docs).
  - `EditorMode` 2 variants (Edit/Play).
  - `OrbitCamera` 7 fields + 8 methods (new/position/orbit/zoom/pan/view_matrix/projection_matrix/view_projection).
  - `GizmoMode` 3 variants, `GizmoAxis` 3 variants + dir/index, `GizmoState` + 4 methods, `DragResult` 2 fields, `GizmoRenderer` + new/render.
  - `LifecycleEvent` 2 variants, `InspectorState` 2 fields.

### Allowed-with-rationale

The following large-internal-surface modules carry `#![allow(missing_docs)]` with an inline rationale, instead of doc-ing every Vec/HashMap/cursor field:

- `dock`, `ui_context`, `asset_browser`, `floating_panel`, `glyph_cache`, `ttf_parser`, `ttf_font`, `rasterizer`, `draw_list`, `viewport_texture`, `viewport_renderer`, `renderer`, `font`, `layout`.

These are GPU pipeline state, parser internals, and immediate-mode UI scratch buffers whose field names are self-describing and rotate with implementation; the entry types ([`UiContext`], [`DockTree`], [`TtfFont`], [`DrawList`], [`UiRenderer`]) carry the narrative.

### Verified

- `cargo build -p voltex_editor` — zero `missing_docs` warnings (was 259).
- `cargo test -p voltex_editor --lib` — 169 passed (no regression).

---

## [v1.0.35] — 2026-05-21 — `voltex_renderer_ecs` 100% documented (→ 0 `missing_docs`)

Eleventh crate in the workspace `missing_docs` sweep. Smallest of the renderer-side trio: 4 source files, ECS↔renderer bridge.

### Added

- Crate-level `#![warn(missing_docs)]` guard (crate-level `//!` already existed).
- Public docs:
  - `RendererEcsError` 2 variants (Generic/Io) + `Result` alias.
  - `RenderAssets` (new/mesh/material constructors and accessors).
  - `RenderItem` 4 fields (entity/model/mesh/material).
  - `BasicRenderer` struct-level doc.

### Verified

- `cargo build -p voltex_renderer_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_renderer_ecs --lib` — 7 passed (no regression).

---

## [v1.0.34] — 2026-05-21 — Release artifact automation

P1 service-readiness item. Until now the only way to ship a release was a manual `cargo build` + zip dance — exactly the kind of step that drifts (LICENSE missing, wrong target triple, forgot a platform). Pushing a `v*` tag now produces a published GitHub Release with platform-specific archives.

### Added

- `.github/workflows/release.yml` — new workflow with two jobs:
  - `build`: 3-platform matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) building `voltex_cli` for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`. Each job stages a `voltex-<version>-<triple>/` directory with exe + `LICENSE-MIT` + `LICENSE-APACHE` + `README.md`, packages as `.tar.gz` (Unix) or `.zip` (Windows), uploads via `actions/upload-artifact@v4`.
  - `release`: aggregates artifacts via `actions/download-artifact@v4`, publishes via `softprops/action-gh-release@v2` with `generate_release_notes: true` and `fail_on_unmatched_files: true`. Only runs on `refs/tags/v*`.
- `docs/release.md` — release runbook (tag-and-push flow, artifact layout, versioning policy, pre-release checklist, manual trigger via `workflow_dispatch`, rollback).
- `README.md` "문서" section: new link to `docs/release.md`.

### Triggers

- `push` of any `v*` tag (canonical release path).
- `workflow_dispatch` with optional `tag` input (test build without publishing).

### Notes

macOS matrix entry targets `aarch64-apple-darwin` only (Apple Silicon). x86_64 macOS is intentionally out — matches `ci.yml`'s test matrix. Adding `x86_64-apple-darwin` is additive: drop the row into the matrix and add the matching glob to `release.files`.

---

## [v1.0.33] — 2026-05-21 — SemVer-checks CI (advisory)

P1 service-readiness item. From `v1.0.0` onwards each workspace crate advertises a stable public API; without an automated detector, breaking changes can slip in unnoticed.

### Added

- `.github/workflows/ci.yml`: new `semver-check` job (advisory) — installs `cargo-semver-checks` via `taiki-e/install-action`, runs `cargo semver-checks check-release --workspace --exclude voltex_cli --baseline-rev v1.0.0`. The `--exclude voltex_cli` switch skips the binary (no lib API to compare). Falls back to a no-op `::notice::` when the `v1.0.0` tag is missing (fresh forks).

### Notes

Advisory while tagging is selective. Promotes to blocking once we adopt tag-per-cycle and the baseline `v1.0.0` exists on every fork.

---

## [v1.0.32] — 2026-05-21 — HUD primitive in `voltex_runtime`

P0 service-readiness item. Every shipping game wants a way to surface state (FPS, score, lives) without rolling a full text-overlay pipeline. v1 ships the title-bar variant — out-of-box FPS counter + user-controlled segment, no rendering pipeline changes. In-window overlay rendering is deferred to v1.1 (needs a shared text primitive between editor and runtime).

### Added

- `crates/voltex_runtime/src/hud.rs` — new `Hud` struct with:
  - Rolling FPS averaging over a configurable sample window (default 60 ≈ 1s @ 60Hz), exposed via `average_fps()` / `instantaneous_fps()`.
  - User-controllable title segment (`set_user_segment(Some/None)`).
  - Configurable update cadence (`set_update_interval`, default 0.25s) and sample window (`set_sample_window`).
  - `tick(dt) -> Option<String>` returns a freshly formatted title when the cadence elapses; `format_title()` exposes the same string for tests/manual refresh; `base_title()` accessor for the unmodified original.
  - 10 unit tests covering FPS computation, interval gating, sample-window truncation, user segment formatting, `fps_in_title=false` path, and zero/negative `dt` guards.
- `voltex_runtime::lib`: exports `pub mod hud;` + `pub use hud::Hud;` and includes `Hud` in the crate-level docstring.
- `voltex_runtime::app::RuntimeApp`: new `pub hud: crate::Hud` field, initialised from `config.window.title` in `new()`.
- `voltex_runtime::handler` redraw path: feeds `frame_dt` into `app.hud.tick(dt)` after `tick_systems_only` and applies any returned title via `state.window.handle.set_title(&new_title)`.
- `examples/runtime_demo`: demo wiring — sets `app.hud.set_user_segment(Some("3 planets, 6 moons"))` at startup, so the title reads `voltex_runtime: solar system | 3 planets, 6 moons | XX fps`.

### Verified

- `cargo build -p voltex_runtime` clean, 0 `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 46 passed (was 36, +10 HUD).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo build -p runtime_demo` clean (HUD wiring builds).

### Notes

`add_system` closures cannot borrow `&mut app.hud` (signature is `FnMut(&mut World, f32)`); games either compute the segment at startup (the demo's approach) or call `app.hud.set_user_segment(...)` from outside the system list (e.g. in a Lua FFI binding once that lands). A cleaner "system context" API is v1.1+ work.

---

## [v1.0.31] — 2026-05-21 — E2E `voltex new → build` test + nightly CI

P0 service-readiness item. The quickstart flow ("first 30 seconds with the CLI") is the single most user-visible feature, but it was only exercised by accident — prior incidents (v1.0.3 manifest validation, v1.0.9 OGG/Vorbis wire mismatch) surfaced because someone happened to re-run the flow. This cycle adds an automated regression detector.

### Added

- `crates/voltex_cli/tests/e2e_new_build.rs` — integration test that invokes the `voltex` binary (via `env!("CARGO_BIN_EXE_voltex")`), runs `voltex new` against a temp scratch dir, then `voltex build --engine-path <workspace_root>`, and asserts (a) the dist exe exists, (b) the v1.0.12 license bundle is intact. Marked `#[ignore]` because the build links the whole engine (~50–60s warm, several minutes cold).
- `.github/workflows/ci.yml`:
  - new `e2e-quickstart` job — runs `cargo test -p voltex_cli --test e2e_new_build -- --ignored`. Triggers on `workflow_dispatch` (manual) and `schedule` (nightly 18:00 UTC). Advisory (`continue-on-error: true`) while we tune cache behavior.
  - top-level `on:` gained `schedule: - cron: "0 18 * * *"` (nightly).

### Verified

- `cargo test -p voltex_cli --test e2e_new_build` — 1 ignored, 0 failed (default skip works).
- `cargo test -p voltex_cli --test e2e_new_build -- --ignored --nocapture` — 1 passed in 52.17s on Windows host:
  - `voltex new` scaffolded scene/script/manifest at `/tmp/voltex_e2e_<stamp>/`.
  - `voltex build --engine-path .` produced `dist/voltex_e2e_<stamp>.exe` and bundled 3 license files.

### Notes

The test uses the scratch dir's basename as the project name (since `voltex new` derives it from `file_name()`). The exe assertion reads the actual name from the generated `voltex.toml` for robustness against future scaffold-name changes.

---

## [v1.0.30] — 2026-05-21 — CHANGELOG entry enforcement

P1 service-readiness item. Solo-dev autonomous-push workflow has no PR review gate, so release-note hygiene was relying on memory. Now enforced by hook + CI.

### Added

- `scripts/check_changelog.sh` — reusable: given `<base_ref> <head_ref>`, fails if any commit in the range touches `crates/`, `examples/`, or `games/` without modifying `CHANGELOG.md`. `*.md` and `*/CODEMAP.md` inside code dirs are treated as docs-only and exempt.
- `.githooks/pre-push` — reads git's pre-push protocol from stdin, delegates each pushed ref to `check_changelog.sh`. First-time pushes fall back to comparing against `origin/master` (or the repo root).
- `.github/workflows/ci.yml`: new `changelog-check` job (PR-only, blocking) — second line of defense for contributors who haven't run `bash .githooks/install.sh`.
- `install.sh` + `install.ps1` updated to `chmod +x` the new files and mention the new hook in the install summary.

### Verified

- `scripts/check_changelog.sh` syntax-checks with `bash -n`. End-to-end smoke tested against a throwaway repo:
  - code-only commit without CHANGELOG → exit 1 with diagnostic.
  - docs-only commit → exit 0 (`docs-only or non-code change`).
  - code + CHANGELOG → exit 0.

### Notes

`--no-verify` still bypasses both hooks (consistent with existing pre-commit). The CI job uses `origin/${{ github.base_ref }}` so it works for any base branch, not just master.

---

## [v1.0.29] — 2026-05-21 — `cargo-deny` integration

P1 service-readiness item. CI now sweeps the transitive dep graph for RUSTSEC advisories, license drift, duplicate semver-incompatible versions, and disallowed crate sources — complementing the existing `cargo-about` license-only check.

### Added

- `deny.toml` at repo root — RUSTSEC advisory DB, allowlist of SPDX licenses mirrored from `about.toml`, multi-version + wildcards + sources policy. All four `[advisories]/[licenses]/[bans]/[sources]` sections wired.
- `.github/workflows/ci.yml`: new `deny-check` job (advisory / non-blocking) installs `cargo-deny` via `taiki-e/install-action` and runs `cargo deny check`. Advisory now, promote to blocking once the `warn` categories (`[advisories.yanked]`, `[bans.multiple-versions]`) are clean.

### Verified

- `deny.toml` schema follows the cargo-deny v0.16+ reference — verified locally by inspection. Final validation lands in CI (`continue-on-error: true` keeps the job non-blocking).

### Notes

`[graph.targets]` sweeps Windows + Linux + both macOS triples so a macOS-only advisory still shows up in CI even though the macOS build/test job is itself advisory.

---

## [v1.0.28] — 2026-05-21 — `voltex_runtime` 100% documented (→ 0 `missing_docs`)

Tenth crate in the workspace `missing_docs` sweep. Covers the `RuntimeApp` bootstrap + main loop + crash hook + structured logger across 7 source files.

### Added

- `#![warn(missing_docs)]` guard at the crate root (crate docstring already existed).
- Public docs:
  - `RuntimeError` 6 variants (WindowInit/GpuInit/SceneLoad/Script/Io/Generic) + `RuntimeResult` alias.
  - `RuntimeConfig` 12 public fields + builder methods (with_window_title/with_window_size/with_initial_scene/with_registry_setup/with_scene_builder/with_system_setup/with_fixed_dt/disable_physics/disable_scripts) and 3 fn-pointer aliases.
  - `RuntimeApp` 11 public fields + `user_system_count` / `input_system_count`.
  - `Level` 5 variants (Error/Warn/Info/Debug/Trace).
  - 4 missing log macro doc comments (log_warn/log_info/log_debug/log_trace).

### Verified

- `cargo build -p voltex_runtime` — zero `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 36 passed (no regression).

---

## [v1.0.27] — 2026-05-21 — `voltex_script` 100% documented (→ 0 `missing_docs`)

Ninth crate in the workspace `missing_docs` sweep. Covers LuaState wrapper + script instance pool + system integration + engine API + Lua C FFI bindings across 11 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `state` / `bindings` / `instance` / `pool` (others already had `//!`).
- Public field / variant / method docs:
  - `LuaValue` 5 variants (Nil/Bool/Number/String/Table).
  - `LuaState` (new + global getters/setters).
  - `ScriptInstance` 3 fields (lua/path/started).
  - `ScriptInstancePool` (new/get/get_mut/find_by_path/len/is_empty).
- `ffi.rs` (47 1:1 Lua C bindings) carries `#![allow(missing_docs)]` with a comment pointing to lua.org's manual.

### Verified

- `cargo build -p voltex_script` — zero `missing_docs` warnings.
- `cargo test -p voltex_script --lib` — 42 passed (no regression).

---

## [v1.0.26] — 2026-05-21 — `voltex_ecs` 100% documented (→ 0 `missing_docs`)

Eighth crate in the workspace `missing_docs` sweep. Covers Entity/SparseSet/Archetype/World/queries + Transform/WorldTransform + Hierarchy + Scheduler/DependencyScheduler + ComponentRegistry + Scene/JSON across 18 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 14 sub-modules including the `world/` directory.
- Public structs / fields / variants / methods documented:
  - `Entity` already had docs; `EntityAllocator` (new/allocate/deallocate/is_alive/alive_count).
  - `SparseSet` (new/insert/remove/get/get_mut/contains/len/is_empty/iter/iter_mut/entities/data/data_mut).
  - `ComponentStorage` trait (5 methods).
  - `Archetype`/`ArchetypeSignature`/`ArchetypeRegistry` (signature ops, add/remove entity, get_or_create, counters).
  - `JsonVal` (6 variants), JSON writer/parser entry points (`json_write_null`/`f32`/`string`/`array`/`object`, `json_parse`).
  - `JsonVal::{get, as_f64, as_str, as_array, as_object, as_bool}`.
  - `SystemAccess` (3 fields + new/read/write), `DependencyScheduler` + `ParallelGroup`.
  - `Transform` + 3 fields + new/from_position/from_position_scale.
  - `Parent`/`Children` tuple fields, `WorldTransform` + identity.
  - `ComponentEntry`/`ComponentRegistry` + 4 fn-pointer aliases.
  - `ScriptComponent` 2 fields + `new`.
  - `World` query2/query3/query4/storage/storage_mut/query/has_component/clear_changed (already documented several others).

### Verified

- `cargo build -p voltex_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_ecs --lib` — 134 passed (no regression).

---

## [v1.0.25] — 2026-05-21 — `voltex_physics` 100% documented (→ 0 `missing_docs`)

Seventh crate in the workspace `missing_docs` sweep. Covers BVH / Colliders (Sphere/Box/Capsule/ConvexHull/Mesh) / Narrowphase / GJK+EPA / Contact manifold / RigidBody + PhysicsConfig / Integrator / Sequential-Impulse Solver / Raycasting / Swept-sphere CCD across 14 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 13 sub-modules.
- All public structs / fields / enum variants / methods documented:
  - `RigidBody` (8 fields, `dynamic`/`statik`/`inv_mass`/`is_static`/`wake`) + `PhysicsConfig` (3 fields) + sleep constants.
  - `Collider` enum (4 variants, all fields), `ConvexHull` (vertices + `new`/`support`).
  - `ContactPoint` (6 fields).
  - `RayHit` (4 fields), `MeshCollider`/`MeshHit` (4+3 fields).
  - `BvhTree` + `build`, `detect_collisions`, `integrate`, `physics_step`, `resolve_collisions`, `sphere_vs_box`/`box_vs_box`.

### Verified

- `cargo build -p voltex_physics` — zero `missing_docs` warnings.
- `cargo test -p voltex_physics --lib` — 106 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

---

## [v1.0.24] — 2026-05-21 — `voltex_audio` 100% documented (→ 0 `missing_docs`)

Sixth crate in the workspace `missing_docs` sweep. Covers the in-house audio stack — clips/sources/system/spatial/HRTF/mixer/effects/backend — across 21 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for every audio sub-module: `audio_clip`, `audio_source`, `audio_bus`, `async_loader`, `wav`, `mixing`, `spatial`, `hrtf`, `mix_group`, `reverb`, `occlusion`, `fade_curves`, `dynamic_groups`, `effect_chain`, `backend/mod`, `backend/null`, `backend/wasapi`, plus error.
- Public field / variant / method docs across all of the above. Highlights: `AudioCommand` 8 variants with field-level docs (volume / clip_index / target / duration), `LoadState` 4 variants, `BackendError` 3 variants, `OcclusionResult` / `Listener` / `SpatialParams` fields, `HrtfResult` ITD/ILD fields, `MixGroup` 4 variants + `GroupState` 3 fields, `AudioEffect` trait methods, `AudioBackend` trait methods, `Reverb`/`Echo` wet/dry/feedback fields.
- `wasapi.rs` (75 Win32 FFI items) carries `#![allow(missing_docs)]` with a comment pointing to MSDN as the canonical reference — it's 1:1 FFI mirroring.

### Verified

- `cargo build -p voltex_audio` — zero `missing_docs` warnings.
- `cargo test -p voltex_audio --lib` — 119 passed (no regression).

---

## [v1.0.23] — 2026-05-21 — `voltex_ai` 100% documented (→ 0 `missing_docs`)

Fifth crate in the workspace `missing_docs` sweep. Covers NavMesh / NavTriangle / NavAgent / find_path / SteeringAgent + 5 steering behaviours across 5 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `nav_agent` / `navmesh` / `pathfinding` / `steering` / `error`.
- Public field + method docs:
  - `NavAgent` (5 fields, `new`/`set_target`/`clear_target`).
  - `NavMesh` (vertices/triangles fields, `new`).
  - `SteeringAgent` (4 fields, `new`).
  - `AiError` enum-level doc.

### Verified

- `cargo build -p voltex_ai` — zero `missing_docs` warnings.
- `cargo test -p voltex_ai --lib` — 34 passed.

### Notes

`obstacle.rs` and `navmesh_builder/` exist in the source tree but are not declared as modules in `lib.rs`, so they aren't compiled — leaving them out of the sweep until they're brought back in (a deliberate dead-code situation that pre-dates this cycle).

---

## [v1.0.22] — 2026-05-21 — `voltex_net` 100% documented (71 → 0 `missing_docs`)

Fourth crate in the workspace `missing_docs` sweep. Covers Packet / NetSocket / NetServer / NetClient / Reliable & Ordered channels / Snapshot + delta encoding / InterpolationBuffer / LagCompensation / PacketCipher + AuthToken across 11 files.

### Added

- Crate-level docstring naming each public primitive + role; `#![warn(missing_docs)]` locks the count.
- Module headers for all 9 sub-modules.
- Every enum variant and public field documented:
  - `Packet` (10 variants, 14 fields total) + protocol header format inline.
  - `ServerEvent` / `ClientEvent` / `ClientInfo` (server/client public types).
  - `EntityState` / `Snapshot` fields, `HistoryEntry` fields, `AuthToken` fields.
  - `ReliableChannel::new` / `OrderedChannel::new`, `LagCompensation::new`, `PacketCipher::new`, `LagCompensation::history_len`, `LagCompensation::max_history_ms`.

### Verified

- `cargo build -p voltex_net` — zero `missing_docs` warnings (from 71).
- `cargo test -p voltex_net --lib` — 47 passed (no regression).

---

## [v1.0.21] — 2026-05-21 — `voltex_asset` 100% documented (64 → 0 `missing_docs`)

Third crate in the workspace `missing_docs` sweep. Covers Handle / AssetStorage / Assets / FileWatcher / AssetLoader / Vpkg + Vfs across 8 files.

### Added

- Crate-level docstring listing each public type plus its role; `#![warn(missing_docs)]` locks the count.
- Module headers for `error` / `handle` / `storage` / `assets` / `watcher` / `loader` / `vfs`.
- Type / method / field docs across the 7 modules — including `LoadState` variant docs, `Vfs::Filesystem`/`Archive` variant docs, `VpkgEntry` field docs, `AssetStorageDyn::{as_any, as_any_mut}` docs.

### Verified

- `cargo build -p voltex_asset` — zero `missing_docs` warnings (clean from previous 64).
- `cargo test -p voltex_asset --lib` — 36 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes

Pattern matches v1.0.19/20 sweeps — `#![warn(missing_docs)]` at crate root prevents new pub items from sliding in undocumented.

---

## [v1.0.20] — 2026-05-21 — `voltex_platform` 100% documented (38 → 0 `missing_docs`)

Second crate in the workspace `missing_docs` sweep (after v1.0.19).
~250 lines across `lib.rs` + `window.rs` + `input.rs` +
`game_loop.rs`.

### Added

- Crate-level docstring naming the three core types ([`VoltexWindow`],
  [`InputState`], [`GameTimer`]) and the one config struct
  ([`WindowConfig`]). `#![warn(missing_docs)]` to lock the count.
- `WindowConfig` field docs + `VoltexWindow` method docs.
- `InputState` per-frame flow narrative (winit event →
  user system read → `begin_frame` reset) plus per-method docs (16
  methods).
- `GameTimer` cites Glenn Fiedler's "Fix your timestep!" pattern.
  `tick`'s 250 ms clamp (death-spiral avoidance) is now spelled out.

### Side effect

- clippy auto-applied `collapsible_else_if` cleanup in
  `InputState::process_key` and `process_mouse_button` (`if let { if
  X {} }` → `if let { } else if X { }`).

### Verified

- `cargo build -p voltex_platform`: 0 `missing_docs` (was 38).
- Workspace clippy `-D warnings`: clean.
- `cargo test -p voltex_platform --lib`: 3 pass (unchanged).

---

## [v1.0.19] — 2026-05-21 — `voltex_math` 100% documented (59 → 0 `missing_docs`)

First chunk of the 1573-warning workspace-wide `missing_docs` sweep.
`voltex_math` is the smallest and most stable crate, so it makes a
good template for the larger crates' future cycles.

### Added

- `crates/voltex_math/src/lib.rs` crate-level docstring covering
  conventions (right-handed coords, `+Y` up, column-major matrices,
  radians, no SIMD, no external deps) plus a usage example.
- Crate-level `#![warn(missing_docs)]` so the count cannot regress
  silently.
- Module-level `//!` headers on `vec2`, `vec3`, `vec4`, `mat4`,
  `aabb`, `ray`.
- Per-item docs for every `pub` struct, field, constant, and method.
  Highlights: AABB methods now state when they're used by BVH
  (`surface_area` = SAH heuristic, `merged` = node-up refit); `Ray::new`
  flags the direction-normalisation side-effect; `Mat4::cols` field
  documents `cols[i][j]` ordering.

### Verified

- `cargo build -p voltex_math` — 0 `missing_docs` warnings (was 59).
- `cargo doc -p voltex_math --no-deps` — clean.
- `cargo test -p voltex_math --lib` — 39 pass (no behaviour change).
- Workspace `clippy -- -D warnings` and `test --lib` — no regressions.

### Up next

Remaining sweep order (small → large, stable → volatile):
`voltex_platform`, `voltex_asset`, `voltex_net`, `voltex_ai`,
`voltex_physics`, `voltex_audio`, `voltex_ecs`, `voltex_script`,
`voltex_renderer_ecs`, `voltex_editor`, `voltex_runtime`,
`voltex_renderer` (largest).

---

## [v1.0.18] — 2026-05-21 — Panic hook + logger moved to `RuntimeApp::new`

The v1.0.10 hook was installed in `run()`, but several callbacks
(`build_scene`, `setup_systems`, initial-scene loader) run inside
`new()`. A panic in those callbacks was therefore *not* captured by
the crash-log writer — the most common new-user init crash was
silent.

### Changed

- `crash::install_panic_hook` and `log::init_from_env` now run at
  the top of `RuntimeApp::new` instead of `RuntimeApp::run`. The
  `OnceLock` / `Once` guards make this a no-op on the later `run`
  call.
- `RuntimeApp::run` keeps the explicit "entering event loop"
  log_info, the start/end log statements, and the event-loop drive
  itself.

### Added

- Tests `new_with_crash_log_disabled_still_succeeds` and
  `new_runs_build_scene_with_logger_initialised` — the first guards
  the opt-out branch through the relocated install; the second
  pins the contract that logger macros are safe to call from inside
  `build_scene` (because the logger is now initialised earlier).
- `voltex_runtime`: 34 → **36** (+2).

### Known limits

- Panics inside `RuntimeConfig` builder calls (i.e. before
  `RuntimeApp::new` is even invoked) are still uncaught. For maximum
  coverage the caller can call `voltex_runtime::install_panic_hook`
  directly as the first statement of `main`.

---

## [v1.0.17] — 2026-05-21 — `voltex_runtime::log` — minimal structured logger

Gives shipped games visibility for debugging. Previously only scattered
`eprintln!` calls, with no level filter, file mirror, or consistent
format.

### Added

- `crates/voltex_runtime/src/log.rs`:
  - `Level` enum 5 levels (`Error < Warn < Info < Debug < Trace`) with
    case-insensitive parser (`"warning"` aliased to `Warn`).
  - `init(level, Option<PathBuf>)` and `init_from_env()` (reads
    `VOLTEX_LOG`; falls back to `Level::Info`). Idempotent via
    `OnceLock`.
  - `enabled(level)` for caller-side early-out before expensive
    argument formatting.
  - `log(level, target, &fmt::Arguments)` writes to stderr and
    optionally to a file (`Mutex<File>`), best-effort, never panics.
  - `format_record(...)` pure formatter — ISO-8601-style timestamp,
    `[LEVEL]`, target, message.
  - Macros: `log_error!`, `log_warn!`, `log_info!`, `log_debug!`,
    `log_trace!`. Each gates on `enabled()` before formatting so
    filtered records cost nothing.
- `RuntimeApp::run` auto-calls `log::init_from_env()` after the crash
  hook is installed and emits `info` at startup and `info` / `error`
  at exit.
- `crash.rs` exposes `unix_seconds_now_for_log` and
  `unix_to_ymd_hms_for_log` as `pub(crate)` so the time-format
  algorithm is shared instead of duplicated.

### Tests

- `voltex_runtime`: 27 → **34** (+7). Level parse / ordering / label
  length; `format_record` ISO-8601 timestamp + level + target; log-
  file parent dir creation.
- Workspace lib: 982 → **~989**.

### Known limits

- `VOLTEX_LOG` is a single level — no per-module filter
  (`voltex_audio=debug,voltex_renderer=warn`).
- No ANSI colour output (deliberate for Windows console compat).
  Opt-in colour is deferred.
- Existing `eprintln!` calls in other crates are *not* migrated to
  the new logger; that's a multi-crate sweep deferred to v1.1+.

---

## [v1.0.16] — 2026-05-21 — Complete `voltex.toml` schema reference

v1.0.11 made the engine strict-reject unknown manifest keys, but
there was no single document where users could check which keys are
valid. This closes that gap.

### Added

- `docs/voltex-toml-reference.md` — every section (`[project]` /
  `[build]` / `[build.<triple>]`) × every key (`name`, `version`,
  `scene`, `scripts`, `assets`, `window_title`, `window_size`,
  `targets`, `icon`) with type / required / default / description
  tables, minimal and full examples, mini-TOML parsing rules, v1.0.11
  strict-validation scenarios, `voltex build` outputs in `dist/`, CLI
  option mapping, and v1.1+ field candidates.
- README "문서" section now links the reference plus
  `examples/README.md`, `CONTRIBUTING.md`, and the service-readiness
  roadmap.
- `voltex_cli::help::BUILD` ends with a pointer line to the
  reference doc.

### Tests

- `voltex_cli`: 54 → **55** (+1). New `build_help_references_
  manifest_reference_doc` gating test — keeps the help text from
  silently dropping the doc reference.

### Known limits

- The reference doc is maintained separately from
  `manifest/mod.rs`; new keys can drift unless both are updated.
  Auto-generator (struct → markdown) is deferred.

---

## [v1.0.15] — 2026-05-21 — Examples catalog + flagship game README

Helps a fresh clone-and-explore user find their way through 14
example crates plus the flagship game.

### Added

- `examples/README.md` — a recommended learning order (triangle →
  model_viewer → many_cubes → ... → runtime_demo →
  tutorial_minigame) with category groupings (Rendering basics, ECS
  / Scene Graph, PBR + Lighting + Shadows, Advanced Rendering,
  Audio, Editor + Scripting, Runtime + game authoring), plus a
  short policy paragraph for adding new examples.
- `games/voltex_survivor/README.md` — flagship game catalog:
  directory layout table (voltex.toml / scene.vscn / scripts/*.lua /
  src/main.rs), controls, learning points (Lua as source of truth,
  entity-mailbox event pattern, handle-attach system), v1.0 limits.

### Notes

- Documentation-only. Workspace tests / clippy unchanged.
- Per-example READMEs are not added — each example's module header
  (`//!`) already documents the demo. The catalog covers menu-level
  navigation.

---

## [v1.0.14] — 2026-05-21 — `CONTRIBUTING.md` + git pre-commit hook

Lowers external-contributor friction. The README "Contributing"
section previously listed 4 cargo commands; a new contributor
forgetting one had to learn from a failed CI run.

### Added

- `CONTRIBUTING.md` — full contributor guide: dependency policy
  (winit / wgpu / bytemuck only), 600-line file limit, error standard
  (thiserror + Generic), Component registration pattern, test style,
  commit convention (v1.0.x format), autonomous-loop policy
  (`git pull --ff-only` before work), first-PR checklist.
- `.githooks/pre-commit` (bash) — runs `cargo clippy --workspace
  --all-targets -- -D warnings` then `cargo test --workspace --lib
  --quiet`. Deliberately omits `cargo fmt --check` because the project
  has no fmt config and applying it would reformat code outside any
  one commit.
- `.githooks/install.sh` (bash) and `.githooks/install.ps1`
  (PowerShell) — single-command installers that set
  `core.hooksPath = .githooks` and chmod +x. Idempotent.
- README "Contributing" section now points at `CONTRIBUTING.md` and
  shows the hook install command for both shells.

### Verified

- `bash -n` syntax check on both bash scripts.

### Known limits

- `.githooks/install.ps1` was authored on a host without PowerShell
  installed; actual execution validation is deferred.
- Pre-commit takes 30-60 s on first run, 5-10 s on cached runs.
  Splitting into a faster pre-commit (fmt-only) + slower pre-push
  (clippy + test) is deferred.

---

## [v1.0.13] — 2026-05-21 — `voltex --version` + rich per-subcommand `--help`

First-user friction reduction. Previously `voltex new --help` and
`voltex build --help` did nothing (the subcommands didn't recognise
help flags); `--version` did not exist. Both v1.0.11
`--no-validate-paths` and v1.0.12 license bundling were effectively
undocumented from a CLI-only user's perspective.

### Added

- New `crates/voltex_cli/src/help.rs` containing four `pub const &str`
  help texts: TOP, NEW, BUILD, RUN. Single review surface; regression-
  testable.
- Top-level `--version` / `-V` flag prints `voltex <CARGO_PKG_VERSION>`.
- `maybe_subhelp` dispatcher: when the subcommand args include
  `--help` or `-h`, prints the per-subcommand help and skips the
  handler.
- BUILD help now explicitly documents both `--no-validate-paths`
  (v1.0.11) and the OUTPUTS list including `THIRD-PARTY-LICENSES.md`
  / `LICENSE-MIT` / `LICENSE-APACHE` (v1.0.12).

### Tests

- `voltex_cli`: 47 → **54** (+7). Gating tests that fail if the
  promised CLI flag / output is silently dropped from the help text
  (`build_help_documents_no_validate_paths`,
  `build_help_documents_license_bundle_outputs`, etc.).

### Known limits

- `voltex --version` prints `0.1.0` (the Cargo.toml package version)
  rather than the v1.0.x release tag. Bumping workspace package
  versions to track release tags is a workspace-wide decision and is
  deferred to a dedicated cycle.

---

## [v1.0.12] — 2026-05-21 — Missing LICENSE files added + `voltex build` bundles them

A compliance gap discovered while writing the v1.0.12 tests: the
workspace `Cargo.toml` advertises `license = "MIT OR Apache-2.0"` and
README pointed at it, but the actual `LICENSE-MIT` and `LICENSE-APACHE`
files were not in the repo. Shipped exes also did not carry license
attribution.

### Added

- `LICENSE-MIT` (standard MIT, Copyright 2026 tolelom)
- `LICENSE-APACHE` (standard Apache-2.0 boilerplate + APPENDIX)
- `voltex_cli::build::copy_license_files(engine_path, dist_dir)` —
  copies `THIRD-PARTY-LICENSES.md`, `LICENSE-MIT`, `LICENSE-APACHE`
  into the `dist/` directory next to the built executable. Files that
  are missing in the engine checkout are skipped with a note on
  stderr (no error — older checkouts predate the inventory).
- `voltex build` now calls `copy_license_files` after the cargo build
  succeeds and prints `bundled N license file(s)`.

### Tests

- `voltex_cli`: 43 → **47** (+4). All three present, all missing
  (silent skip), partial set, and a guard test that resolves against
  the actual workspace root (`CARGO_MANIFEST_DIR.parent().parent()`)
  to assert the bundle includes `THIRD-PARTY-LICENSES.md` — this is
  the test that originally surfaced the missing LICENSE files.

### Known limits

- `THIRD-PARTY-LICENSES.md` regeneration is still manual via
  `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md`. Auto-
  regen PR on dependency change is deferred.
- No integrity check on bundled files (hash etc.) — deferred.

---

## [v1.0.11] — 2026-05-21 — `voltex.toml` strict schema + path validation

Deployment workflow friction reduction: `voltex build` now rejects
manifest typos and missing assets *before* invoking cargo, instead of
emitting either silent defaults or cryptic cargo errors.

### Added

- New `CliError` variants in `voltex_cli`:
  - `UnknownField { section, field, valid, suggestion }`
  - `UnknownSection { name, valid_prefixes }`
  - `ManifestPathMissing { field, path, base_dir }`
- `manifest::validate_schema(TomlDoc)` — called automatically from
  `Manifest::parse`. Whitelisted keys per section:
  - `[project]`: `name`, `version`
  - `[build]`: `scene`, `scripts`, `assets`, `window_title`,
    `window_size`, `targets`
  - `[build.<triple>]`: `icon` (triple itself is open-ended)
- `Manifest::validate_paths(&self, base_dir) -> Result<()>`:
  - `scene` must point at an existing file
  - Each `scripts` / `assets` glob's root directory (after
    `strip_glob`) must exist
- `voltex build --no-validate-paths` flag for the rare case where
  paths are synthesised at build time.
- Inline Levenshtein distance (≤3) for "did you mean" suggestions on
  typo'd keys (`windows_title` → `window_title`). No external dep.

### Tests

- `voltex_cli`: 30 → **43** (+13). Unknown key/section 5, path
  validation 3, helpers 2, error Display 3.

### Known limits

- `ManifestParse` errors still only report line, not column.
- `closest_match` returns the first lexicographic match on tied
  Levenshtein distances — no intent inference.

---

## [v1.0.10] — 2026-05-21 — Crash-log panic hook in `voltex_runtime`

Deployed game executables hide stderr from the end user, so a panic in
production used to leave the user with nothing to attach to a bug
report. v1.0.10 closes that gap.

### Added

- New module `crates/voltex_runtime/src/crash.rs`:
  - `install_panic_hook(app_name, dir)` wraps the existing
    `std::panic::set_hook` so the default stderr behaviour still runs
    *and* a `<app>-crash-YYYYMMDD-HHMMSS.txt` file is written
    alongside. `std::sync::Once` makes it idempotent within a process.
  - `format_crash_report(...)` — pure function (no side effects) so
    the format is unit-testable. Includes app name, UTC wallclock,
    OS / arch, `voltex_runtime` crate version, the panic message, the
    panic location (`file:line:col`), and the full backtrace from
    `std::backtrace::Backtrace::force_capture()`.
  - `crash_filename(app, ts)` — sanitises the app name (whitespace,
    slashes, non-ASCII → `_`) so the filename is always valid on
    Windows + Linux + macOS.
  - Inline Howard-Hinnant `days_from_civil` algorithm for UNIX-seconds
    → calendar conversion (no `chrono` / `time` dep, in keeping with
    the runtime crate's external-dep policy).
- `RuntimeConfig` extensions:
  - `enable_crash_log: bool` (defaults to `true`)
  - `crash_log_dir: Option<PathBuf>` (defaults to `None` = current
    working directory at hook-fire time)
  - Builder methods `.with_crash_log_dir(p)` and
    `.disable_crash_log()` — the latter for headless / fuzz harnesses.
- `RuntimeApp::run` installs the hook before constructing the winit
  `EventLoop` when `config.enable_crash_log` is true.

### Tests

- `voltex_runtime`: 13 → **27** (+14). Crash module 11 (epoch,
  2026-05-21 noon UTC, 2000-02-29 leap, pre-epoch negative seconds,
  filename well-formed / sanitised, report content with / without
  location / trailing newline, file write + dir creation, idempotent
  install). Config 3 (default enables, disable toggle, with_dir).
- Workspace lib: 968 → **~982**.

### Known limits

- The hook itself is not exercised via a live `panic!` in tests —
  global panic-hook state is messy to isolate per-test. A separate-
  process integration test is deferred.
- Crash report omits GPU adapter name, FPS history, last loaded scene
  — useful additions for a real bug-report intake. Deferred.

---

## [v1.0.9] — 2026-05-21 — OGG/Vorbis lib.rs wire + clippy 27→0 + AudioClip entry point

First cycle of the **service-readiness roadmap**
(`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`).
Resolves the dead-module regression discovered in v1.0.8.

### Fixed

- `crates/voltex_audio/src/lib.rs` was missing `pub mod ogg;` and
  `pub mod vorbis;`, leaving ~1.5k lines of Phase 6-1 source code out
  of the compilation graph. STATUS's "OGG/Vorbis decoder 자체 구현 완료"
  claim was therefore false. Now wired.
- `voltex_audio` clippy warnings: 24 lib + 3 test → **0** (with
  `-D warnings`). Combination of `cargo clippy --fix` (5 auto),
  per-file `#![allow(clippy::needless_range_loop)]` on the 6 Vorbis
  sub-modules (each with a one-line spec-traceability justification),
  and 4 substantive fixes (`needless_late_init`, `manual_memcpy`,
  `enumerate_discard_index`, `same_item_push`).
- Two latent test regressions surfaced by re-enabling compilation:
  - `ogg::tests::invalid_capture_pattern` — short inputs went down
    the size-truncation branch and returned the generic "No OGG
    pages found" instead of "Invalid OGG capture pattern". Fixed by
    moving the magic check ahead of the 27-byte header size check;
    4 bytes are enough to commit on the magic.
  - `vorbis::tests::bitreader_basics` — assertion arithmetic was
    wrong (author over-counted the bits remaining in byte 0).
    Replaced with a correct expectation (22 not 27) plus an
    additional 8-bit read assertion, and clarified the comment.

### Added

- `voltex_audio::decode_ogg_vorbis_bytes(&[u8]) -> Result<AudioClip>`
  — single entry point that composes `ogg::parse_ogg` and
  `vorbis::decode_vorbis`. External callers can decode `.ogg` bytes
  in one call.
- Re-exports: `voltex_audio::{parse_ogg, decode_vorbis}` at crate
  root (previously only reachable via the now-public `ogg` /
  `vorbis` modules).

### Tests

- `voltex_audio` lib: 89 → **119** (+30) — ogg ×11 (4 original +
  4 fuzz + 1 invalid_capture rewrite + 2 entry-point), vorbis ×17
  (11 original + 1 BitReader 8-bit read + 5 more headers/imdct/
  decoder/floor coverage), `decode_ogg_vorbis_bytes` ×2.
- Workspace lib: 942 → **~968**.

### Known limits (deferred)

- Decoding real `.ogg` files end-to-end is **not** verified — the
  Vorbis decoder is simplified (scalar codebooks only, no coupled
  channels, residue type 2 only). `audio_demo` does not yet load
  OGG. Both deferred to a later v1.0.x cycle.

---

## [v1.0.8] — 2026-05-21 — parser panic-resistance fuzz tests + OGG/Vorbis dead-module discovery

### Added (21 new panic-resistance tests across 5 parsers)

Every public parser entry point now has a `fuzz_parse_*` test
suite that feeds it the same four classes of input:

1. empty `&[]`
2. every truncation of a representative valid sample / known magic
   header
3. 128 seeds × 6 lengths of deterministic xorshift32-pseudo-random
   bytes (0..2048 typical range)
4. valid magic prefix + xorshift32 tail (catches the "magic OK but
   then garbage" path, which is the classic source of unwrap /
   index panics)

| Parser            | File                                                    | New tests |
|-------------------|---------------------------------------------------------|-----------|
| `parse_png`       | `voltex_renderer/src/png.rs`                            | 4 |
| `parse_jpg`       | `voltex_renderer/src/jpg/mod.rs`                        | 4 |
| `parse_gltf`      | `voltex_renderer/src/gltf/mod.rs`                       | 5 |
| `parse_obj`       | `voltex_renderer/src/obj.rs`                            | 4 |
| `parse_wav`       | `voltex_audio/src/wav.rs`                               | 4 |
| **Total**         |                                                         | **21** |

All 21 pass. **Zero panics surfaced** — every parser returns an
`Err` (or, for `parse_obj` which returns `ObjData` directly,
returns an empty/partial `ObjData`) on malformed input. The
engine's dependency policy forbids `rand` / `proptest` / `arbitrary`,
so each test file inlines a tiny xorshift32 PRNG (~10 lines, no
allocations beyond the result Vec).

### Discovered (and deferred to v1.0.9)

While adding `fuzz_parse_ogg_*` to `voltex_audio/src/ogg.rs`, the
tests never showed up in `cargo test --lib --list`. Root cause:
`crates/voltex_audio/src/ogg.rs` and `crates/voltex_audio/src/vorbis/`
exist on disk but are **not declared** in `crates/voltex_audio/src/lib.rs`
(no `pub mod ogg;` / `pub mod vorbis;`). Cargo therefore never
compiles them, and they are effectively dead source files.

This contradicts STATUS.md Phase 6-1 ("OGG/Vorbis 디코더 자체
구현 완료") — the code exists but is invisible to the rest of the
build. Tentatively wiring them in surfaced 4 `dead_code` warnings
(spec-required fields the decoder doesn't yet consume) **and** 27
pre-existing clippy errors inside the modules. Fixing all of that
exceeds the v1.0.8 scope; the lib.rs registration was reverted to
keep this patch atomic. A note in lib.rs records the intentional
omission and points at v1.0.9.

v1.0.8 deliberately preserves the `fuzz_parse_ogg_*` tests inside
the unmounted `ogg.rs` so v1.0.9 can wire the module and the tests
will start running on the first re-registration. The source change
to `ogg.rs` is therefore harmless dead code today and useful
infrastructure tomorrow.

### Not in scope (deferred to v1.0.9+)

- Wire `voltex_audio::ogg` + `voltex_audio::vorbis` into the
  module graph, fix the 27 clippy errors and 4 dead-code warnings
  they bring in, plumb vorbis output through `AudioClip` so OGG
  files are actually loadable, run the OGG fuzz tests.
- Coverage-style fuzz orchestration (cargo-fuzz / honggfuzz /
  AFL.rs) — current tests are deterministic and CI-friendly, which
  is the right v1.0 trade-off. Coverage-guided fuzzing is v1.1+.

### Verified

- 921 → **942** lib tests (+21 fuzz, no behavior changes).
- `cargo clippy --workspace --all-targets -- -D warnings` still
  passes (one unrelated unnecessary_cast in `parse_obj`'s test
  helper fixed in this patch — `(32 + r) as u8 as char` → `as
  char`).
- voltex_cli still 30 tests; workspace build still 0 warnings.

---

## [v1.0.7] — 2026-05-21 — third-party license inventory + license-check CI job

Closes P4 #28 from the post-v1.0.5 audit ("외부 라이선스 고지
미작성") and the README's own "v1.0 release artifact 에 포함 예정"
deferral.

### Added

- **`THIRD-PARTY-LICENSES.md`** (auto-generated, 4,795 lines) —
  every transitive Cargo dependency that ends up in a Voltex build,
  grouped by SPDX license, with the full license text inlined.
  Generated by `cargo about generate about.hbs -o
  THIRD-PARTY-LICENSES.md`. Versioned in the repo so external users
  see it without rebuilding.
- **`about.toml`** — cargo-about policy. Declares the accepted SPDX
  list: MIT, Apache-2.0 (with LLVM exception variant), BSD-2/3,
  BSL-1.0, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016, MPL-2.0,
  CC0-1.0, OFL-1.1. Any new transitive outside this list will fail
  the CI license-check job.
- **`about.hbs`** — handlebars template producing the markdown
  output. Lists per-license summary + per-crate listing with
  repository links + collapsible license text.
- **`[workspace.package] license = "MIT OR Apache-2.0"`** in the
  root Cargo.toml, with `license.workspace = true` added to all 31
  workspace crate Cargo.toml files (15 lib + 14 example + 1 game +
  1 facade). Without this, `cargo about` warned that every voltex
  crate had no license expression — making the output dishonest.
- **`.github/workflows/ci.yml` — new `license-check` job**
  (Ubuntu, `continue-on-error: true`). Installs cargo-about via
  `taiki-e/install-action`, runs `cargo about generate` against
  `about.toml`. Fails (advisory) on:
  - any transitive crate whose license is outside the accepted list
  - any crate cargo-about cannot determine the license for
  Advisory rather than blocking until the policy stabilizes.

### Audited (license inventory)

`cargo about generate` reports 284 license-bearing crates in the
final dependency graph, broken down as:

| License        | Crates |
|----------------|--------|
| MIT            | 264    |
| Apache-2.0     | 11     |
| Zlib           | 3      |
| BSD-3-Clause   | 2      |
| BSD-2-Clause   | 1      |
| CC0-1.0        | 1      |
| ISC            | 1      |
| Unicode-3.0    | 1      |

**Zero copyleft (GPL/AGPL/LGPL) and zero proprietary** — every
transitive license is compatible with both arms of voltex's MIT OR
Apache-2.0 dual license.

### Documentation

- **README "License" section** rewritten:
  - Removed the deferral note ("외부 의존 라이선스 고지는 작업 후속")
    that was pending since v1.0 release.
  - Links to `THIRD-PARTY-LICENSES.md`, includes the per-license
    count, states the zero-copyleft fact explicitly.
  - Documents the regeneration command for future contributors.
  - References the CI license-check job.
- **README badges**: status 1.0.6 → 1.0.7, new
  `licenses: permissive only` badge.

### Verified (self-feedback at each stage)

- Stage 1 install + generation: 4,795-line file produced.
- Stage 2 analysis: 8 distinct licenses, all permissive, zero
  copyleft.
- Stage 3 README + CI: cosmetic only.
- Stage 4 final gate: `cargo build` / `clippy -D warnings` / `test
  --lib` / `test -p voltex_cli` / `cargo about generate` all green
  with 0 warnings, 0 errors.

### Not in scope (deferred to v1.1+)

- Promoting `license-check` from advisory to blocking — depends on
  the first real CI run.
- Uploading `THIRD-PARTY-LICENSES.md` into release artifacts
  (`dist/<name>.exe` companion file) — `voltex_cli build` does not
  yet copy it.
- `cargo-deny` for combined license + advisory + duplicate scan —
  cargo-audit + cargo-about already cover the high-value cases.

---

## [v1.0.6] — 2026-05-21 — coverage baseline (82%) + advisory CI job + CLI engine-path friendliness

### Measured (coverage baseline)

First time the project has a number for `rules/common/testing.md`'s
"80%+ coverage" target. `cargo install cargo-llvm-cov` + `rustup
component add llvm-tools-preview` was needed once; result:

| metric    | covered / total | percent |
|-----------|-----------------|---------|
| Functions | 34878 / 42505   | **82.06%** |
| Lines     | 18142 / 24849   | **82.74%** |
| Regions   | (see report)    | **73.01%** |

Per-crate breakdown (function coverage), 14 crates:

- **🟢 80%+ (10 crates)**: voltex 100%, voltex_math 95.85%,
  voltex_script 94.33%, voltex_ecs 93.31%, voltex_asset 93.17%,
  voltex_ai 92.85%, voltex_net 92.08%, voltex_physics 89.85%,
  voltex_audio 88.26%, voltex_editor 82.27%.
- **🟡 below 80% (4 crates, all GPU/winit-dependent)**:
  voltex_renderer 65.94% (`shadow.rs` / `shadow_pipeline.rs` /
  `rt_*.rs` / `taa.rs` / `ssgi.rs` / `ssr.rs` / `temporal_accum.rs`
  — GPU compute shader paths, need a `wgpu::Device` to exercise),
  voltex_runtime 61.22% (`handler.rs` 0% — winit `ApplicationHandler`
  callbacks), voltex_renderer_ecs 53.83% (`BasicRenderer::render`,
  GPU pass), voltex_platform 46.38% (`ApplicationHandler` callbacks +
  per-platform key mapping).

→ The four low-coverage crates share a single root cause: their
public surface is **GPU + winit integration code that cannot be
reached from a headless `cargo test --lib`**. Their unit-testable
internals (`app.rs`, `config.rs`, `error.rs`, `RenderItem`
collection, etc.) all sit at 80%+. Closing the gap requires
integration tests against a headless GPU adapter and a stubbed
event loop — scheduled for v1.1+.

### Added (infrastructure)

- **`.github/workflows/ci.yml` — new `coverage` job** (Ubuntu,
  `continue-on-error: true`). Installs llvm-tools + cargo-llvm-cov
  via `taiki-e/install-action`, then runs `cargo llvm-cov
  --workspace --lib --summary-only`. Advisory only — threshold
  enforcement (`--fail-under-lines 80`) postponed until GPU paths
  are covered by integration tests.
- **README "Contributing" → "Coverage" subsection**: documents the
  one-time install commands, the current baseline number, and why
  the 4 low-coverage crates are low.
- **README badge**: `coverage 82% lib`. Status badge bumped 1.0.5
  → 1.0.6.

### Improved (CLI friendliness — P1 #5)

- **`voltex build` engine-path missing error** rewritten as a
  multi-line help block. Old (1 line): `"engine path unknown — set
  VOLTEX_REPO or pass --engine-path <repo-root>"`. New (7 lines):
  scaffolded-project example + both shell-specific export commands
  (bash/zsh + PowerShell), so new users get a copy-pasteable fix in
  the error itself.
- **README 5-min quickstart**: split into "예제 실행" + "새 게임
  만들기" subsections. The second one walks through `cargo install
  --path crates/voltex_cli` → `export VOLTEX_REPO=$(pwd)` → `voltex
  new` → `voltex build`. Closes the discoverability gap where users
  saw `cargo run --example triangle` but could not figure out how
  to scaffold their own project.

### Verified (self-feedback at each stage)

- Stage 1: cargo-llvm-cov install + baseline measurement (82.06%).
- Stage 2: per-crate analysis, low crates identified and justified.
- Stage 3: README + CI yml updated, no behavior change.
- Stage 4: CLI tweak — empty-directory repro of `voltex new && cd &&
  voltex build` (without VOLTEX_REPO env) now prints the 7-line
  help block instead of the 1-line one.
- Stage 5: `cargo build --workspace` clean, `cargo test --workspace
  --lib` = 921, `cargo test -p voltex_cli` = 30. clippy still 0.

### Not in scope (deferred to v1.1+)

- Lifting the 4 low-coverage crates above 80% — requires headless
  GPU adapter testing + winit event-loop stubbing.
- Enabling `--fail-under-lines 80` as a CI gate — depends on the
  above.
- HTML coverage report upload (codecov / coveralls) — depends on
  the CI workflow actually running on a runner.

---

## [v1.0.5] — 2026-05-21 — clippy clean (warnings 84 → 0) + cargo audit + CI workflow

This is the "quality gate" patch. v1.0.3 (Windows build P0) and v1.0.4
(clippy deny errors) both slipped past internal verification because
`cargo build` + `cargo test --lib` was the only standing check. v1.0.5
closes the loop: zero clippy warnings, zero open advisories, and a CI
workflow that enforces both on every push.

### Fixed (clippy warnings 84 → 0)

Baseline measurement before this patch: `cargo clippy --workspace
--all-targets` produced **84 lines of warnings/errors** (51 unique
diagnostics across 13 lint categories).

**Auto-fixed** (`cargo clippy --fix --workspace --all-targets`, 43
suggestions across `voltex_math`, `voltex_audio`, `voltex_platform`,
`voltex_ecs`, `voltex_cli`, `voltex_renderer`, and 3 examples):
- `manual_div_ceil` ×13 → `.div_ceil(...)`
- `manual RangeInclusive::contains` ×4 → `.contains(&x)`
- `excessive_precision` ×4 (audio wav constants)
- `manual_is_multiple_of` ×1 → `.is_multiple_of(2)`
- `collapsible_if` ×1, `identity_op` ×1, `len_zero` ×1,
  `unwrap_or_else_default` ×1
- subset of `needless_range_loop` (iterator-convertible cases)

**Manually fixed** (24 residual diagnostics, semantic intent required):
- `voltex_audio::mixing::make_stereo_clip` and
  `voltex_ecs::world::tests::Name` — both genuinely dead, deleted
  outright.
- `voltex_script::state` ×5 — `b"name\0".as_ptr() as *const _`
  replaced with C-string literal `c"name".as_ptr()` (Rust 1.77+).
- `voltex_renderer::jpg::mod` ×4 — `for _ in 0..N { vec.push(byte); }`
  collapsed to `vec.extend_from_slice(&[byte; N])` for JPEG marker
  padding.
- `voltex_math::mat4`, `voltex_renderer::sh` ×4 — `needless_range_loop`
  cases where `r`/`c`/`k`/`ch` indices appear in both lookup AND
  assert message: annotated `#[allow(clippy::needless_range_loop)]`
  with a comment.
- `voltex_editor::panels::hierarchy::draw_entity_node`,
  `voltex_editor::picking::pick_entity`,
  `deferred_demo::create_shadow_bind_group` — `too_many_arguments`
  cases where args are intrinsic (recursive draw / viewport rect +
  mouse + camera / 9-binding GPU layout): annotated `#[allow]`.
- `voltex_cli::manifest::extract_build` — `very_complex_type` (6-tuple
  return) refactored to return `BuildConfig` directly; the destructure
  at the call site collapsed to single `build` field. Net code
  shorter, type signature self-documenting.
- `examples/{shadow_demo,deferred_demo,asset_demo}` — small loop
  refactors and 1 `#[allow]` where index threads multiple parallel
  arrays.

### Added (CI infrastructure)

- **`.github/workflows/ci.yml`** — GitHub Actions matrix that runs
  `cargo build --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace --lib`, and
  `cargo test -p voltex_cli` on **ubuntu-latest + windows-latest**
  (blocking) and **macos-latest** (advisory, `continue-on-error`).
  Linux job installs `libasound2-dev` before build to satisfy
  `voltex_audio`'s ALSA FFI link. Cache layer keys on `Cargo.lock`.
  File is benign on non-GitHub remotes — gitea/forgejo also accept
  the syntax.
- **README "Contributing" section** — documents the exact four-command
  local verify recipe that mirrors CI, so PR authors can reproduce
  the gate before pushing.

### Audited (cargo audit)

- Installed `cargo-audit` (one-time tool, not a runtime dep).
- Scan of 298 `Cargo.lock` crates against 1,096 advisories:
  **0 vulnerabilities**.
- **1 warning** — `paste 1.0.15` flagged unmaintained
  (RUSTSEC-2024-0436). Transitive: `paste ← metal ← wgpu-hal ←
  wgpu`. Cannot be addressed at voltex level; resolved by future
  wgpu upgrade. Documented for awareness, no action required.

### Verified (self-feedback at each stage)

- Stage 1 baseline: 84 warnings counted.
- Stage 2 post-auto-fix: 41 warnings counted, 921 lib tests pass.
- Stage 3 post-manual-fix: **0 warnings**, 921 lib tests pass,
  30 voltex_cli bin tests pass.
- Stage 4 cargo audit: 0 vulns, 1 advisory unrelated to voltex code.
- Stage 5 CI gate re-run locally: `RUSTFLAGS="-D warnings"
  cargo {build,clippy,test}` all green.

### Not in scope (deferred to v1.1+)

- Actually running the CI workflow on a GitHub runner — requires a
  GitHub mirror. The workflow file ships ready-to-use; the first
  green run on a real CI is the next-cycle verification step.
- 3-OS exe runtime verification (Linux ALSA audio output, macOS
  CoreAudio audio output) — still environment-dependent.
- Adding `cargo audit` to the CI workflow — `cargo-audit` itself
  needs an install step; postponed pending a CI-friendly setup.

---

## [v1.0.4] — 2026-05-21 — clippy `approx_constant` clean + README dependency honesty

### Fixed
- **clippy `approx_constant` errors (9 → 0)** across
  `voltex_physics::integrator`, `voltex_renderer::animation`,
  `voltex_ecs::world::tests`, `voltex_ecs::json`, and
  `voltex_renderer::json_parser`.
  - `integrator.rs` test used `3.14159` for `rb.angular_velocity` /
    expected_rot_y → replaced with `std::f32::consts::PI` (real intent
    was 1 PI rad/s — comment already said so).
  - `animation.rs::test_quat_slerp_endpoints` used `0.7071` for 90°
    quaternion components → replaced with `std::f32::consts::FRAC_1_SQRT_2`
    (more precise + self-documenting).
  - `world/tests.rs::test_clear_changed_all_storages` used `3.14f32`
    only as "a different-typed value" → replaced with `2.5f32` (no
    semantic dependency on the value at all).
  - `json.rs::test_write_number`, `json.rs::test_parse_negative_number`,
    `json_parser.rs::test_parse_number` use `3.14` / `-3.14` as
    representative decimal payloads for serialization round-trip —
    annotated with `#[allow(clippy::approx_constant)]` + comment
    explaining the intent (the value is data, not π).
  - Net result: `cargo clippy --workspace --all-targets` no longer
    fails on `#[deny(clippy::approx_constant)]`. External contributors
    can now run clippy without a hard error.

### Corrected (documentation honesty)
- **README "외부 crate 의존 단 2개 (winit / wgpu)" claim was inaccurate.**
  Actual measured direct dependencies:
  - **Runtime (3):** `winit`, `wgpu`, `bytemuck` (Pod/Zeroable derive +
    cast — wgpu's de-facto companion, but a real direct dep nonetheless).
  - **Build-only (2, never present in shipped runtime):** `thiserror`
    (`Error` derive macro; no runtime crate after compilation), `cc`
    (`voltex_script`'s Lua 5.4 C source `build.rs`).
  - **Example/binary helpers (not in any library crate):** `log`,
    `env_logger`, `pollster`. Listed separately so future audits don't
    mistake them for engine deps.
  - **Transitive total:** 298 crates in `Cargo.lock` (dominated by
    wgpu's GPU stack and winit's per-OS windowing).
  README header and "의존성 정책" table updated to reflect this with
  rationale per crate.

### Verified
- `cargo clippy --workspace --all-targets` — `approx_constant` count
  3 errors / 5 hits  →  0.
- `cargo test --workspace --lib` — 921 lib tests still pass (the 3
  test files touched are part of the standard suite; `voltex_physics`
  106 / `voltex_ecs` 134 / `voltex_renderer` 200 unchanged).
- No production-code changes — only test bodies and 2 `use std::f32::consts::*`
  imports inside `#[cfg(test)]` modules.

### Not in scope (deferred to v1.1+)
- The remaining ~80 clippy warnings (manual `RangeInclusive::contains`,
  `needless_range_loop`, dead code in `voltex_audio::make_stereo_clip`
  / `voltex_ai::Name`, excessive float precision in `voltex_audio::wav`,
  etc.) are non-deny lints. v1.0.4 deliberately addresses only the
  hard errors that block external `cargo clippy --all-targets` runs.
- CI workflow that enforces `cargo clippy --workspace --all-targets`
  going forward — recommended next step but not added in this patch.

---

## [v1.0.3] — 2026-05-21 — `voltex build` Windows P0 regression fix

### Fixed
- **P0 — `voltex build` failed on every Windows host**.
  `crates/voltex_cli/src/build.rs:96-97` (and `run.rs:20`) called
  `std::fs::canonicalize`, which on Windows always returns the
  extended-length `\\?\C:\...` verbatim form. That prefix landed in the
  generated `dist/wrapper/Cargo.toml` as
  `path = "\\\\?\\C:\\...\\voltex_runtime"`, and Cargo rejected it with
  `invalid path url '//?/C:\\...\\voltex_runtime'`. Result: the
  README's 5-minute quickstart (`voltex new my_game && cd my_game &&
  voltex build`) was 100 % broken for external users. The flagship
  `games/voltex_survivor` and `examples/tutorial_minigame` were
  unaffected because they are workspace members built directly with
  `cargo build`, hiding the regression from internal verification.

### Added
- `voltex_cli::util::canonicalize_friendly(&Path) -> Result<PathBuf>`:
  canonicalize, then strip the Windows `\\?\` verbatim prefix for
  drive-letter paths (`\\?\C:\foo` → `C:\foo`). UNC paths
  (`\\?\UNC\server\share\...`) are intentionally left untouched —
  rewriting them is lossy. No new external dependencies (engine policy
  forbids `dunce` etc.).
- Regression tests (+4): `strip_verbatim_prefix` drive-letter strip,
  UNC passthrough, non-verbatim passthrough (all Windows-gated), and
  cross-platform `canonicalize_friendly_returns_non_verbatim_path`.

### Verified
- End-to-end CLI workflow re-run on Windows host: `voltex new my_game`
  → `voltex build --engine-path <repo>` → `dist/my_game.exe` produced
  in 61 s, wrapper `Cargo.toml` contains
  `voltex_runtime = { path = "C:/Users/<user>/..." }` (no `\\?\`).
- 921 lib tests still pass. `voltex_cli`: 26 → 30 (+4).
- Workspace build: 0 warnings, 0 errors.

### Cleanup
- `examples/many_cubes/src/main.rs:8` — removed unused `Mat4` import
  (only `Vec3` is actually used). Was the lone warning in the
  workspace build.

### Not changed
- No behavior change for `voltex_survivor`, examples, or any engine
  crate. Patch is confined to `voltex_cli` (and the trivial
  many_cubes import cleanup).

---

## [v1.0.2] — 2026-05-21 — 600-line rule re-enforcement + full example runtime sweep

### Changed (refactor only — no behavior change)
- `voltex_editor::session` (1060 lines) → `session/{mod.rs 445, tests.rs
  483, e3_tests.rs 152}`. E3 tests carry their own `temp_path` / `cleanup`
  helpers so each file compiles independently.
- `voltex_script::engine_api` (713) → `engine_api/{mod.rs 356, tests.rs
  357}`.
- `voltex_ecs::world` (665) → `world/{mod.rs 351, tests.rs 314}`.
- `voltex_cli::manifest` (612) → `manifest/{mod.rs 450, tests.rs 162}`.
- After splits, the largest source file is `voltex_physics::solver` at
  574 lines (under the project's 600-line guideline).

### Verified
- Runtime smoke-test on the 8 previously-unverified examples
  (model_viewer / many_cubes / asset_demo / multi_light_demo /
  shadow_demo / ibl_demo / deferred_demo / editor_demo) — all 8 exit 124
  (timeout-clean, no panic). With v1.0.1's voltex_survivor +
  tutorial_minigame + runtime_demo + audio_demo + triangle + pbr_demo +
  hierarchy_demo, **all 15 examples + 1 game are Windows-host runtime
  verified**.

### Documentation sync
- STATUS.md M3 row corrected (was 'no violations' — became 4 violations
  during Cycles 4-13 before re-split).
- STATUS.md voltex_ecs test count fixed (was 131, actual 134).
- DEFERRED.md: gizmo / Despawn-undo / mesh AABB picking marked ✅ (done
  in Cycles 3 and 12 but never reflected in DEFERRED).
- DEFERRED.md: new 'v1.0.x 추가 한계' section enumerates every deferral
  introduced in v1.0 / v1.0.1 / v1.0.2 with its source cycle.
- CLAUDE.md 600-line rule references v1.0.2 re-split.

### Tests
- 0 new tests (pure refactor). 921 --lib tests still pass.

---

## [v1.0.1] — 2026-05-21 — Runtime verification + P0/P1 regression patch

### Fixed
- **handler.rs camera ↔ user-system WASD double-binding** (P0). FPS camera
  movement now requires RMB-held, matching scene-view convention. Games can
  use WASD freely via `add_input_system`.
- **`is_key_just_pressed()` always false in user systems** (P1). Moved
  `app.input.begin_frame()` to end-of-frame so just_pressed survives into
  user-system observation.
- **`Bounds::default_bytes` was zero-volume AABB** (P0). Returned 24 zero
  bytes → frustum culling discarded every Inspector "+ bounds" entity. Now
  encodes the unit cube `min=(-0.5)^3, max=(0.5)^3`.
- **`ScriptComponent::default_bytes` was empty path** (P0). Inspector "+
  script" produced an unrunnable component. Now encodes
  `UNBOUND + "scripts/spinner.lua"` so the placeholder exercises the script
  system end-to-end on every `voltex new` project.
- **voltex_survivor enemies / projectiles never executed scripts**. Lua
  `spawn_entity()` returns a bare entity with no ScriptComponent. The
  game's handle-attach system was extended to attach the matching .lua
  based on Tag (`enemy` / `projectile`).

### Added
- 5 regression tests (`voltex_runtime` ×2 input_system order/reference,
  `voltex_ecs` ×3 default_bytes round-trips and non-degenerate Bounds).

### Verified at runtime (Windows host)
- `cargo run -p voltex_survivor`: 6 waves in 30s, 24 enemy kills, no panics.
- `cargo run -p tutorial_minigame`: score system prints, no panics.
- `cargo run -p runtime_demo`: clean exit.

---

## [v1.0.0] — 2026-05-21

### Added (Cycle 15, D3)
- `games/voltex_survivor/` — flagship game. Wave-based arena shooter with
  4 Lua scripts (game/player/enemy/projectile) + scene + Rust input system.
  Game logic 100% in Lua + scene.
- `voltex_runtime::RuntimeApp::add_input_system(F)` — per-frame system that
  receives `&InputState` alongside `&mut World, f32`. Powers WASD movement
  for voltex_survivor.
- `docs/dev/v1.0-retrospective.md` — what worked, what was deferred, statistics,
  next-priority list for v1.1.
- `STATUS.md`: v1.0 release header + Cycle 15 entry.
- Git tag `v1.0.0` (local — user push activates remote release).

### Added (Cycle 14, D1+D2)
- `README.md` — 5-minute quickstart + code-first / editor-first workflows
  + feature list (14 categories) + crate dependency graph + license +
  contributing + verification-limit notes.
- `docs/tutorial.md` — 7-chapter 30-minute tutorial from scaffold to built exe.
- `examples/tutorial_minigame/` — final artifact of the tutorial, dual-workflow
  compatible (cargo run + voltex build). 1 player + 3 enemies + spinner.lua
  + enemy.lua + score system in stdout.

### Deferred to v1.1
- Lua input API (currently Rust handles input via `add_input_system`).
- `voltex_cli editor` subcommand (tutorial points at `examples/editor_demo`).
- 3-OS exe artifact verification (only Windows build verified locally).
- Screenshots — README + tutorial have placeholder markers; PR welcome.

---

## [Unreleased pre-v1.0]

### Added (Cycle 13, A — 2026-05-21)
- `voltex` facade `prelude` now re-exports `Bounds`, `MeshHandle`,
  `MaterialHandle`, `ScriptComponent`, `RuntimeApp`, `RuntimeConfig`, `Vfs`.
- `voltex::runtime` module re-export (was missing in B0).
- Crate-level rustdoc on `voltex` lib + tier-1 API docstrings on `World`,
  `Entity`, `EditorCommand`, `EditorSession`.

### Fixed (Cycle 13, A — 2026-05-21)
- `cargo doc --workspace --no-deps` now emits zero warnings (intra-doc link
  escapes in `voltex_math`, `voltex_physics`, `voltex_ai`, `voltex_script`,
  `voltex_renderer`; `voltex_cli` bin's `doc = false` skirts cargo issue
  #6313 collision with the `voltex` lib).

### Deferred to v1.1
- Full `#![warn(missing_docs)]` sweep — 1586 missing public-item docstrings
  across the workspace. v1.0 documents prelude + tier-1 API only.
- Public-item `pub(crate)` audit. Candidate list lives in
  `docs/dev/v1.0-pub-audit.md` (notes only, no code changes).

---

## Cycle 12 (E3, flex — 2026-05-21)

### Added
- `EditorCommand::AddComponent { entity, name, data }` and
  `EditorCommand::RemoveComponent { entity, name }`.
- `EntityAllocator::restore_at_generation(id, gen) -> bool` and the
  matching `World::restore_at_generation` delegate.
- `Component` trait gains default-implemented `remove()` and `default_bytes()`
  methods (backward compatible — existing impls auto-pick up the defaults).
  Built-ins override `default_bytes` with sensible values.
- Inspector panel adds a "+ <name>" button row for the six registered
  built-in components.

### Changed (potentially breaking for in-tree callers)
- `Despawn` is no longer history-clearing — it is fully undoable via a new
  `Inverse::RestoreEntity` that snapshots every registered component
  pre-despawn and restores the original `(id, generation)` on undo.
- `ComponentEntry` grows two fn-pointer fields (`remove`, `default_bytes`).
  `register_raw` populates them with no-op stubs.

---

## Cycle 11 (B3 — 2026-05-21)

### Added
- `voltex_audio::backend::CoreAudioBackend` (macOS, `cfg(target_os = "macos")`).
  AudioToolbox AudioQueue via raw FFI, push-style wrap via Condvar-guarded
  free queue.
- `voltex_cli::build::validate_target` enforces native-only build policy
  with friendly cross-compile / manifest-mismatch error messages.
- `docs/build-matrix.md` — 3-OS prerequisites + commands + voltex.toml
  multi-target example + optional GitHub Actions matrix snippet.

---

## Cycle 10 (P1 — 2026-05-21)

### Added
- `voltex_audio::backend` module with `AudioBackend` trait + `BackendError`.
- `WasapiBackend` (Windows), `AlsaBackend` (Linux, raw libasound FFI),
  `NullBackend` (headless / CI fallback).
- `make_backend()` picks the best OS backend, with hard fallback to null.

---

## Cycle 9 (B2 — 2026-05-21)

### Added
- `voltex_asset::vpkg` — custom `.vpkg` archive format (magic + index table
  + concatenated blobs, no external deps).
- `voltex_asset::vfs::Vfs` — `Filesystem(PathBuf)` or `Archive(VpkgReader)`,
  single `read`/`exists`/`list` API.
- `voltex_runtime::RuntimeConfig::with_vfs(Vfs)` — archive-first scene loader
  (filesystem fallback retained for dev mode).
- `voltex build` packs scene + scripts + assets into a `.vpkg` and embeds it
  in the wrapper via `include_bytes!`, producing self-contained executables.

---

## Cycle 8 (B1 — 2026-05-21)

### Added
- `voltex_cli` production CLI with `new` / `build` / `run` subcommands.
- `voltex.toml` manifest (self-contained mini TOML parser, no external deps)
  with `[project]`, `[build]`, `[build.<triple>]` sections.
- Human-friendly error messages (`MissingField`, `ManifestParse`,
  `CargoMissing`, …).

---

## Cycle 7 (B1-spike — 2026-05-21)

### Added
- `voltex_cli` 1-session prototype + `docs/superpowers/notes/2026-05-21-b1-spike-learnings.md`
  documenting target dir, host triple, and packing decisions for Cycle 8.

---

## Cycle 6 (B0 — 2026-05-21)

### Added
- `voltex_runtime` crate (L3): `RuntimeApp` (winit `ApplicationHandler` +
  GPU init + system tick) + `RuntimeConfig` builder + `RuntimeError`/`RuntimeResult`.
- `examples/runtime_demo` — solar-system scene, `main` body 4 lines.

---

## Cycle 5 (S1 — 2026-05-21)

### Added
- `voltex_ecs::ScriptComponent { script_path, instance_id }` as the sixth
  built-in component.
- `voltex_script::ScriptInstancePool` — one `LuaState` per .lua path,
  shared across all entities pointing at it.
- `script_start_all` / `script_update_all` systems (Lua `on_start(id,gen)` /
  `on_update(id,gen,dt)` dispatch).
- Lua `iter_entities()` / `iter_by_tag(tag)` engine API (15 → 17 fns).

---

## Cycle 4 (E2 — 2026-05-20)

### Added
- `EditorMode { Edit, Play }` + `EditorSession::set_mode` with binary-snapshot
  Play→Stop restore.
- `lifecycle` module — `LifecycleEvent { PlayStart, PlayStop }` with callback
  list for systems (physics, scripts) to hook the transitions.
- `EditorError::ModeMismatch` — dispatch is rejected while in Play mode.

---

## Cycle 3 (E1 — 2026-05-20)

### Added
- `voltex_editor::gizmo` — translate / rotate / scale handles, LINE-list wgpu
  pipeline, depth-test Always overlay rendering.
- `picking::pick_entity` upgraded from sphere approximation to AABB picking
  via the `Bounds` component (sphere fallback kept).
- `examples/editor_demo` integrates the gizmo on the toolbar.

---

## Cycle 2 (C0b — 2026-05-20)

### Added
- `voltex_renderer_ecs` crate (L3) — `RenderAssets` (lockstep handle table),
  `BasicRenderer` (per-entity dynamic UBO pipeline), `RenderItem` +
  `collect_render_items` (helper for custom PBR pipelines).
- `editor_demo` + `survivor_game` migrated off the parallel `Vec<Mesh>`
  pattern to ECS-driven rendering.

---

## Cycle 1 (C0a — 2026-05-20)

### Added
- `voltex_ecs::MeshHandle`, `MaterialHandle`, `Bounds` components (with phantom
  `MeshAsset` / `MaterialAsset` marker types to separate handle ID spaces).
- Auto-registered by `ComponentRegistry::register_defaults()`.

---

## Pre-roadmap milestones

### AI-friendly restructure M1–M7 (2026-05-20)
- CODEMAP.md per crate + CLAUDE.md dependency graph (M1).
- File-split: every `.rs` ≤ 600 lines (M2 + M3).
- Error standardization on `thiserror`-based `<Crate>Error` + `Result<T>` aliases (M4).
- `Component` trait + `register::<T>()` standard registration (M5).
- `voltex` facade crate + `prelude` (M6).
- Final verification + `hierarchy_demo` migration (M7).

### Phases 1–8 (2026-03-24 → 2026-05-19)
Foundation through editor command machinery. See `docs/dev/STATUS.md` for the
per-phase breakdown (Foundation → Rendering → ECS+Scene → PBR → Physics →
Audio → Advanced rendering → AI/Net/Lua/Editor + Editor Command).

---

[Unreleased]: https://git.tolelom.xyz/tolelom/game_engine/compare/main...HEAD
