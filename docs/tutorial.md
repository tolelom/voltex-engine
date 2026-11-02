# Voltex 튜토리얼

> 빈 프로젝트를 만들고, 회전하는 큐브와 플레이어를 쫓는 적이 있는 미니 게임을 거쳐, `voltex build`로 실행 파일 하나까지 만든다. 완성본은 `examples/tutorial_minigame/`에 있다.

## 1. 프로젝트 만들기

### 1.1 준비

| OS | 필요한 것 |
|---|---|
| Windows 10·11 x64 | Rust stable, Visual Studio 2022 Build Tools |
| Ubuntu 22.04 이상 | Rust stable, `libasound2-dev` |
| macOS 13 이상 arm64 | Rust stable, `xcode-select --install` |

### 1.2 voltex CLI 설치

```bash
git clone https://git.tolelom.xyz/tolelom/game_engine voltex
cd voltex
cargo install --path crates/voltex_cli
```

`voltex --help`가 동작하면 된다.

### 1.3 새 프로젝트

```bash
cd ..        # 엔진 저장소 옆에서 작업
voltex new minigame
cd minigame
```

`voltex new`가 만드는 구조:

```
minigame/
├── voltex.toml           # 매니페스트(프로젝트 이름, 빌드 설정)
├── scene.vscn            # 빈 씬 {"version":1,"entities":[]}
├── scripts/
│   └── spinner.lua       # 예시 스크립트
└── assets/
    └── .gitkeep
```

`voltex.toml`은 이렇게 채워진다.

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

필드 전체는 [voltex-toml-reference.md](voltex-toml-reference.md).

## 2. 에디터와 첫 씬

### 2.1 에디터 실행

에디터는 `examples/editor_demo`로 실행한다. 엔진 저장소에서:

```bash
cd /path/to/voltex
cargo run -p editor_demo
```

### 2.2 엔티티 만들기

툴바의 Spawn 버튼을 누르면 씬에 엔티티가 하나 생긴다. Hierarchy 패널에서 고르면 오른쪽 Inspector에 속성이 나온다.

Inspector의 Transform 슬라이더로 위치를 바꾸면 뷰포트의 큐브가 바로 움직인다. 에디터의 모든 변경은 `EditorCommand`로 처리되므로 다음이 함께 일어난다.

1. `scene.vscn`에 저장
2. `.log` 파일에 JSON 한 줄로 기록
3. 툴바 Undo 버튼으로 되돌리기 가능

Spawn과 위치 변경을 한 번씩 한 뒤 `scene.vscn`을 열면 transform 컴포넌트를 가진 엔티티 하나가 들어 있다.

### 2.3 저장

툴바 Save 버튼은 자동 저장과 같은 일을 지금 바로 한다. `scene.vscn`은 JSON이라 텍스트 에디터로 읽고 git diff로 비교할 수 있다.

## 3. Lua 스크립트

### 3.1 spinner.lua

`voltex new`가 만든 `scripts/spinner.lua`:

```lua
function on_start(id, gen) end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
```

- 엔진 API는 엔티티를 `(id, gen)` 정수 두 개로 받는다.
- `get_rotation`이 `nil`을 돌려주면 이미 사라진 엔티티다. 그냥 넘어간다.
- `on_start`, `on_update`는 없어도 된다.

### 3.2 엔티티에 스크립트 붙이기

Inspector 아래의 "+ script" 버튼을 누르면 `ScriptComponent`가 붙는다. 경로 기본값은 `scripts/spinner.lua`다. Inspector에는 경로를 바꾸는 입력란이 없으므로 다른 스크립트를 쓰려면 `scene.vscn`에서 경로를 고치거나 `examples/tutorial_minigame/scene.vscn`을 참고한다.

### 3.3 Play와 Stop

툴바 Play 버튼을 누르면 현재 씬을 저장해 두고 스크립트를 실행한다. Stop을 누르면 저장해 둔 상태로 돌아간다.

```
        ┌──────────────┐                ┌──────────────┐
 Edit → │ 씬 스냅샷 저장 │ → Play 모드 →  │ 스크립트 실행  │
        └──────────────┘                └──────────────┘
                                               │
        ┌──────────────┐                       │
 Edit ← │ 스냅샷 복원    │ ←── Stop ─────────────┘
        └──────────────┘
```

Play 중에는 Spawn, 위치 변경 같은 편집이 `EditorError::ModeMismatch`로 거부된다. 복원되는 것은 `ComponentRegistry`에 등록된 컴포넌트뿐이다.

Spawn → "+ script" → Play 순서로 누르면 큐브가 돈다.

## 4. 적과 입력

### 4.1 입력

키 입력은 Lua에서는 전역 `input` 테이블(`input.is_key_pressed` 등), Rust에서는 `RuntimeApp::add_input_system`으로 받는다. 이 튜토리얼은 단순하게 하려고 입력 없이 플레이어가 저절로 움직이게 한다.

### 4.2 플레이어: Rust

```rust
fn register_player_system(app: &mut RuntimeApp) {
    app.add_system(|world, dt| {
        let players: Vec<_> = world.query::<Tag>()
            .filter(|(_, t)| t.0 == "player")
            .map(|(e, _)| e)
            .collect();
        for e in players {
            if let Some(t) = world.get_mut::<Transform>(e) {
                t.position.x += dt * 1.0;   // 매 프레임 +X로 이동
            }
        }
    });
}
```

### 4.3 적: Lua

`scripts/enemy.lua`:

```lua
-- 매 프레임 플레이어 쪽으로 이동한다.
function on_update(id, gen, dt)
    local ex, ey, ez = get_position(id, gen)
    if ex == nil then return end

    local p_id, p_gen = find_by_tag("player")
    if p_id == nil then return end

    local px, py, pz = get_position(p_id, p_gen)
    if px == nil then return end

    local dx = px - ex
    local dz = pz - ez
    local len = math.sqrt(dx*dx + dz*dz)
    if len < 0.05 then return end   -- 충분히 가까우면 멈춤
    local speed = 0.5 * dt
    set_position(id, gen, ex + dx/len * speed, ey, ez + dz/len * speed)
end
```

여러 엔티티가 같은 스크립트 경로를 가리키면 Lua 상태 하나를 함께 쓴다. 엔티티별 값은 전역 변수가 아니라 `(id, gen)`으로 구분한다.

큐브 하나에 `Tag("player")`, 큐브 셋에 `Tag("enemy")`와 `ScriptComponent("scripts/enemy.lua")`를 붙이고 Play하면 적이 플레이어 쪽으로 모인다.

## 5. 점수

살아 있는 적의 수를 점수로 삼아 바뀔 때마다 콘솔에 찍는다.

```rust
fn register_score_system(app: &mut RuntimeApp) {
    let mut last_count: usize = usize::MAX;
    app.add_system(move |world, _dt| {
        let enemies = world
            .query::<Tag>()
            .filter(|(_, t)| t.0 == "enemy")
            .count();
        if enemies != last_count {
            println!("[score] enemies alive: {}", enemies);
            last_count = enemies;
        }
    });
}
```

화면 위에 그리려면 `RuntimeConfig::with_hud(true)`로 HUD를 켜고 `RuntimeApp::set_hud_fn`에서 그린다.

## 6. 빌드와 실행

### 6.1 빌드

`minigame` 폴더에서:

```bash
voltex build
```

`dist/`에 실행 파일이 생긴다. `--target windows-x64`·`linux-x64`·`macos-arm64`로 대상을 적을 수 있지만, 지금 쓰는 OS와 다른 대상은 거부한다.

```
voltex: cross-compile not supported.
  current host: windows-x64
  requested target: macos-arm64
  hint: run `voltex build --target macos-arm64` on a macos-arm64 machine.
```

다른 OS용 실행 파일은 그 OS에서 `voltex build`를 다시 돌린다. OS별 준비물은 [build-matrix.md](build-matrix.md).

### 6.2 실행

```bash
./dist/minigame           # Linux·macOS
./dist/minigame.exe       # Windows
```

씬, 스크립트, 에셋이 `.vpkg`로 묶여 실행 파일 안에 들어가므로 `dist/` 폴더를 다른 곳으로 옮겨도 실행된다.

## 7. 더 해 볼 것

| 주제 | 시작점 |
|---|---|
| 물리 | `examples/editor_demo`는 Play 때 큐브에 RigidBody와 Collider를 붙인다(`EditorSession::add_lifecycle`) |
| 3D 모델 | `voltex_image::parse_gltf`로 glTF·GLB를 읽는다. `assets/`에 두면 `voltex build`가 함께 묶는다 |
| PBR 재질 | `examples/pbr_demo` |
| 그림자·IBL | `examples/shadow_demo`, `multi_light_demo`, `ibl_demo`, `deferred_demo` |
| 오디오 | `examples/audio_demo` |
| 네트워크 | `voltex_net`의 스냅샷과 보간 |
| 완성된 게임 | `games/voltex_survivor`(Lua), `games/voltex_depths`(Rust) |
