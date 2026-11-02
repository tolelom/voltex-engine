# Voltex 30분 튜토리얼

0 에서 시작해 약 30분 안에 다음 두 가지를 끝낸다:

1. **빈 Voltex 프로젝트** 를 만들고
2. **회전하는 큐브 + 적 + 점수 표시** 가 있는 미니 게임을
3. **`voltex build`** 로 단일 실행파일로 묶기

최종 결과물은 `examples/tutorial_minigame/` 에 들어있으니, 막히면 거기와
비교하면 된다. 각 chapter 끝의 `git checkout` 힌트가 chapter 별 진행
상태로 점프하게 해 준다.

> **메모**: 본 튜토리얼은 텍스트 + ASCII 다이어그램 중심이다. 스크린샷
> 마커 `<screenshot: ...>` 는 PR 환영 — v1.0 release 후 사용자 머신에서
> 캡쳐된 이미지를 받아 채울 예정이다.

---

## Chapter 1 — 프로젝트 만들기 (3 min)

### 1.1 사전 요구

| OS | Rust | 추가 |
|---|---|---|
| Windows 10/11 x64 | stable | Visual Studio 2022 Build Tools |
| Ubuntu 22.04+ | stable | `libasound2-dev` |
| macOS 13+ arm64 | stable | `xcode-select --install` |

### 1.2 voltex CLI 설치

워크스페이스 안에서 `voltex_cli` 를 빌드해 PATH 에 둔다:

```bash
git clone https://git.tolelom.xyz/tolelom/game_engine voltex
cd voltex
cargo install --path crates/voltex_cli
```

`voltex --help` 가 동작하면 성공.

### 1.3 새 프로젝트 스캐폴드

```bash
cd ..        # voltex repo 옆에서 작업
voltex new minigame
cd minigame
ls
```

`voltex new` 는 다음 구조를 생성한다:

```
minigame/
├── voltex.toml           # 매니페스트 (project name + build 설정)
├── scene.vscn            # 빈 씬 {"entities":[]}
├── scripts/
│   └── spinner.lua       # sample 스크립트
└── assets/
    └── .gitkeep
```

`voltex.toml` 내용은 자동 채워진다:

```toml
[project]
name = "minigame"
version = "0.1.0"

[build]
scene = "scene.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
window_title = "minigame"
window_size = [1280, 720]
```

> 체크포인트: 이 시점의 결과는 `examples/tutorial_minigame/` 의 Chapter 1
> 커밋 상태와 같다.

---

## Chapter 2 — 에디터 실행 + 첫 씬 (5 min)

### 2.1 에디터 진입점

현재 v1.0 의 에디터 진입점은 `examples/editor_demo` 다 (`voltex_cli` 의 editor
모드는 v1.1 후속). voltex 워크스페이스 안에서:

```bash
cd /path/to/voltex
cargo run -p editor_demo
```

`<screenshot: editor_demo 실행 직후 화면, 도크 + 빈 viewport>`

### 2.2 Spawn + Inspector 사용

도크 좌상단 Toolbar 의 **Spawn** 버튼 → 빈 씬에 entity 1 개가 만들어진다.
Hierarchy 패널에 `Entity { id, gen }` 라벨이 보이고, 클릭하면 우측
Inspector 가 활성화된다.

Inspector 에서 Transform 슬라이더로 `Pos X / Y / Z` 를 조정해 본다 — 뷰포트
큐브가 실시간으로 움직인다. Inspector 의 모든 변경은 `EditorCommand` 로
dispatch 되므로 자동으로:

1. `scene.vscn` 으로 다시 저장됨
2. `.log` 파일에 jsonl 한 줄로 기록됨
3. 한 단계 undo 가능 (Ctrl-Z 핫키 v1.1, 일단 Toolbar 의 Undo 버튼)

> 체크포인트: Spawn 1 회 + SetTransform 1 회 후 `scene.vscn` 의 내용을
> 열어 보면 `{"entities":[{...,"components":[{"name":"transform","data":...}]}]}`
> 형태의 한 entity 가 들어 있다.

### 2.3 Save → 외부 에디터로도 확인

Toolbar **Save** 버튼은 명시적 저장 (auto-save 와 동일 효과). 외부 텍스트
에디터로 `scene.vscn` 을 열어보면 JSON 으로 사람이 읽을 수 있다.

---

## Chapter 3 — Lua 스크립트 추가 (5 min)

### 3.1 spinner.lua 살펴보기

`voltex new` 가 만들어준 `scripts/spinner.lua` 의 내용:

```lua
-- spinner.lua — sample 스크립트. ScriptComponent 로 entity 에 붙이면
-- 매 프레임 Y 축 회전.
function on_start(id, gen) end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
```

핵심 규약:
- 모든 engine API 는 `entity` 핸들 = `(id, gen)` 정수 두 개로 인자 받는다.
- `get_rotation` 이 `nil` 을 반환하면 stale 핸들 → silent skip.
- `on_start` / `on_update` 는 옵션이다. 함수 없으면 silent skip.

### 3.2 entity 에 ScriptComponent 부착

에디터 Inspector 하단의 **"+ script"** 버튼 → 해당 entity 에 `ScriptComponent`
가 default_bytes (instance_id = UNBOUND, path = "") 로 추가됨.

> v1.0 한계: Inspector 에는 path 텍스트 입력 UI 가 아직 없다. 임시 우회:
> `scene.vscn` 을 외부 에디터로 열어 해당 entity 의 script 컴포넌트 데이터
> 4 바이트 (instance_id = `ffffffff`) 뒤에 path UTF-8 바이트를 직접 채워 넣는다.
> 또는 본 튜토리얼의 권장 — `examples/tutorial_minigame/scene.vscn` 를 참고로
> 통째로 복사.

### 3.3 Play 모드 진입

Toolbar **Play** 버튼:

```
        ┌─────────────┐                ┌─────────────┐
 Edit → │ snapshot 캡쳐│ → Play 모드 →  │ scripts 실행 │
        └─────────────┘                └─────────────┘
                                              │
        ┌─────────────┐                       │
 Edit ← │ snapshot 복원 │ ←── Stop ───────────┘
        └─────────────┘
```

Play 중에는 dispatch (Spawn / SetTransform / Despawn 등) 가 모두
`EditorError::ModeMismatch` 로 거부된다. Stop 누르면 Play 직전 상태가
완벽히 복원 (등록된 컴포넌트만 한정 — 자세한 정책은 [E2 spec](superpowers/specs/2026-05-20-e2-play-stop-lifecycle-design.md)).

> 체크포인트: `cargo run -p editor_demo` → Spawn → "+ script" →
> path 채우기 → Play → 큐브가 회전해야 함.

---

## Chapter 4 — 적 + 입력 (7 min)

### 4.1 입력 정책

Lua engine_api 는 **입력을 노출하지 않는다** (v1.1 후속). 입력 처리는
Rust 측 `main.rs` 의 사용자 시스템이 담당하고, Lua 는 "한 entity 의 상태
변경 / 검색 / 충돌 검사 같은 게임 로직" 에 집중한다.

### 4.2 player 동작 — Rust 측

`src/main.rs` 의 사용자 시스템에서:

```rust
fn register_player_input(app: &mut RuntimeApp) {
    app.add_system(|world, dt| {
        // input 은 (현 시점에) world 에 직접 노출되지 않음 — 사용자가 직접
        // 자기 InputState 를 system 캡쳐로 공유하거나, 아래처럼 Tag 로
        // entity 를 찾아 미리 정해진 동작만 함.
        let players: Vec<_> = world.query::<Tag>()
            .filter(|(_, t)| t.0 == "player")
            .map(|(e, _)| e)
            .collect();
        for e in players {
            // 예: 매 프레임 +X 로 천천히 이동
            if let Some(t) = world.get_mut::<Transform>(e) {
                t.position.x += dt * 1.0;
            }
        }
    });
}
```

(완전한 입력 통합은 v1.1 의 `InputState as ECS resource` 작업 — 본 튜토리얼은
"Player 가 자동 전진" 으로 단순화)

### 4.3 enemy.lua — AI 측

`scripts/enemy.lua`:

```lua
-- enemy.lua — 매 프레임 player 위치로 천천히 이동.

function on_update(id, gen, dt)
    local ex, ey, ez = get_position(id, gen)
    if ex == nil then return end

    -- find_by_tag 는 (id, gen) 두 정수를 반환.
    local p_id, p_gen = find_by_tag("player")
    if p_id == nil then return end

    local px, py, pz = get_position(p_id, p_gen)
    if px == nil then return end

    -- (player - enemy) 방향으로 0.5 unit/sec.
    local dx = px - ex
    local dz = pz - ez
    local len = math.sqrt(dx*dx + dz*dz)
    if len < 0.05 then return end  -- 너무 가까우면 멈춤
    local speed = 0.5 * dt
    set_position(id, gen, ex + dx/len * speed, ey, ez + dz/len * speed)
end
```

각 enemy entity 에 `ScriptComponent { script_path = "scripts/enemy.lua" }`
를 붙이면 같은 LuaState 가 여러 entity 에 공유된다 (`ScriptInstancePool` 의
"path 당 1 LuaState" 패턴).

> 체크포인트: 큐브 1 개에 `Tag("player")`, 큐브 3 개에 `Tag("enemy")` +
> `ScriptComponent("scripts/enemy.lua")` → Play → 적들이 player 를 향해 모임.

---

## Chapter 5 — UI + 점수 (5 min)

### 5.1 immediate-mode UI

Voltex 의 에디터 UI 는 자체 immediate-mode 위젯이다 (외부 GUI crate 0).
런타임 게임에서도 같은 `UiContext` 를 쓸 수 있다.

```rust
// main.rs 의 system 안:
let mut score = 0u32;
app.add_system(move |world, dt| {
    let _ = dt;
    // (예: 매 프레임 score = enemy 수)
    let enemy_count = world.query::<Tag>()
        .filter(|(_, t)| t.0 == "enemy")
        .count();
    score = enemy_count as u32;
});

// UI 그리기는 RuntimeApp 의 with_ui_callback 으로 별도 등록 (v1.1
// 정식 API). v1.0 은 직접 UiContext 를 외부에서 만들어 호출하거나,
// editor_demo 의 hud_panel 구조를 참고.
```

본 튜토리얼은 v1.0 의 한계 (런타임에서 UiContext 통합 미정식화) 를
인정하고, "점수 = 살아 있는 enemy 수" 를 콘솔 로그로 표시:

```rust
println!("[score] enemies alive: {}", score);
```

> v1.1 의 HUD 통합 후 본 chapter 가 진짜 그래픽 UI 로 업그레이드된다.

---

## Chapter 6 — Save + Build (3 min)

### 6.1 Save

에디터에서 Toolbar **Save** 누르면 `scene.vscn` 갱신. 외부 텍스트
에디터로 열어 git diff 가능.

### 6.2 Build

`minigame` 폴더 안에서:

```bash
voltex build --target windows-x64    # 호스트가 Windows
# 또는
voltex build --target linux-x64
voltex build --target macos-arm64
```

`voltex_cli` 는 **native-only** 정책 — 호스트 OS 와 다른 target 을 요청하면
다음과 같이 거부:

```
voltex: cross-compile not supported.
  current host: windows-x64
  requested target: macos-arm64
  hint: run `voltex build --target macos-arm64` on a macos-arm64 machine.
```

다른 OS exe 가 필요하면 그 OS 머신에서 다시 `voltex build`. 자세한 사전
요구는 [docs/build-matrix.md](build-matrix.md).

### 6.3 Run

```bash
./dist/minigame           # Linux / macOS
./dist/minigame.exe       # Windows
```

`dist/` 폴더는 단독 이동 가능 — scene + scripts + assets 가 `.vpkg` 로
exe 안에 임베드되어 있다.

> 체크포인트: `minigame.exe` 가 다른 폴더 (또는 USB) 로 옮겨도 동작해야 함.

---

## Chapter 7 — 다음 단계 (2 min)

축하 — 첫 voltex 게임을 빌드하고 단독 exe 까지 만들었다. 다음에 시도해 볼
것들:

- **물리** — `examples/editor_demo` 의 Play 모드는 cube 들에 RigidBody +
  Collider 를 자동 부착한다. `lifecycles` 콜백 (E2) 으로 직접 등록해 본다.
- **3D 모델** — `voltex_renderer::gltf::load_gltf` 로 .glb 임포트.
  `assets/` 에 두면 `voltex build` 가 자동 패킹.
- **PBR 머티리얼** — `MaterialUniform` + ORM 텍스처. `examples/pbr_demo`.
- **그림자 + IBL** — `examples/shadow_demo` / `multi_light_demo` /
  `ibl_demo` / `deferred_demo`.
- **오디오** — `voltex_audio::AudioSystem` + WAV/Vorbis 디코딩 + 3D
  positional. `examples/audio_demo`.
- **멀티플레이어** — `voltex_net` + Snapshot + Interpolation. v1.1
  데모 추가 예정.
- **멀티 OS exe** — Mac / Linux 머신에서 같은 `voltex build` 실행.
  [docs/build-matrix.md](build-matrix.md) 참고.

전체 spec / plan 은 `docs/superpowers/` 에 cycle 별 보존되어 있다 — 어떤
기능이 어떻게 진화했는지 추적 가능.

질문 / 버그 / PR — git repo 의 issue tracker 에. Korean / English 둘 다
환영.

---

## 참고: tutorial_minigame 의 git history

`examples/tutorial_minigame/` 는 본 튜토리얼의 최종 산출물이다. 각
chapter 끝의 "체크포인트" 와 다음 커밋 hash 가 1:1 매핑되어 있어, 막히면
`git log examples/tutorial_minigame` 로 비교 가능:

- Ch 1: 빈 스캐폴드
- Ch 2: scene.vscn 에 player cube 1개
- Ch 3: player cube 에 spinner.lua 부착
- Ch 4: enemy 3개 + enemy.lua 추가
- Ch 5: src/main.rs 의 score 시스템
- Ch 6: build 가능 상태 (Cargo.toml + voltex.toml 최종)

(현재 cycle 14 단독 커밋으로 1 회 묶여 들어가지만, 향후 사용자가 각 chapter
별로 분리된 PR / branch 로 학습 흐름을 따라갈 수 있게 의도되어 있다.)
