//! 게임 컴포넌트/리소스/이벤트. 씬 직렬화 안 하므로 Component trait 불필요.
use voltex::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Title,
    Playing,
    GameOver,
    Victory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    Chaser,
    Shooter,
    Boss,
}

#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hp: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Projectile {
    pub team: Team,
    pub damage: i32,
}

pub struct Enemy {
    pub kind: EnemyKind,
    pub region: usize,
    pub shoot: Timer,    // 사수 발사 주기 / 보스 패턴 주기
    pub touch: Timer,    // 접촉 데미지 쿨다운
    pub boss_dash_next: bool,
}

impl Enemy {
    pub fn new(kind: EnemyKind, region: usize) -> Self {
        let shoot = match kind {
            EnemyKind::Shooter => Timer::repeating(1.4),
            EnemyKind::Boss => Timer::repeating(2.5),
            EnemyKind::Chaser => Timer::repeating(1_000_000.0),
        };
        Self {
            kind,
            region,
            shoot,
            touch: Timer::repeating(0.8),
            boss_dash_next: true,
        }
    }
}

pub struct Door {
    pub gate_region: usize,
    pub cell: (usize, usize),
}

/// 이벤트: 적 사망 (door_system이 RegionCounts 갱신에 사용)
pub struct EnemyKilled {
    pub region: usize,
}

// ---- World 리소스 (world.clear() 시 함께 소멸 — start_game이 재삽입) ----
pub struct Score(pub u32);
pub struct RegionCounts(pub Vec<u32>);
