# ECS Integration Design (Audio + AI)

## Overview

AudioSource와 NavAgent ECS 컴포넌트를 추가하여 오디오와 AI를 엔티티 시스템에 연결한다.

## Scope

- AudioSource 컴포넌트 + audio_sync_system
- NavAgent 컴포넌트 + nav_agent_system
- 각 crate에 voltex_ecs 의존성 추가

## AudioSource

`crates/voltex_audio/src/audio_source.rs`

```rust
use voltex_math::Vec3;

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub clip_id: u32,
    pub volume: f32,
    pub spatial: bool,
    pub looping: bool,
    pub playing: bool,
    pub played_once: bool,  // 이미 재생 시작했는지 (중복 방지)
}

impl AudioSource {
    pub fn new(clip_id: u32) -> Self {
        AudioSource {
            clip_id,
            volume: 1.0,
            spatial: false,
            looping: false,
            playing: false,
            played_once: false,
        }
    }

    pub fn spatial(mut self) -> Self { self.spatial = true; self }
    pub fn looping(mut self) -> Self { self.looping = true; self }
    pub fn play(&mut self) { self.playing = true; self.played_once = false; }
    pub fn stop(&mut self) { self.playing = false; }
}
```

### audio_sync_system

```rust
use voltex_ecs::world::World;
use voltex_ecs::transform::Transform;

pub fn audio_sync_system(world: &mut World, audio: &mut AudioSystem) {
    for (entity, transform, source) in world.query2::<Transform, AudioSource>() {
        if source.playing && !source.played_once {
            if source.spatial {
                audio.play_3d(source.clip_id, transform.position, source.volume);
            } else {
                audio.play(source.clip_id, source.volume);
            }
            // Mark as started (don't re-trigger every frame)
            // Need mutable access — use get_mut after query
        }
    }
    // Separate pass for mutation (borrow rules)
    let entities: Vec<_> = world.query::<AudioSource>()
        .filter(|(_, s)| s.playing && !s.played_once)
        .map(|(e, _)| e)
        .collect();
    for entity in entities {
        if let Some(source) = world.get_mut::<AudioSource>(entity) {
            source.played_once = true;
        }
    }
}
```

## NavAgent

`crates/voltex_ai/src/nav_agent.rs`

```rust
use voltex_math::Vec3;

#[derive(Debug, Clone)]
pub struct NavAgent {
    pub target: Option<Vec3>,
    pub speed: f32,
    pub path: Vec<Vec3>,
    pub current_waypoint: usize,
    pub reached: bool,
}

impl NavAgent {
    pub fn new(speed: f32) -> Self {
        NavAgent {
            target: None,
            speed,
            path: Vec::new(),
            current_waypoint: 0,
            reached: false,
        }
    }

    pub fn set_target(&mut self, target: Vec3) {
        self.target = Some(target);
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
    }

    pub fn clear_target(&mut self) {
        self.target = None;
        self.path.clear();
        self.current_waypoint = 0;
        self.reached = false;
    }
}
```

### nav_agent_system

```rust
use voltex_ecs::world::World;
use voltex_ecs::transform::Transform;

pub fn nav_agent_system(world: &mut World, navmesh: &NavMesh, dt: f32) {
    // Collect entities that need path finding
    let needs_path: Vec<_> = world.query2::<Transform, NavAgent>()
        .filter(|(_, _, agent)| agent.target.is_some() && agent.path.is_empty() && !agent.reached)
        .map(|(e, t, a)| (e, t.position, a.target.unwrap()))
        .collect();

    // Find paths
    for (entity, start, goal) in needs_path {
        if let Some(path) = navmesh.find_path(start, goal) {
            if let Some(agent) = world.get_mut::<NavAgent>(entity) {
                agent.path = path;
                agent.current_waypoint = 0;
            }
        }
    }

    // Move agents along paths
    let moving: Vec<_> = world.query2::<Transform, NavAgent>()
        .filter(|(_, _, a)| !a.path.is_empty() && !a.reached)
        .map(|(e, t, a)| (e, t.position, a.speed, a.path[a.current_waypoint], a.current_waypoint, a.path.len()))
        .collect();

    for (entity, pos, speed, waypoint, wp_idx, path_len) in moving {
        let dir = waypoint - pos;
        let dist = dir.length();
        let arrival_dist = 0.3;

        if dist < arrival_dist {
            // Reached waypoint
            if let Some(agent) = world.get_mut::<NavAgent>(entity) {
                agent.current_waypoint += 1;
                if agent.current_waypoint >= path_len {
                    agent.reached = true;
                }
            }
        } else {
            // Move toward waypoint
            let move_dir = dir.normalize();
            let move_dist = (speed * dt).min(dist);
            if let Some(transform) = world.get_mut::<Transform>(entity) {
                transform.position = pos + move_dir * move_dist;
            }
        }
    }
}
```

## Dependencies

`voltex_audio/Cargo.toml` 추가:
```toml
voltex_ecs.workspace = true
voltex_math.workspace = true  # (이미 있을 수 있음)
```

`voltex_ai/Cargo.toml` 추가:
```toml
voltex_ecs.workspace = true
voltex_math.workspace = true  # (이미 있을 수 있음)
```

## Testing

### AudioSource
- new(): 기본값 검증
- play/stop: playing 상태 변경
- spatial/looping 빌더 패턴

### NavAgent
- new(): 기본값 검증
- set_target: target 설정, path 초기화
- clear_target: 모든 상태 초기화

### audio_sync_system (ECS 통합)
- World에 Transform + AudioSource 엔티티 → system 호출 → played_once 변경 확인

### nav_agent_system (ECS 통합)
- World에 Transform + NavAgent 엔티티 → target 설정 → system 호출 → position 변경 확인
- 간단한 NavMesh 필요 (직선 경로)
