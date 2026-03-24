# Phase 2: Rendering Basics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** OBJ 모델을 로딩하고, BMP 텍스처를 입히고, Blinn-Phong 라이팅으로 렌더링하고, FPS 카메라로 돌려볼 수 있다.

**Architecture:** voltex_math에 Vec2, Vec4, Mat4를 추가하고, voltex_renderer에 Mesh/OBJ 파서/카메라/텍스처/라이팅을 구현한다. Uniform 버퍼로 카메라 행렬과 라이트 데이터를 셰이더에 전달하고, Blinn-Phong WGSL 셰이더로 라이팅을 처리한다. 새 `examples/model_viewer` 앱이 모든 것을 통합한다.

**Tech Stack:** Rust 1.94, wgpu 28.0, winit 0.30, bytemuck 1.x, pollster 0.4

**Spec:** `docs/superpowers/specs/2026-03-24-voltex-engine-design.md` Phase 2 섹션

**변경 사항 (스펙 대비):** PNG/JPG 디코더는 별도 Phase로 분리. Phase 2에서는 BMP 로더만 자체 구현한다.

---

## File Structure

```
crates/
├── voltex_math/src/
│   ├── lib.rs              # 모듈 re-export 업데이트
│   ├── vec2.rs             # Vec2 구현 (NEW)
│   ├── vec3.rs             # 기존 유지
│   ├── vec4.rs             # Vec4 구현 (NEW)
│   └── mat4.rs             # Mat4 구현 (NEW)
├── voltex_renderer/src/
│   ├── lib.rs              # 모듈 re-export 업데이트
│   ├── gpu.rs              # depth texture 생성 추가 (MODIFY)
│   ├── vertex.rs           # MeshVertex 추가 (MODIFY)
│   ├── pipeline.rs         # mesh pipeline 추가 (MODIFY)
│   ├── shader.wgsl         # 기존 유지 (triangle용)
│   ├── mesh.rs             # Mesh 구조체 + GPU 업로드 (NEW)
│   ├── obj.rs              # OBJ 파서 (NEW)
│   ├── camera.rs           # Camera + FpsController (NEW)
│   ├── texture.rs          # BMP 로더 + GPU 텍스처 (NEW)
│   ├── light.rs            # DirectionalLight + uniform (NEW)
│   └── mesh_shader.wgsl    # Blinn-Phong 셰이더 (NEW)
examples/
├── triangle/               # 기존 유지
└── model_viewer/           # 모델 뷰어 데모 (NEW)
    ├── Cargo.toml
    └── src/
        └── main.rs
assets/                     # 테스트용 에셋 (NEW)
└── cube.obj                # 기본 큐브 모델
```

---

## Task 1: voltex_math — Vec2, Vec4

**Files:**
- Create: `crates/voltex_math/src/vec2.rs`
- Create: `crates/voltex_math/src/vec4.rs`
- Modify: `crates/voltex_math/src/lib.rs`

Vec2는 UV 좌표에 필요하고, Vec4는 동차 좌표(homogeneous coordinates)와 색상에 필요하다.

- [ ] **Step 1: vec2.rs 테스트 + 구현 작성**

```rust
// crates/voltex_math/src/vec2.rs
use std::ops::{Add, Sub, Mul, Neg};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        Self { x: self.x / len, y: self.y / len }
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self { x: self.x * rhs, y: self.y * rhs }
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self { x: -self.x, y: -self.y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
    }

    #[test]
    fn test_add() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn test_dot() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a.dot(b), 11.0);
    }

    #[test]
    fn test_length() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.length() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize() {
        let v = Vec2::new(4.0, 0.0);
        assert_eq!(v.normalize(), Vec2::new(1.0, 0.0));
    }
}
```

- [ ] **Step 2: vec4.rs 테스트 + 구현 작성**

```rust
// crates/voltex_math/src/vec4.rs
use std::ops::{Add, Sub, Mul, Neg};
use crate::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0, z: 1.0, w: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn from_vec3(v: Vec3, w: f32) -> Self {
        Self { x: v.x, y: v.y, z: v.z, w }
    }

    pub fn xyz(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }
}

impl Add for Vec4 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z, w: self.w + rhs.w }
    }
}

impl Sub for Vec4 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z, w: self.w - rhs.w }
    }
}

impl Mul<f32> for Vec4 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs, w: self.w * rhs }
    }
}

impl Neg for Vec4 {
    type Output = Self;
    fn neg(self) -> Self {
        Self { x: -self.x, y: -self.y, z: -self.z, w: -self.w }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.w, 4.0);
    }

    #[test]
    fn test_from_vec3() {
        let v3 = Vec3::new(1.0, 2.0, 3.0);
        let v4 = Vec4::from_vec3(v3, 1.0);
        assert_eq!(v4, Vec4::new(1.0, 2.0, 3.0, 1.0));
    }

    #[test]
    fn test_xyz() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v.xyz(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_dot() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a.dot(b), 70.0); // 5+12+21+32
    }

    #[test]
    fn test_add() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a + b, Vec4::new(6.0, 8.0, 10.0, 12.0));
    }
}
```

- [ ] **Step 3: lib.rs 업데이트**

```rust
// crates/voltex_math/src/lib.rs
pub mod vec2;
pub mod vec3;
pub mod vec4;

pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p voltex_math`
Expected: 모든 테스트 PASS (기존 10개 + Vec2 5개 + Vec4 5개 = 20개)

- [ ] **Step 5: 커밋**

```bash
git add crates/voltex_math/
git commit -m "feat(math): add Vec2 and Vec4 types"
```

---

## Task 2: voltex_math — Mat4

**Files:**
- Create: `crates/voltex_math/src/mat4.rs`
- Modify: `crates/voltex_math/src/lib.rs`

4x4 행렬. 카메라 View/Projection 행렬, 모델 변환에 필요하다. Column-major 저장 (wgpu/WGSL 표준).

- [ ] **Step 1: mat4.rs 테스트 + 구현 작성**

```rust
// crates/voltex_math/src/mat4.rs
use crate::{Vec3, Vec4};

/// 4x4 행렬 (column-major). wgpu/WGSL과 동일한 메모리 레이아웃.
/// cols[0] = 첫 번째 열, cols[1] = 두 번째 열, ...
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub const fn from_cols(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], c3: [f32; 4]) -> Self {
        Self { cols: [c0, c1, c2, c3] }
    }

    /// GPU에 전달할 수 있는 f32 배열 반환
    pub fn as_slice(&self) -> &[f32; 16] {
        // Safety: [f32; 4] x 4 == [f32; 16] in memory
        unsafe { &*(self.cols.as_ptr() as *const [f32; 16]) }
    }

    /// 행렬 곱셈 (self * rhs)
    pub fn mul_mat4(&self, rhs: &Mat4) -> Mat4 {
        let mut result = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                result[c][r] = self.cols[0][r] * rhs.cols[c][0]
                    + self.cols[1][r] * rhs.cols[c][1]
                    + self.cols[2][r] * rhs.cols[c][2]
                    + self.cols[3][r] * rhs.cols[c][3];
            }
        }
        Mat4 { cols: result }
    }

    /// 4x4 행렬 * Vec4
    pub fn mul_vec4(&self, v: Vec4) -> Vec4 {
        Vec4::new(
            self.cols[0][0] * v.x + self.cols[1][0] * v.y + self.cols[2][0] * v.z + self.cols[3][0] * v.w,
            self.cols[0][1] * v.x + self.cols[1][1] * v.y + self.cols[2][1] * v.z + self.cols[3][1] * v.w,
            self.cols[0][2] * v.x + self.cols[1][2] * v.y + self.cols[2][2] * v.z + self.cols[3][2] * v.w,
            self.cols[0][3] * v.x + self.cols[1][3] * v.y + self.cols[2][3] * v.z + self.cols[3][3] * v.w,
        )
    }

    /// 이동 행렬
    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        Self::from_cols(
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [x, y, z, 1.0],
        )
    }

    /// 균등 스케일 행렬
    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        Self::from_cols(
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, sz, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }

    /// X축 회전 (라디안)
    pub fn rotation_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::from_cols(
            [1.0, 0.0, 0.0, 0.0],
            [0.0, c, s, 0.0],
            [0.0, -s, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }

    /// Y축 회전 (라디안)
    pub fn rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::from_cols(
            [c, 0.0, -s, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [s, 0.0, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }

    /// Z축 회전 (라디안)
    pub fn rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::from_cols(
            [c, s, 0.0, 0.0],
            [-s, c, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }

    /// Look-at 뷰 행렬 (오른손 좌표계)
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize(); // forward
        let r = f.cross(up).normalize();    // right
        let u = r.cross(f);                 // true up

        Self::from_cols(
            [r.x, u.x, -f.x, 0.0],
            [r.y, u.y, -f.y, 0.0],
            [r.z, u.z, -f.z, 0.0],
            [-r.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
        )
    }

    /// 원근 투영 행렬 (fov_y: 라디안, aspect: width/height)
    /// wgpu NDC: x,y [-1,1], z [0,1] (왼손 depth)
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y / 2.0).tan();
        let range_inv = 1.0 / (near - far);
        Self::from_cols(
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far * range_inv, -1.0],
            [0.0, 0.0, near * far * range_inv, 0.0],
        )
    }

    /// 전치 행렬
    pub fn transpose(&self) -> Self {
        let c = &self.cols;
        Self::from_cols(
            [c[0][0], c[1][0], c[2][0], c[3][0]],
            [c[0][1], c[1][1], c[2][1], c[3][1]],
            [c[0][2], c[1][2], c[2][2], c[3][2]],
            [c[0][3], c[1][3], c[2][3], c[3][3]],
        )
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, rhs: Mat4) -> Mat4 {
        self.mul_mat4(&rhs)
    }
}

impl std::ops::Mul<Vec4> for Mat4 {
    type Output = Vec4;
    fn mul(self, rhs: Vec4) -> Vec4 {
        self.mul_vec4(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn mat4_approx_eq(a: &Mat4, b: &Mat4) -> bool {
        a.cols.iter().zip(b.cols.iter())
            .all(|(ca, cb)| ca.iter().zip(cb.iter()).all(|(x, y)| approx_eq(*x, *y)))
    }

    #[test]
    fn test_identity_mul() {
        let m = Mat4::translation(1.0, 2.0, 3.0);
        let result = Mat4::IDENTITY * m;
        assert!(mat4_approx_eq(&result, &m));
    }

    #[test]
    fn test_translation_mul_vec4() {
        let m = Mat4::translation(10.0, 20.0, 30.0);
        let v = Vec4::new(1.0, 2.0, 3.0, 1.0);
        let result = m * v;
        assert!(approx_eq(result.x, 11.0));
        assert!(approx_eq(result.y, 22.0));
        assert!(approx_eq(result.z, 33.0));
        assert!(approx_eq(result.w, 1.0));
    }

    #[test]
    fn test_scale() {
        let m = Mat4::scale(2.0, 3.0, 4.0);
        let v = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let result = m * v;
        assert!(approx_eq(result.x, 2.0));
        assert!(approx_eq(result.y, 3.0));
        assert!(approx_eq(result.z, 4.0));
    }

    #[test]
    fn test_rotation_y_90() {
        let m = Mat4::rotation_y(std::f32::consts::FRAC_PI_2);
        let v = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let result = m * v;
        assert!(approx_eq(result.x, 0.0));
        assert!(approx_eq(result.z, -1.0)); // 오른손 좌표계: +X -> -Z
    }

    #[test]
    fn test_look_at_origin() {
        let eye = Vec3::new(0.0, 0.0, 5.0);
        let target = Vec3::ZERO;
        let up = Vec3::Y;
        let view = Mat4::look_at(eye, target, up);
        // eye에서 원점을 바라보면, 원점은 카메라 앞(−Z)에 있어야 함
        let p = view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(approx_eq(p.x, 0.0));
        assert!(approx_eq(p.y, 0.0));
        assert!(approx_eq(p.z, -5.0)); // 카메라 뒤쪽으로 5
    }

    #[test]
    fn test_perspective_near_plane() {
        let proj = Mat4::perspective(
            std::f32::consts::FRAC_PI_4,
            1.0,
            0.1,
            100.0,
        );
        // near plane 위의 점은 z=0으로 매핑되어야 함
        let p = proj * Vec4::new(0.0, 0.0, -0.1, 1.0);
        let ndc_z = p.z / p.w;
        assert!(approx_eq(ndc_z, 0.0));
    }

    #[test]
    fn test_transpose() {
        let m = Mat4::from_cols(
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        );
        let t = m.transpose();
        assert_eq!(t.cols[0], [1.0, 5.0, 9.0, 13.0]);
        assert_eq!(t.cols[1], [2.0, 6.0, 10.0, 14.0]);
    }

    #[test]
    fn test_as_slice() {
        let m = Mat4::IDENTITY;
        let s = m.as_slice();
        assert_eq!(s[0], 1.0);  // col0[0]
        assert_eq!(s[5], 1.0);  // col1[1]
        assert_eq!(s[10], 1.0); // col2[2]
        assert_eq!(s[15], 1.0); // col3[3]
    }
}
```

- [ ] **Step 2: lib.rs 업데이트**

```rust
// crates/voltex_math/src/lib.rs
pub mod vec2;
pub mod vec3;
pub mod vec4;
pub mod mat4;

pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;
pub use mat4::Mat4;
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_math`
Expected: 모든 테스트 PASS (20 + 8 = 28개)

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_math/
git commit -m "feat(math): add Mat4 with transforms, look_at, perspective"
```

---

## Task 3: voltex_renderer — MeshVertex + Mesh + depth buffer

**Files:**
- Modify: `crates/voltex_renderer/src/vertex.rs`
- Create: `crates/voltex_renderer/src/mesh.rs`
- Modify: `crates/voltex_renderer/src/gpu.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

기존 `Vertex`(position+color)는 유지하고, 3D 렌더링용 `MeshVertex`(position+normal+uv)를 추가한다. `Mesh` 구조체로 GPU 버퍼를 관리한다. GpuContext에 depth texture를 추가한다.

- [ ] **Step 1: vertex.rs에 MeshVertex 추가**

```rust
// crates/voltex_renderer/src/vertex.rs — 기존 Vertex 아래에 추가

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl MeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            // position
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // normal
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            // uv
            wgpu::VertexAttribute {
                offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}
```

- [ ] **Step 2: mesh.rs 작성**

```rust
// crates/voltex_renderer/src/mesh.rs
use crate::vertex::MeshVertex;
use wgpu::util::DeviceExt;

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl Mesh {
    pub fn new(device: &wgpu::Device, vertices: &[MeshVertex], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }
}
```

- [ ] **Step 3: gpu.rs에 depth texture 추가**

`GpuContext`에 `depth_view` 필드를 추가하고, `create_depth_texture` 헬퍼를 만든다. `resize`에서도 depth texture를 재생성한다.

```rust
// gpu.rs 수정 — 구조체에 필드 추가:
pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
    pub depth_view: wgpu::TextureView,
}

// 헬퍼 함수 추가 (impl GpuContext 밖이나 안):
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

// new_async에서 depth_view 생성:
// let depth_view = create_depth_texture(&device, config.width, config.height);
// Self { ..., depth_view }

// resize에서 depth_view 재생성:
// self.depth_view = create_depth_texture(&self.device, width, height);
```

- [ ] **Step 4: lib.rs 업데이트**

```rust
// crates/voltex_renderer/src/lib.rs
pub mod gpu;
pub mod pipeline;
pub mod vertex;
pub mod mesh;

pub use gpu::{GpuContext, DEPTH_FORMAT};
pub use mesh::Mesh;
```

- [ ] **Step 5: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 빌드 성공

참고: 기존 `examples/triangle`은 `depth_stencil_attachment: None`을 사용하므로 depth 관련 변경에 영향 없음. 단, `GpuContext::new`의 반환값에 `depth_view`가 추가되었으므로 triangle 예제도 자동으로 이 필드를 받게 됨.

- [ ] **Step 6: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add MeshVertex, Mesh, and depth buffer support"
```

---

## Task 4: voltex_renderer — OBJ 파서

**Files:**
- Create: `crates/voltex_renderer/src/obj.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

최소한의 OBJ 파서: `v` (position), `vn` (normal), `vt` (texcoord), `f` (face) 파싱. 삼각형 면만 지원 (quad면은 삼각형 2개로 분할).

- [ ] **Step 1: obj.rs 테스트 + 구현 작성**

```rust
// crates/voltex_renderer/src/obj.rs
use crate::vertex::MeshVertex;

pub struct ObjData {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

/// OBJ 파일 텍스트를 파싱하여 MeshVertex + 인덱스 배열 반환.
/// 삼각형/쿼드 면 지원. 쿼드는 삼각형 2개로 분할.
pub fn parse_obj(source: &str) -> ObjData {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut texcoords: Vec<[f32; 2]> = Vec::new();

    let mut vertices: Vec<MeshVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // 중복 정점 제거를 위한 맵 (v/vt/vn 인덱스 조합 → 최종 인덱스)
    let mut vertex_map: std::collections::HashMap<(u32, u32, u32), u32> = std::collections::HashMap::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let prefix = match parts.next() {
            Some(p) => p,
            None => continue,
        };

        match prefix {
            "v" => {
                let x: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let y: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let z: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                positions.push([x, y, z]);
            }
            "vn" => {
                let x: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let y: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let z: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                normals.push([x, y, z]);
            }
            "vt" => {
                let u: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let v: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                texcoords.push([u, v]);
            }
            "f" => {
                let face_verts: Vec<(u32, u32, u32)> = parts
                    .map(|token| parse_face_vertex(token))
                    .collect();

                // 삼각형 fan 분할 (tri, quad, n-gon 모두 지원)
                for i in 1..face_verts.len().saturating_sub(1) {
                    for &fi in &[0, i, i + 1] {
                        let (vi, ti, ni) = face_verts[fi];
                        let key = (vi, ti, ni);
                        let idx = if let Some(&existing) = vertex_map.get(&key) {
                            existing
                        } else {
                            let pos = if vi > 0 { positions[(vi - 1) as usize] } else { [0.0; 3] };
                            let norm = if ni > 0 { normals[(ni - 1) as usize] } else { [0.0, 1.0, 0.0] };
                            let uv = if ti > 0 { texcoords[(ti - 1) as usize] } else { [0.0; 2] };
                            let new_idx = vertices.len() as u32;
                            vertices.push(MeshVertex { position: pos, normal: norm, uv });
                            vertex_map.insert(key, new_idx);
                            new_idx
                        };
                        indices.push(idx);
                    }
                }
            }
            _ => {} // 무시: mtllib, usemtl, s, o, g 등
        }
    }

    ObjData { vertices, indices }
}

/// "v/vt/vn" 또는 "v//vn" 또는 "v/vt" 또는 "v" 형식 파싱
fn parse_face_vertex(token: &str) -> (u32, u32, u32) {
    let mut parts = token.split('/');
    let v: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let vt: u32 = parts.next().unwrap_or("").parse().unwrap_or(0);
    let vn: u32 = parts.next().unwrap_or("").parse().unwrap_or(0);
    (v, vt, vn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_triangle() {
        let obj = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1
";
        let data = parse_obj(obj);
        assert_eq!(data.vertices.len(), 3);
        assert_eq!(data.indices.len(), 3);
        assert_eq!(data.vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(data.vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(data.vertices[0].normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_parse_quad_triangulated() {
        let obj = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1 4//1
";
        let data = parse_obj(obj);
        // quad → 2 triangles → 6 indices
        assert_eq!(data.indices.len(), 6);
    }

    #[test]
    fn test_parse_with_uv() {
        let obj = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vt 0.0 0.0
vt 1.0 0.0
vt 0.0 1.0
vn 0.0 0.0 1.0
f 1/1/1 2/2/1 3/3/1
";
        let data = parse_obj(obj);
        assert_eq!(data.vertices[0].uv, [0.0, 0.0]);
        assert_eq!(data.vertices[1].uv, [1.0, 0.0]);
        assert_eq!(data.vertices[2].uv, [0.0, 1.0]);
    }

    #[test]
    fn test_vertex_dedup() {
        let obj = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
f 1//1 2//1 3//1
f 1//1 3//1 2//1
";
        let data = parse_obj(obj);
        // 같은 v/vt/vn 조합은 정점 재사용
        assert_eq!(data.vertices.len(), 3);
        assert_eq!(data.indices.len(), 6);
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 추가**

```rust
// crates/voltex_renderer/src/lib.rs에 추가:
pub mod obj;
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_renderer`
Expected: OBJ 파서 테스트 4개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): implement OBJ parser with triangle/quad support"
```

---

## Task 5: voltex_renderer — Camera

**Files:**
- Create: `crates/voltex_renderer/src/camera.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

Camera는 위치/회전을 관리하고, view-projection 행렬을 계산한다. FpsController는 WASD + 마우스로 카메라를 조종한다.

- [ ] **Step 1: camera.rs 작성**

```rust
// crates/voltex_renderer/src/camera.rs
use voltex_math::{Vec3, Mat4};

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // 라디안, Y축 회전
    pub pitch: f32,  // 라디안, X축 회전
    pub fov_y: f32,  // 라디안
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(position: Vec3, aspect: f32) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_4, // 45도
            aspect,
            near: 0.1,
            far: 100.0,
        }
    }

    /// 카메라가 바라보는 방향 벡터
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// 카메라의 오른쪽 방향 벡터
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    /// 뷰 행렬
    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at(self.position, target, Vec3::Y)
    }

    /// 투영 행렬
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective(self.fov_y, self.aspect, self.near, self.far)
    }

    /// view-projection 행렬
    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }
}

/// FPS 스타일 카메라 컨트롤러
pub struct FpsController {
    pub speed: f32,
    pub mouse_sensitivity: f32,
}

impl FpsController {
    pub fn new() -> Self {
        Self {
            speed: 5.0,
            mouse_sensitivity: 0.003,
        }
    }

    /// WASD 이동. forward/right/up은 각각 W-S, D-A, Space-Shift 입력.
    pub fn process_movement(
        &self,
        camera: &mut Camera,
        forward: f32,   // +1 = W, -1 = S
        right: f32,     // +1 = D, -1 = A
        up: f32,        // +1 = Space, -1 = Shift
        dt: f32,
    ) {
        let cam_forward = camera.forward();
        let cam_right = camera.right();
        let velocity = self.speed * dt;

        camera.position = camera.position
            + cam_forward * (forward * velocity)
            + cam_right * (right * velocity)
            + Vec3::Y * (up * velocity);
    }

    /// 마우스 이동으로 카메라 회전
    pub fn process_mouse(&self, camera: &mut Camera, dx: f64, dy: f64) {
        camera.yaw += dx as f32 * self.mouse_sensitivity;
        camera.pitch -= dy as f32 * self.mouse_sensitivity;

        // pitch 제한 (-89도 ~ 89도)
        let limit = 89.0_f32.to_radians();
        camera.pitch = camera.pitch.clamp(-limit, limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_default_forward() {
        let cam = Camera::new(Vec3::ZERO, 1.0);
        let fwd = cam.forward();
        // yaw=0, pitch=0 → forward = (0, 0, -1)
        assert!((fwd.x).abs() < 1e-5);
        assert!((fwd.y).abs() < 1e-5);
        assert!((fwd.z + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_camera_yaw_90() {
        let mut cam = Camera::new(Vec3::ZERO, 1.0);
        cam.yaw = std::f32::consts::FRAC_PI_2; // 90도 → forward = (1, 0, 0)
        let fwd = cam.forward();
        assert!((fwd.x - 1.0).abs() < 1e-5);
        assert!((fwd.z).abs() < 1e-5);
    }

    #[test]
    fn test_fps_pitch_clamp() {
        let ctrl = FpsController::new();
        let mut cam = Camera::new(Vec3::ZERO, 1.0);
        // 매우 큰 마우스 이동
        ctrl.process_mouse(&mut cam, 0.0, -100000.0);
        assert!(cam.pitch <= 89.0_f32.to_radians() + 1e-5);
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 추가**

```rust
// crates/voltex_renderer/src/lib.rs에 추가:
pub mod camera;
pub use camera::{Camera, FpsController};
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_renderer`
Expected: camera 테스트 3개 + OBJ 테스트 4개 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add Camera and FpsController"
```

---

## Task 6: voltex_renderer — Light + Blinn-Phong 셰이더 + 메시 파이프라인

**Files:**
- Create: `crates/voltex_renderer/src/light.rs`
- Create: `crates/voltex_renderer/src/mesh_shader.wgsl`
- Modify: `crates/voltex_renderer/src/pipeline.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

Uniform 버퍼를 사용하여 카메라 행렬과 라이트 데이터를 셰이더에 전달한다. Blinn-Phong 셰이더로 Directional Light 라이팅을 구현한다.

- [ ] **Step 1: light.rs 작성**

```rust
// crates/voltex_renderer/src/light.rs
use bytemuck::{Pod, Zeroable};

/// GPU로 전달되는 카메라 uniform 데이터
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: [0.0; 3],
            _padding: 0.0,
        }
    }
}

/// GPU로 전달되는 Directional Light uniform 데이터
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 3],
    pub _padding1: f32,
    pub color: [f32; 3],
    pub ambient_strength: f32,
}

impl LightUniform {
    pub fn new() -> Self {
        Self {
            direction: [0.0, -1.0, -1.0], // 위에서 앞쪽으로
            _padding1: 0.0,
            color: [1.0, 1.0, 1.0],
            ambient_strength: 0.1,
        }
    }
}
```

- [ ] **Step 2: mesh_shader.wgsl 작성**

```wgsl
// crates/voltex_renderer/src/mesh_shader.wgsl

struct CameraUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
};

struct LightUniform {
    direction: vec3<f32>,
    color: vec3<f32>,
    ambient_strength: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;

// 텍스처 바인드 그룹 (group 1)
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(model_v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = camera.model * vec4<f32>(model_v.position, 1.0);
    out.world_pos = world_pos.xyz;
    // 노멀은 모델 행렬의 역전치로 변환해야 하지만, 균등 스케일이면 모델 행렬로 충분
    out.world_normal = (camera.model * vec4<f32>(model_v.normal, 0.0)).xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.uv = model_v.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);

    let normal = normalize(in.world_normal);
    let light_dir = normalize(-light.direction);

    // Ambient
    let ambient = light.ambient_strength * light.color;

    // Diffuse
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diff * light.color;

    // Specular (Blinn-Phong)
    let view_dir = normalize(camera.camera_pos - in.world_pos);
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0);
    let specular = spec * light.color * 0.5;

    let result = (ambient + diffuse + specular) * tex_color.rgb;
    return vec4<f32>(result, tex_color.a);
}
```

- [ ] **Step 3: pipeline.rs에 메시 파이프라인 추가**

```rust
// crates/voltex_renderer/src/pipeline.rs에 추가
use crate::vertex::MeshVertex;
use crate::gpu::DEPTH_FORMAT;

pub fn create_mesh_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Mesh Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mesh_shader.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Mesh Pipeline Layout"),
        bind_group_layouts: &[camera_light_layout, texture_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Mesh Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[MeshVertex::LAYOUT],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}
```

- [ ] **Step 4: lib.rs 업데이트**

```rust
// crates/voltex_renderer/src/lib.rs
pub mod gpu;
pub mod pipeline;
pub mod vertex;
pub mod mesh;
pub mod obj;
pub mod camera;
pub mod light;

pub use gpu::{GpuContext, DEPTH_FORMAT};
pub use mesh::Mesh;
pub use camera::{Camera, FpsController};
pub use light::{CameraUniform, LightUniform};
```

- [ ] **Step 5: 빌드 확인**

Run: `cargo build -p voltex_renderer`
Expected: 빌드 성공

- [ ] **Step 6: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add Blinn-Phong shader, light uniforms, mesh pipeline"
```

---

## Task 7: voltex_renderer — BMP 텍스처 로더

**Files:**
- Create: `crates/voltex_renderer/src/texture.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

비압축 24-bit/32-bit BMP 파일을 파싱하여 RGBA 픽셀 데이터를 추출하고, wgpu Texture + BindGroup으로 업로드한다.

- [ ] **Step 1: texture.rs 작성**

```rust
// crates/voltex_renderer/src/texture.rs

/// BMP 파일을 파싱하여 RGBA 픽셀 데이터 반환.
/// 비압축 24-bit (RGB) 및 32-bit (RGBA) BMP만 지원.
pub fn parse_bmp(data: &[u8]) -> Result<BmpImage, String> {
    if data.len() < 54 {
        return Err("BMP file too small".into());
    }
    if &data[0..2] != b"BM" {
        return Err("Not a BMP file".into());
    }

    let pixel_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
    let bpp = u16::from_le_bytes([data[28], data[29]]);

    let compression = u32::from_le_bytes([data[30], data[31], data[32], data[33]]);
    if compression != 0 {
        return Err(format!("Compressed BMP not supported (compression={})", compression));
    }

    if bpp != 24 && bpp != 32 {
        return Err(format!("Unsupported BMP bit depth: {}", bpp));
    }

    let w = width.unsigned_abs();
    let h = height.unsigned_abs();
    let bottom_up = height > 0;
    let bytes_per_pixel = (bpp / 8) as usize;
    let row_size = ((bpp as u32 * w + 31) / 32 * 4) as usize; // 4-byte aligned rows

    let mut pixels = vec![0u8; (w * h * 4) as usize];

    for row in 0..h {
        let src_row = if bottom_up { h - 1 - row } else { row };
        let src_offset = pixel_offset + (src_row as usize) * row_size;

        for col in 0..w {
            let src_idx = src_offset + (col as usize) * bytes_per_pixel;
            let dst_idx = ((row * w + col) * 4) as usize;

            if src_idx + bytes_per_pixel > data.len() {
                return Err("BMP pixel data truncated".into());
            }

            // BMP stores BGR(A)
            pixels[dst_idx] = data[src_idx + 2];     // R
            pixels[dst_idx + 1] = data[src_idx + 1]; // G
            pixels[dst_idx + 2] = data[src_idx];     // B
            pixels[dst_idx + 3] = if bpp == 32 { data[src_idx + 3] } else { 255 }; // A
        }
    }

    Ok(BmpImage { width: w, height: h, pixels })
}

pub struct BmpImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA
}

/// RGBA 픽셀 데이터를 wgpu 텍스처로 업로드하고, BindGroup을 반환.
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
}

impl GpuTexture {
    pub fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Diffuse Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self { texture, view, sampler, bind_group }
    }

    /// 1x1 흰색 텍스처 (텍스처 없는 메시용 기본값)
    pub fn white_1x1(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self::from_rgba(device, queue, 1, 1, &[255, 255, 255, 255], layout)
    }

    /// BindGroupLayout 정의 (group 1에서 사용)
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bmp_24bit(width: u32, height: u32, pixel_bgr: [u8; 3]) -> Vec<u8> {
        let row_size = ((24 * width + 31) / 32 * 4) as usize;
        let pixel_data_size = row_size * height as usize;
        let file_size = 54 + pixel_data_size;

        let mut data = vec![0u8; file_size];
        // Header
        data[0] = b'B'; data[1] = b'M';
        data[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        data[10..14].copy_from_slice(&54u32.to_le_bytes());
        // DIB header
        data[14..18].copy_from_slice(&40u32.to_le_bytes()); // header size
        data[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        data[22..26].copy_from_slice(&(height as i32).to_le_bytes());
        data[26..28].copy_from_slice(&1u16.to_le_bytes()); // planes
        data[28..30].copy_from_slice(&24u16.to_le_bytes()); // bpp
        // compression = 0 (already zeroed)

        // Pixel data
        for row in 0..height {
            for col in 0..width {
                let offset = 54 + (row as usize) * row_size + (col as usize) * 3;
                data[offset] = pixel_bgr[0];
                data[offset + 1] = pixel_bgr[1];
                data[offset + 2] = pixel_bgr[2];
            }
        }
        data
    }

    #[test]
    fn test_parse_bmp_24bit() {
        let bmp = make_bmp_24bit(2, 2, [255, 0, 0]); // BGR: blue
        let img = parse_bmp(&bmp).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        // BGR [255,0,0] → RGBA [0,0,255,255]
        assert_eq!(img.pixels[0], 0);   // R
        assert_eq!(img.pixels[1], 0);   // G
        assert_eq!(img.pixels[2], 255); // B
        assert_eq!(img.pixels[3], 255); // A
    }

    #[test]
    fn test_parse_bmp_not_bmp() {
        let data = vec![0u8; 100];
        assert!(parse_bmp(&data).is_err());
    }

    #[test]
    fn test_parse_bmp_too_small() {
        let data = vec![0u8; 10];
        assert!(parse_bmp(&data).is_err());
    }
}
```

- [ ] **Step 2: lib.rs에 모듈 추가**

```rust
// crates/voltex_renderer/src/lib.rs에 추가:
pub mod texture;
pub use texture::GpuTexture;
```

- [ ] **Step 3: 테스트 통과 확인**

Run: `cargo test -p voltex_renderer`
Expected: BMP 테스트 3개 + 기존 테스트 PASS

- [ ] **Step 4: 커밋**

```bash
git add crates/voltex_renderer/
git commit -m "feat(renderer): add BMP texture loader and GPU texture upload"
```

---

## Task 8: 테스트 에셋 + Model Viewer 데모

**Files:**
- Create: `assets/cube.obj`
- Create: `examples/model_viewer/Cargo.toml`
- Create: `examples/model_viewer/src/main.rs`
- Modify: `Cargo.toml` (워크스페이스에 model_viewer 추가)

모든 구현을 통합하는 model_viewer 데모. OBJ 큐브를 로드하고, Blinn-Phong 라이팅으로 렌더링하고, WASD + 마우스로 카메라를 조종한다.

- [ ] **Step 1: cube.obj 작성**

```obj
# assets/cube.obj
# Simple cube with normals and UVs

v -0.5 -0.5  0.5
v  0.5 -0.5  0.5
v  0.5  0.5  0.5
v -0.5  0.5  0.5
v -0.5 -0.5 -0.5
v  0.5 -0.5 -0.5
v  0.5  0.5 -0.5
v -0.5  0.5 -0.5

vn  0.0  0.0  1.0
vn  0.0  0.0 -1.0
vn  1.0  0.0  0.0
vn -1.0  0.0  0.0
vn  0.0  1.0  0.0
vn  0.0 -1.0  0.0

vt 0.0 0.0
vt 1.0 0.0
vt 1.0 1.0
vt 0.0 1.0

# Front face
f 1/1/1 2/2/1 3/3/1 4/4/1
# Back face
f 6/1/2 5/2/2 8/3/2 7/4/2
# Right face
f 2/1/3 6/2/3 7/3/3 3/4/3
# Left face
f 5/1/4 1/2/4 4/3/4 8/4/4
# Top face
f 4/1/5 3/2/5 7/3/5 8/4/5
# Bottom face
f 5/1/6 6/2/6 2/3/6 1/4/6
```

- [ ] **Step 2: Cargo.toml 워크스페이스 업데이트**

```toml
# Cargo.toml 루트 — members에 추가:
[workspace]
resolver = "2"
members = [
    "crates/voltex_math",
    "crates/voltex_platform",
    "crates/voltex_renderer",
    "examples/triangle",
    "examples/model_viewer",
]
```

- [ ] **Step 3: model_viewer Cargo.toml 작성**

```toml
# examples/model_viewer/Cargo.toml
[package]
name = "model_viewer"
version = "0.1.0"
edition = "2021"

[dependencies]
voltex_math.workspace = true
voltex_platform.workspace = true
voltex_renderer.workspace = true
wgpu.workspace = true
winit.workspace = true
bytemuck.workspace = true
pollster.workspace = true
env_logger.workspace = true
log.workspace = true
```

- [ ] **Step 4: model_viewer main.rs 작성**

```rust
// examples/model_viewer/src/main.rs
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};
use voltex_math::{Vec3, Mat4};
use voltex_platform::{VoltexWindow, WindowConfig, InputState, GameTimer};
use voltex_renderer::{
    GpuContext, Mesh, Camera, FpsController,
    CameraUniform, LightUniform, GpuTexture,
    pipeline, obj,
};
use wgpu::util::DeviceExt;

struct ModelViewerApp {
    state: Option<AppState>,
}

struct AppState {
    window: VoltexWindow,
    gpu: GpuContext,
    mesh_pipeline: wgpu::RenderPipeline,
    mesh: Mesh,
    camera: Camera,
    fps_controller: FpsController,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    camera_light_bind_group: wgpu::BindGroup,
    diffuse_texture: GpuTexture,
    input: InputState,
    timer: GameTimer,
    model_rotation: f32,
}

fn camera_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera+Light Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

impl ApplicationHandler for ModelViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let config = WindowConfig {
            title: "Voltex - Model Viewer".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        };
        let window = VoltexWindow::new(event_loop, &config);
        let gpu = GpuContext::new(window.handle.clone());

        // OBJ 로드
        let obj_source = include_str!("../../../assets/cube.obj");
        let obj_data = obj::parse_obj(obj_source);
        let mesh = Mesh::new(&gpu.device, &obj_data.vertices, &obj_data.indices);

        // Uniform 버퍼
        let camera_uniform = CameraUniform::new();
        let camera_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_uniform = LightUniform::new();
        let light_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layouts
        let cl_layout = camera_light_bind_group_layout(&gpu.device);
        let tex_layout = GpuTexture::bind_group_layout(&gpu.device);

        let camera_light_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera+Light Bind Group"),
            layout: &cl_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // 기본 흰색 텍스처 (BMP 파일 없으면 이걸 사용)
        let diffuse_texture = GpuTexture::white_1x1(&gpu.device, &gpu.queue, &tex_layout);

        // 파이프라인
        let mesh_pipeline = pipeline::create_mesh_pipeline(
            &gpu.device,
            gpu.surface_format,
            &cl_layout,
            &tex_layout,
        );

        let (w, h) = window.inner_size();
        let camera = Camera::new(Vec3::new(0.0, 1.0, 3.0), w as f32 / h as f32);

        self.state = Some(AppState {
            window,
            gpu,
            mesh_pipeline,
            mesh,
            camera,
            fps_controller: FpsController::new(),
            camera_uniform,
            camera_buffer,
            light_uniform,
            light_buffer,
            camera_light_bind_group,
            diffuse_texture,
            input: InputState::new(),
            timer: GameTimer::new(60),
            model_rotation: 0.0,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event: winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: key_state,
                    ..
                },
                ..
            } => {
                let pressed = key_state == winit::event::ElementState::Pressed;
                state.input.process_key(key_code, pressed);
                if key_code == KeyCode::Escape && pressed {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                state.gpu.resize(size.width, size.height);
                if size.width > 0 && size.height > 0 {
                    state.camera.aspect = size.width as f32 / size.height as f32;
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.input.process_mouse_move(position.x, position.y);
            }

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let pressed = btn_state == winit::event::ElementState::Pressed;
                state.input.process_mouse_button(button, pressed);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                state.input.process_scroll(y);
            }

            WindowEvent::RedrawRequested => {
                state.timer.tick();
                let dt = state.timer.frame_dt();

                // 마우스 우클릭 드래그로 카메라 회전
                if state.input.is_mouse_button_pressed(winit::event::MouseButton::Right) {
                    let (dx, dy) = state.input.mouse_delta();
                    state.fps_controller.process_mouse(&mut state.camera, dx, dy);
                }

                // WASD 이동
                let forward = if state.input.is_key_pressed(KeyCode::KeyW) { 1.0 }
                    else if state.input.is_key_pressed(KeyCode::KeyS) { -1.0 }
                    else { 0.0 };
                let right = if state.input.is_key_pressed(KeyCode::KeyD) { 1.0 }
                    else if state.input.is_key_pressed(KeyCode::KeyA) { -1.0 }
                    else { 0.0 };
                let up = if state.input.is_key_pressed(KeyCode::Space) { 1.0 }
                    else if state.input.is_key_pressed(KeyCode::ShiftLeft) { -1.0 }
                    else { 0.0 };
                state.fps_controller.process_movement(&mut state.camera, forward, right, up, dt);

                state.input.begin_frame();

                // 모델 자동 회전
                state.model_rotation += dt * 0.5;
                let model = Mat4::rotation_y(state.model_rotation);

                // Uniform 업데이트
                state.camera_uniform.view_proj = state.camera.view_projection().cols;
                state.camera_uniform.model = model.cols;
                state.camera_uniform.camera_pos = [
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                ];
                state.gpu.queue.write_buffer(
                    &state.camera_buffer,
                    0,
                    bytemuck::cast_slice(&[state.camera_uniform]),
                );

                // 렌더링
                let output = match state.gpu.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(wgpu::SurfaceError::Lost) => {
                        let (w, h) = state.window.inner_size();
                        state.gpu.resize(w, h);
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                        return;
                    }
                    Err(_) => return,
                };

                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Mesh Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.1,
                                    b: 0.15,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.gpu.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });

                    render_pass.set_pipeline(&state.mesh_pipeline);
                    render_pass.set_bind_group(0, &state.camera_light_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.diffuse_texture.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, state.mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        state.mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..state.mesh.num_indices, 0, 0..1);
                }

                state.gpu.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = ModelViewerApp { state: None };
    event_loop.run_app(&mut app).unwrap();
}
```

- [ ] **Step 5: 빌드 확인**

Run: `cargo build -p model_viewer`
Expected: 빌드 성공

- [ ] **Step 6: 실행 확인**

Run: `cargo run -p model_viewer`
Expected: 큐브가 Blinn-Phong 라이팅으로 렌더링됨. 자동 회전. 마우스 우클릭+드래그로 카메라 회전. WASD로 이동. ESC로 종료.

- [ ] **Step 7: 커밋**

```bash
git add Cargo.toml assets/ examples/model_viewer/
git commit -m "feat: add model viewer demo with OBJ loading, Blinn-Phong lighting, FPS camera"
```

---

## Phase 2 완료 기준 체크리스트

- [ ] `cargo build --workspace` 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] `cargo run -p triangle` — 기존 삼각형 데모 여전히 동작
- [ ] `cargo run -p model_viewer` — 큐브 렌더링, 라이팅, 카메라 조작 동작
- [ ] OBJ 파서 테스트 통과 (삼각형, 쿼드, UV, 정점 중복 제거)
- [ ] BMP 파서 테스트 통과
- [ ] Mat4 테스트 통과 (identity, translation, rotation, look_at, perspective)
