# Voltex Survivor — Top-Down Survival Game Spec

## Overview

Voltex 엔진의 기능을 종합적으로 활용하는 쿼터뷰 3D 서바이벌 게임. 아레나에서 웨이브별로 몰려오는 적을 처치.

## Genre / Camera

쿼터뷰 탑다운 서바이벌. 싱글플레이.

## Engine Features Used

- voltex_renderer: PBR, directional light, shadow
- voltex_ecs: Entity, World, Transform, query
- voltex_physics: Collider, detect_collisions, raycast
- voltex_ai: NavMesh, A* pathfinding, steering (seek, arrive)
- voltex_audio: WAV playback, 3D spatial audio, mixer
- voltex_math: Vec3, Mat4
- voltex_platform: Window, InputState, GameTimer
- voltex_editor: IMGUI HUD (HP, wave, score)

## Game Structure

```
examples/survivor_game/
├── Cargo.toml
└── src/
    ├── main.rs          — entry point, window, game loop
    ├── game.rs          — GameState, ECS World, tick logic
    ├── player.rs        — Player component, movement, shooting
    ├── enemy.rs         — Enemy component, spawn, AI chase
    ├── projectile.rs    — Projectile component, movement, collision
    ├── arena.rs         — Arena mesh + NavMesh generation
    ├── camera.rs        — Quarter-view camera following player
    ├── wave.rs          — Wave system (enemy count escalation)
    └── hud.rs           — IMGUI HUD overlay
```

## Gameplay

### Arena
- 20x20 unit flat floor (Y=0 plane)
- 4~6 obstacle boxes randomly placed
- NavMesh: floor triangulated excluding obstacles

### Player
- Visual: blue sphere (radius 0.5)
- Movement: WASD on XZ plane, speed 8 units/sec
- Aim: mouse position projected onto Y=0 plane (raycast from camera)
- Shoot: left click, fires projectile toward aim direction
- Fire rate: 0.2s cooldown
- HP: 3, contact with enemy = -1 HP, invincibility 1s after hit
- Collider: Sphere { radius: 0.5 }

### Enemy
- Visual: red sphere (radius 0.4)
- Spawn: at arena edges, random position
- AI: find_path on NavMesh from enemy to player, then seek steering
- Speed: 3 units/sec (slower than player)
- Contact with player: deal damage, enemy destroyed
- Collider: Sphere { radius: 0.4 }

### Projectile
- Visual: small yellow sphere (radius 0.15)
- Movement: straight line, speed 20 units/sec
- Lifetime: 2 seconds, then despawn
- Collision: hit enemy → destroy both projectile and enemy, +100 score
- Collider: Sphere { radius: 0.15 }

### Wave System
- Wave 1: 3 enemies, 5s delay
- Wave N: 2+N enemies, 5s between waves
- After all enemies in wave killed → next wave starts after delay
- No max wave (infinite escalation)

### Camera
- Quarter-view: player_pos + Vec3(0, 15, 10)
- look_at(player_pos)
- Fixed angle, follows player

### HUD (IMGUI)
- Top-left: HP hearts or "HP: 3/3"
- Top-right: "Wave: 5" + "Score: 1200"
- Game over: "GAME OVER - Score: XXXX" centered
- Press R to restart

### Audio
- Shoot SFX: short sine burst (generated procedurally)
- Enemy death SFX: lower pitch burst
- BGM: optional, simple looping tone
- 3D spatial: enemy death sounds at enemy position

## Rendering

Forward PBR pipeline (existing). No deferred needed for simple scene.

- Floor: large flat box, gray PBR material (metallic=0, roughness=0.8)
- Obstacles: dark boxes, PBR material
- Player: blue sphere (base_color blue, metallic=0.3, roughness=0.5)
- Enemies: red sphere (base_color red)
- Projectiles: yellow sphere (base_color yellow, emissive-like bright)
- Light: 1 directional light (sun), shadow enabled
- Clear color: dark blue sky

## Controls

| Input | Action |
|-------|--------|
| WASD | Move player |
| Mouse move | Aim direction |
| Left click | Shoot |
| R | Restart (after game over) |
| ESC | Quit |

## Components (ECS)

```rust
struct Player { hp: i32, fire_cooldown: f32, invincible_timer: f32 }
struct Enemy { speed: f32 }
struct Projectile { velocity: Vec3, lifetime: f32 }
struct Health { current: i32, max: i32 }
```

All entities also have Transform + Collider.

## Game Loop (per frame)

1. Process input (WASD → player velocity, mouse → aim)
2. Update player position
3. Update enemy AI (pathfinding + steering)
4. Update projectile positions
5. Run collision detection
6. Process collisions (projectile↔enemy, enemy↔player)
7. Update wave system
8. Update camera
9. Render scene (PBR)
10. Render HUD (IMGUI overlay)
11. Present

## Out of Scope

- Multiplayer
- Power-ups / items
- Different enemy types
- Particle effects
- Level progression / save
