# CLI 번들 E2E 메시 레그 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.102/106 (texture/audio E2E). 3종 에셋 E2E 커버리지 완결.

## 설계

`e2e_texture_bundle.rs` 에 메시 레그 추가:

- 씬에 **두 번째 엔티티** (transform + `mesh_path("models/tri.obj")`) —
  기존 엔티티는 `primitive_mesh` 가 materialize 에서 MeshHandle 을 먼저
  부착해 mesh_path resolve 가 멱등 skip 되므로 분리 필수.
- `models/tri.obj` (선언 `assets/` 밖) — 최소 삼각형 OBJ 텍스트
  (`Mesh::from_obj` 가 디코드).
- 단언 추가: 자동 동봉 로그 / `.vpkg` 키 / 런 로그 `1 mesh path(s)`
  (texture 와 같은 프레임이면 `resolved 1 texture path(s), 1 mesh path(s)`
  한 줄 — substring 단언이라 분리/병합 무관).

## 검증

- E2E 1회 완주 (RTX 4080). clippy/test 그린.

## 비범위

- glTF/GLB 레그 (디코더 동일 경로 — OBJ 로 충분), 픽셀 검증.
