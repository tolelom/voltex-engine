# 기여 안내

> Voltex Engine 코드를 고칠 때 지키는 규칙과 커밋 전에 돌리는 검사.

## 개발 환경

- Rust stable과 clippy(`rustup component add clippy`)
- Linux는 ALSA 헤더: `sudo apt-get install -y libasound2-dev`
- 선택 도구: cargo-llvm-cov(커버리지), cargo-about(의존 crate 라이선스 고지 생성, `cargo install --locked cargo-about --features cli`), cargo-deny(취약점·라이선스 검사, 설정은 `deny.toml`)
- 파서 퍼징: [fuzz/README.md](fuzz/README.md)

## 커밋 전 검사

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test -p voltex_cli
```

voltex_cli는 바이너리 crate라 `--lib`에 잡히지 않아 따로 돌린다.

GPU가 필요한 테스트는 `#[ignore]`로 빠져 있다. GPU가 있는 기기에서 `cargo test --workspace --tests -- --ignored`로 돌린다.

### git 훅

```bash
git config core.hooksPath scripts/hooks
```

훅은 `scripts/hooks/`에 있고, 실제 검사는 `scripts/`의 스크립트가 한다.

- 커밋할 때: clippy와 `cargo test --workspace --lib`
- push할 때: engine/, tools/, examples/, games/를 바꿨는데 CHANGELOG.md를 고치지 않았으면 막는다. aarch64 Linux 대상 clippy도 돈다(타깃이 없으면 건너뜀).

한 번 건너뛸 때는 `--no-verify`.

## 코드 규칙

### 의존성

엔진 외부 crate는 wgpu·winit(GPU와 창)과 작은 보조 crate(bytemuck, thiserror, log, pollster)만 쓴다. Lua는 C 소스를 저장소에 넣어 `cc`로 빌드하고, env_logger는 예제와 게임에서만 쓴다. 나머지는 직접 구현한다. 새 외부 crate는 추가하기 전에 먼저 논의한다.

의존 crate를 바꾸면 `scripts/gen_third_party_licenses.sh`로 `THIRD-PARTY-LICENSES.md`를 다시 만든다. `voltex build`가 이 파일을 게임 `dist/`에 같이 넣는다.

### 파일 크기

한 파일은 600줄 이하로 둔다. 넘으면 책임별로 하위 모듈로 나눈다.

### 에러 처리

에러를 돌려주는 엔진 crate는 `src/error.rs`에 `<Crate>Error` enum과 `Result<T>` alias를 둔다(thiserror).

- 공통 variant는 `Generic(String)`과 `#[from] std::io::Error`. `From<String>`, `From<&str>`이 있어 `?`와 `"메시지".into()`로 문자열 에러를 바로 올릴 수 있다.
- `voltex_editor`의 `EditorError`는 `PartialEq`·`Clone`을 유지하려고 io 에러를 `Io(String)`으로 담는다.
- 에러 타입이 없는 crate: `voltex_math`, `voltex_platform`, `voltex_physics`.
- 도구는 예외다. `voltex_cli`는 외부 crate 없이 직접 쓴 `CliError`, `voltex_capture`는 `Result<_, String>`.

### 컴포넌트

씬 파일에 저장할 컴포넌트는 `voltex_ecs::Component`를 구현하고 `ComponentRegistry`에 등록한다. 등록하지 않은 컴포넌트는 `serialize_scene_json`, `serialize_scene_binary` 결과에 빠진다.

```rust
use voltex_ecs::{Component, ComponentRegistry, Entity, World};

struct Health(u32);

impl Component for Health {
    const NAME: &'static str = "health"; // 바꾸면 기존 씬 파일을 못 읽는다

    fn serialize(world: &World, entity: Entity) -> Option<Vec<u8>> {
        let h = world.get::<Health>(entity)?;
        Some(h.0.to_le_bytes().to_vec())
    }

    fn deserialize(world: &mut World, entity: Entity, data: &[u8]) -> voltex_ecs::Result<()> {
        let bytes: [u8; 4] = data
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or("health: 4바이트 미만")?;
        world.add(entity, Health(u32::from_le_bytes(bytes)));
        Ok(())
    }
}

let mut registry = ComponentRegistry::new();
registry.register_defaults(); // Transform, Tag, MeshHandle, MaterialHandle, Bounds, ScriptComponent
registry.register::<Health>();
```

씬 파일은 사용자가 고칠 수 있는 입력이라 `deserialize`는 짧거나 깨진 데이터에도 패닉 없이 에러를 돌려준다. trait를 구현할 수 없는 외부 타입은 `register_raw(name, ser, deser)`로 등록한다.

### 시스템

시스템은 `fn(&mut World)` 함수나 같은 모양의 클로저다. `RuntimeApp`을 쓰는 게임은 `RuntimeConfig::with_system_setup`에서 등록하고, 단독으로 돌릴 때는 시스템 함수를 원하는 순서로 직접 호출한다. (`voltex_ecs::experimental::scheduler::Scheduler`도 같은 일을 하지만 엔진은 쓰지 않는 실험 API다.)

### import

게임과 예제는 facade `voltex`로 가져온다. `use voltex::prelude::*`에 수학, ECS 핵심, 입력, `RuntimeApp` 같은 자주 쓰는 타입이 있고, 렌더러·물리처럼 영역이 정해진 타입은 `voltex::renderer::GpuContext`, `voltex::physics::RigidBody`처럼 모듈 경로로 쓴다.

### 테스트

- 단위 테스트는 모듈 안 `#[cfg(test)] mod tests`. 길어지면 같은 폴더의 `tests.rs`로 뗀다.
- 통합 테스트는 crate의 `tests/`.

### 커밋

- 한 커밋에 한 가지 변경. 모듈 분리와 기능 변경은 나눈다.
- 메시지는 `<type>(<범위>): <요약>`. type은 feat, fix, refactor, test, docs, build, chore, ci. 범위는 crate나 게임 이름(`fix(physics)`, `feat(leap)`).
- 코드를 바꾸면 CHANGELOG.md의 Unreleased 절에 적는다. 공개 API가 바뀌면 그 crate의 `CODEMAP.md`도 고친다.

## 라이선스

보낸 코드는 이 저장소와 같은 조건(MIT 또는 Apache-2.0 중 선택)으로 배포된다.
