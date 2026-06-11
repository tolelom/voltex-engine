// Task 2에서 EnemyKind만 정의. 후속 태스크에서 확장 예정.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    Chaser,
    Shooter,
    Boss,
}
