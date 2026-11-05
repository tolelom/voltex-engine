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

/// How two shapes overlap, as reported by the narrowphase (no entities yet).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Penetration {
    /// Unit direction from A to B.
    pub normal: Vec3,
    /// Overlap distance along `normal` (≥ 0).
    pub depth: f32,
    /// Deepest point of A inside B, in world space.
    pub point_on_a: Vec3,
    /// Deepest point of B inside A, in world space.
    pub point_on_b: Vec3,
}

impl Penetration {
    /// The same overlap seen from B's side: normal reversed, points swapped.
    pub fn flipped(self) -> Self {
        Self {
            normal: -self.normal,
            depth: self.depth,
            point_on_a: self.point_on_b,
            point_on_b: self.point_on_a,
        }
    }
}
