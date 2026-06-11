//! Voltex Depths 게임 로직 라이브러리. bin(main.rs)과 통합 테스트(tests/)가 공유.
//!
//! - [`combat`] — 조준 unprojection, 투사체, 접촉 데미지 매트릭스, 사망 처리
//! - [`components`] — 게임 컴포넌트/리소스/이벤트 (Enemy, Health, Score, EnemyKilled 등)
//! - [`enemy`] — 적 AI: region 활성화, 추적/사수/보스 패턴, 속도 적분
//! - [`game_state`] — 문 개방, 게임 시작/재시작, HUD 그리기
//! - [`level`] — ASCII 맵 파싱, region/NavMesh 빌드, 레벨 시딩, 축 분리 이동
pub mod combat;
pub mod components;
pub mod enemy;
pub mod game_state;
pub mod level;
