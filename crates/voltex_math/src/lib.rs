//! Voltex math primitives — `f32`-precision vectors, 4×4 column-major
//! matrices, axis-aligned bounding boxes, and rays. Foundation of every
//! other voltex crate (renderer, physics, AI, ECS-Transform).
//!
//! Conventions:
//! - **Right-handed** coordinate system; `+Y` up.
//! - **Column-major** [`Mat4`] storage — multiplies as `M * v` where `v`
//!   is a column vector. Matches glsl / wgsl uniform layout.
//! - Angles in radians unless otherwise stated.
//! - No SIMD intrinsics — plain `f32` arithmetic for portability.
//!
//! No external dependencies beyond `std`.
//!
//! ```
//! use voltex_math::{Vec3, Mat4};
//! let view = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
//! let proj = Mat4::perspective(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
//! let vp = proj * view;
//! assert!(vp.inverse().is_some());
//! ```

#![warn(missing_docs)]

pub mod vec2;
pub mod vec3;
pub mod vec4;
pub mod mat4;
pub mod aabb;
pub mod ray;

pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;
pub use mat4::Mat4;
pub use aabb::AABB;
pub use ray::Ray;
