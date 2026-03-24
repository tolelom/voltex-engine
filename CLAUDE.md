# Voltex Engine

Rust 게임 엔진 프로젝트. wgpu 28.0 + winit 0.30 기반.

## 작업 현황
- `docs/STATUS.md` — 완료된 Phase, crate 구조, 테스트 현황, 다음 작업
- `docs/DEFERRED.md` — 간소화/미뤄진 항목 목록

## 스펙
- `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` — 전체 엔진 설계

## 구현 계획
- `docs/superpowers/plans/` — 각 Phase별 상세 구현 계획

## 빌드/테스트
```bash
cargo build --workspace
cargo test --workspace
```

## 규칙
- Cargo path: `export PATH="$HOME/.cargo/bin:$PATH"` (Windows bash)
- wgpu 28.0 API: `immediate_size`, `multiview_mask`, `TexelCopyTextureInfo` 등 28.0 전용 필드 사용
- WGSL vec3 alignment: Rust struct에서 vec3 뒤에 padding 필요 (16바이트 정렬)
- max_bind_groups=4 (group 0~3). 리소스를 합쳐서 4개 이내로 유지
- Dynamic UBO 패턴: per-entity uniform은 aligned staging buffer + dynamic offset 사용
- 기존 예제(triangle, model_viewer 등)는 mesh_shader.wgsl(Blinn-Phong) 사용 — PBR 변경에 영향 없음
