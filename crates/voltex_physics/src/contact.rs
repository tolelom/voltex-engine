use voltex_ecs::Entity;
use voltex_math::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub normal: Vec3,
    pub depth: f32,
    pub point_on_a: Vec3,
    pub point_on_b: Vec3,
}
