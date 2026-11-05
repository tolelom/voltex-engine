# voltex_math

## 책임 (한 줄)
선형대수 원시 타입 — Vec2/3/4, Mat4, Quat, AABB, Ray + RNG + 이징 + Color.

## 공개 API 진입점
- `Vec2`, `Vec3`, `Vec4` — 2D/3D/4D 벡터 (src/vec2.rs, src/vec3.rs, src/vec4.rs). `Vec3::min`/`max`/`mul_elementwise`는 성분별 연산
- `Quat` — 단위 쿼터니언 회전. `IDENTITY`(`identity()`도 같음)/`from_axis_angle`/`from_euler_yxz`/`to_euler_yxz`/`to_mat4`/`slerp`/`integrate_angular_velocity`(옛 이름 `integrate`, deprecated)/`from_array`·`to_array`(`[x, y, z, w]`) (src/quat.rs)
- `Mat4` — 4x4 변환 행렬, transform/lookAt/perspective/orthographic/inverse (src/mat4.rs)
- `AABB` — 축 정렬 바운딩 박스 (src/aabb.rs)
- `Ray` — 광선 (origin + direction) (src/ray.rs)
- `Color` — RGBA f32. `rgb`/`rgba`/`from_hex(0xRRGGBB)`/`from_hex_rgba(0xRRGGBBAA)`(옛 이름 `hex`/`hexa`, deprecated)/`from_hsv`/`from_array`/`to_array`/`with_alpha`/`lerp` + 명명 상수(WHITE/BLACK/RED/…/GRAY/TRANSPARENT) + `From<[f32;4]>`/`Into<[f32;4]>`(renderer 색 경로와 무변경 연결) (src/color.rs)
- `easing` 모듈 — `lerp(a,b,t)` + `linear`/`ease_in/out/in_out_quad`/`_cubic`/`smoothstep`/`ease_out_back` 이징 곡선(t∈[0,1] clamp, 끝점 0/1 고정) (src/easing.rs). `Vec2::lerp`/`Vec3::lerp` 메서드도 추가. Timer.fraction()과 결합해 tween
- `Rng` — SplitMix64 결정론적 PRNG. `seeded`/`next_u64`/`next_u32`/`next_f32`/`range_f32`/`range_i32`/`chance`/`unit_vec2`/`unit_vec3` (src/rng.rs). 외부 dep 0, 시드 동일→시퀀스 동일

## 모듈 구조
- `vec2`, `vec3`, `vec4` — 벡터 (src/vec2.rs, vec3.rs, vec4.rs)
- `mat4` — 4x4 행렬 + 변환 함수 (src/mat4.rs)
- `quat` — Quat (src/quat.rs)
- `aabb` — AABB (src/aabb.rs)
- `ray` — Ray (src/ray.rs)
- `rng` — Rng 결정론적 PRNG (src/rng.rs)
- `easing` — 이징 곡선 + lerp (src/easing.rs)
- `color` — Color RGBA (hex/hsv/lerp/명명상수) (src/color.rs)

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
