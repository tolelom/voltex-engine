//! Old public paths of the modules and items moved to [`crate::experimental`].
//!
//! Deprecated aliases kept so existing code still compiles; nothing in the
//! engine uses them. A module alias (`voltex_physics::mesh_collider`, …) warns when the module
//! itself is imported with `use`; type and function aliases warn at every use.

// ── Modules ──────────────────────────────────────────────────────────────

/// Moved to [`experimental::mesh_collider`](crate::experimental::mesh_collider).
#[deprecated(note = "moved to `voltex_physics::experimental::mesh_collider`; not used by the engine")]
pub mod mesh_collider {
    pub use crate::experimental::mesh_collider::*;
}

// ── Types that were re-exported at the crate root ────────────────────────

/// Moved to [`experimental::mesh_collider::MeshCollider`](crate::experimental::mesh_collider::MeshCollider).
#[deprecated(note = "moved to `voltex_physics::experimental::mesh_collider::MeshCollider`; not used by the engine")]
pub type MeshCollider = crate::experimental::mesh_collider::MeshCollider;

/// Moved to [`experimental::mesh_collider::MeshHit`](crate::experimental::mesh_collider::MeshHit).
#[deprecated(note = "moved to `voltex_physics::experimental::mesh_collider::MeshHit`; not used by the engine")]
pub type MeshHit = crate::experimental::mesh_collider::MeshHit;

// ── Functions that were re-exported at the crate root ────────────────────

/// Moved to [`experimental::mesh_collider::ray_vs_mesh`](crate::experimental::mesh_collider::ray_vs_mesh).
#[deprecated(note = "moved to `voltex_physics::experimental::mesh_collider::ray_vs_mesh`; not used by the engine")]
#[inline]
pub fn ray_vs_mesh(
    ray: &voltex_math::Ray,
    mesh: &crate::experimental::mesh_collider::MeshCollider,
) -> Option<crate::experimental::mesh_collider::MeshHit> {
    crate::experimental::mesh_collider::ray_vs_mesh(ray, mesh)
}

/// Moved to [`experimental::mesh_collider::ray_vs_mesh_all`](crate::experimental::mesh_collider::ray_vs_mesh_all).
#[deprecated(note = "moved to `voltex_physics::experimental::mesh_collider::ray_vs_mesh_all`; not used by the engine")]
#[inline]
pub fn ray_vs_mesh_all(
    ray: &voltex_math::Ray,
    mesh: &crate::experimental::mesh_collider::MeshCollider,
) -> Vec<crate::experimental::mesh_collider::MeshHit> {
    crate::experimental::mesh_collider::ray_vs_mesh_all(ray, mesh)
}
