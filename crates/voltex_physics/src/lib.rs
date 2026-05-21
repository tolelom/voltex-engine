//! # voltex_physics
//!
//! Physics primitives — collision detection + rigid body simulation.
//!
//! - Broadphase: [`BvhTree`].
//! - Colliders: [`Collider`] (Sphere/Box/Capsule/ConvexHull/Mesh) +
//!   [`ConvexHull`] + [`MeshCollider`].
//! - Narrowphase: [`narrow`] + [`gjk`] + [`contact::ContactPoint`].
//! - Solver: [`physics_step`] / [`resolve_collisions`] (Sequential Impulse).
//! - Integration: [`integrate`], [`inertia_tensor`], [`inv_inertia`].
//! - Raycasting: [`raycast`] / [`raycast_all`] + [`ray_vs_triangle`] /
//!   [`ray_vs_plane`].
//! - CCD: [`swept_sphere_vs_aabb`].

#![warn(missing_docs)]

pub mod bvh;
pub mod ray;
pub mod collider;
pub mod contact;
pub mod narrow;
pub mod gjk;
pub mod collision;
pub mod rigid_body;
pub mod integrator;
pub mod solver;
pub mod raycast;
pub mod ccd;
pub mod mesh_collider;

pub use bvh::BvhTree;
pub use collider::{Collider, ConvexHull};
pub use contact::ContactPoint;
pub use collision::detect_collisions;
pub use rigid_body::{RigidBody, PhysicsConfig};
pub use integrator::{integrate, inertia_tensor, inv_inertia};
pub use solver::{resolve_collisions, physics_step};
pub use raycast::{RayHit, raycast, raycast_all};
pub use ray::{ray_vs_triangle, ray_vs_plane};
pub use ccd::swept_sphere_vs_aabb;
pub use mesh_collider::{MeshCollider, MeshHit, ray_vs_mesh, ray_vs_mesh_all};
