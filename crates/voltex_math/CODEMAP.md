# voltex_math

## 책임 (한 줄)
선형대수 원시 타입 — Vec2/3/4, Mat4, AABB, Ray.

## 공개 API 진입점
- `Vec2`, `Vec3`, `Vec4` — 2D/3D/4D 벡터 (src/vec2.rs, src/vec3.rs, src/vec4.rs)
- `Mat4` — 4x4 변환 행렬, transform/lookAt/perspective/orthographic/inverse (src/mat4.rs)
- `AABB` — 축 정렬 바운딩 박스 (src/aabb.rs)
- `Ray` — 광선 (origin + direction) (src/ray.rs)

## 모듈 구조
- `vec2`, `vec3`, `vec4` — 벡터 (src/vec2.rs, vec3.rs, vec4.rs)
- `mat4` — 4x4 행렬 + 변환 함수 (src/mat4.rs)
- `aabb` — AABB (src/aabb.rs)
- `ray` — Ray (src/ray.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std만 사용)

## 변경 시 주의점
- 거의 모든 crate가 의존 (ai, audio, ecs, editor, physics, platform, renderer, script)
- 시그니처/메모리 레이아웃 변경 시 GPU 셰이더(WGSL)와 정렬 깨질 수 있음 (vec3는 16바이트 padding 필요)
- Mat4 row-major vs column-major 선택은 셰이더와 일치해야 함
- `Mat4::inverse`는 3x3 minor 기반 표준 cofactor 전개. 이전의 2x2 minor 단축 구현은 행 0+1 또는 2+3 안에 들지 않는 3x3 minor 엔트리에서 잘못된 결과를 냈음 (8-7에서 수정). 변경 시 `test_inverse_composes_to_identity` 회귀 테스트 유지

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_math --lib`
- 현재 테스트 개수: 39개 (8-7 +2: inverse roundtrip × VP+translation)
