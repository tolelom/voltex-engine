//! RenderItem + collect_render_items — Task 3 에서 구현.

use voltex_ecs::{Entity, MeshHandle, World};
use voltex_math::Mat4;
use voltex_renderer::MaterialUniform;

use crate::assets::RenderAssets;

pub struct RenderItem {
    pub entity: Entity,
    pub model: Mat4,
    pub mesh: MeshHandle,
    pub material: MaterialUniform,
}

pub fn collect_render_items(_world: &mut World, _assets: &RenderAssets) -> Vec<RenderItem> {
    Vec::new()
}
