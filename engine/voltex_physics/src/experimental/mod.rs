//! Experimental physics pieces.
//!
//! Implemented and unit-tested, but not used by the engine runtime yet.
//! APIs here may change or be removed.
//!
//! [`MeshCollider`](mesh_collider::MeshCollider) answers ray queries only: it
//! is not a [`Collider`](crate::Collider) variant, so collision detection and
//! [`raycast`](crate::raycast) never see it. The old paths
//! (`voltex_physics::MeshCollider`, `voltex_physics::mesh_collider`, …) still
//! resolve but are deprecated.

pub mod mesh_collider;
