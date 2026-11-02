//! Contact manifold produced by the narrowphase.

use voltex_ecs::Entity;
use voltex_math::Vec3;

/// One contact point between two colliding entities.
#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    /// First entity participating in the contact.
    pub entity_a: Entity,
    /// Second entity participating in the contact.
    pub entity_b: Entity,
    /// Contact normal in world space (points from A to B).
    pub normal: Vec3,
    /// Penetration depth along the normal (positive = overlapping).
    pub depth: f32,
    /// Contact point in world space, expressed relative to A.
    pub point_on_a: Vec3,
    /// Contact point in world space, expressed relative to B.
    pub point_on_b: Vec3,
}
