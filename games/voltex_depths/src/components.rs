//! 게임 컴포넌트/리소스/이벤트. 씬 직렬화 안 하므로 Component trait 불필요.
// NOTE: 아래 개별 #[allow(dead_code)]는 main이 아직 시스템을 연결하지 않아 임시.
// 후속 태스크(게임 루프)에서 호출자가 생기는 대로 제거.
use voltex::prelude::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Title,
    Playing,
    GameOver,
    Victory,
}

#[allow(dead_code)]
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

#[allow(dead_code)] // 필드(.0) 읽기는 이동 시스템에서 생김
#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub Vec3);

#[allow(dead_code)] // hp 읽기는 데미지/HUD 시스템에서 생김
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hp: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Projectile {
    pub team: Team,
    pub damage: i32,
}

#[allow(dead_code)] // 필드 읽기는 적 AI 시스템에서 생김
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
        // touch Timer를 "이미 만료" 상태로 초기화: 첫 접촉에서 즉시 데미지.
        // Repeating의 finished() == just_finished() 이므로 한 주기 tick 으로 만든다.
        let mut touch = Timer::repeating(0.8);
        touch.tick(0.8);
        Self {
            kind,
            region,
            shoot,
            touch,
            boss_dash_next: true,
        }
    }
}

#[allow(dead_code)] // 필드 읽기는 door_system에서 생김
pub struct Door {
    pub gate_region: usize,
    pub cell: (usize, usize),
}

/// 이벤트: 적 사망 (door_system이 RegionCounts 갱신에 사용)
#[allow(dead_code)]
pub struct EnemyKilled {
    pub region: usize,
}

// ---- World 리소스 (world.clear() 시 함께 소멸 — start_game이 재삽입) ----
#[allow(dead_code)] // 필드 읽기는 HUD/door_system에서 생김
pub struct Score(pub u32);
#[allow(dead_code)]
pub struct RegionCounts(pub Vec<u32>);
