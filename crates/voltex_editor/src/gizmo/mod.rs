//! Gizmo — translate / rotate / scale handles for the selected entity.
//!
//! Drag math lives in `translate`, `rotate`, `scale` submodules. Rendering
//! is in `renderer`. State and transitions live here.

pub mod translate;
pub mod rotate;
pub mod scale;
pub mod renderer;

use voltex_ecs::{Entity, Transform, World, WorldTransform};
use voltex_math::{Mat4, Vec3};

pub use renderer::GizmoRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

impl GizmoAxis {
    pub fn dir(self) -> Vec3 {
        match self {
            GizmoAxis::X => Vec3::new(1.0, 0.0, 0.0),
            GizmoAxis::Y => Vec3::new(0.0, 1.0, 0.0),
            GizmoAxis::Z => Vec3::new(0.0, 0.0, 1.0),
        }
    }
    pub fn index(self) -> usize {
        match self {
            GizmoAxis::X => 0,
            GizmoAxis::Y => 1,
            GizmoAxis::Z => 2,
        }
    }
}

/// Result returned by `end_drag`: caller dispatches `SetTransform` so the
/// drag becomes a single undo entry. `initial` is the Transform value at
/// drag start (caller rolls World back to this so `dispatch` captures the
/// correct inverse), `final_transform` is the value at drag end.
pub struct DragResult {
    pub initial: Transform,
    pub final_transform: Transform,
}

#[derive(Debug, Clone)]
pub(crate) struct DragState {
    pub(crate) axis: GizmoAxis,
    pub(crate) initial_transform: Transform,
    pub(crate) gizmo_origin: Vec3,
    pub(crate) plane_normal: Vec3,
    /// For translate/scale: axis projection of the ray-plane hit at drag start.
    /// For rotate: angle at drag start.
    pub(crate) anchor: f32,
}

pub struct GizmoState {
    pub mode: GizmoMode,
    pub(crate) drag: Option<DragState>,
}

impl GizmoState {
    pub fn new() -> Self {
        Self { mode: GizmoMode::Translate, drag: None }
    }

    pub fn set_mode(&mut self, mode: GizmoMode) {
        if self.drag.is_none() {
            self.mode = mode;
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn active_axis(&self) -> Option<GizmoAxis> {
        self.drag.as_ref().map(|d| d.axis)
    }

    /// World position of the gizmo for the given entity (uses WorldTransform
    /// translation, else Transform.position, else None).
    pub fn gizmo_origin(world: &World, entity: Entity) -> Option<Vec3> {
        if let Some(wt) = world.get::<WorldTransform>(entity) {
            return Some(Vec3::new(wt.0.cols[3][0], wt.0.cols[3][1], wt.0.cols[3][2]));
        }
        world.get::<Transform>(entity).map(|t| t.position)
    }

    /// Try to start a drag. Returns true if a gizmo axis was hit.
    #[allow(clippy::too_many_arguments)]
    pub fn try_begin_drag(
        &mut self,
        world: &mut World,
        entity: Entity,
        view_proj: &Mat4,
        cam_pos: Vec3,
        rect: (f32, f32, f32, f32),
        mouse: (f32, f32),
    ) -> bool {
        if self.drag.is_some() {
            return false;
        }
        voltex_ecs::propagate_transforms(world);
        let origin = match Self::gizmo_origin(world, entity) {
            Some(o) => o,
            None => return false,
        };
        let initial = match world.get::<Transform>(entity) {
            Some(t) => *t,
            None => return false,
        };
        let inv = match view_proj.inverse() {
            Some(m) => m,
            None => return false,
        };
        let ndc = crate::picking::ndc_from_viewport(mouse.0, mouse.1, rect.0, rect.1, rect.2, rect.3);
        let ray = crate::picking::ray_from_ndc(&inv, cam_pos, ndc);

        let drag = match self.mode {
            GizmoMode::Translate => translate::try_begin(origin, &ray, cam_pos, initial),
            GizmoMode::Rotate    => rotate::try_begin(origin, &ray, cam_pos, initial),
            GizmoMode::Scale     => scale::try_begin(origin, &ray, cam_pos, initial),
        };
        if let Some(d) = drag {
            self.drag = Some(d);
            true
        } else {
            false
        }
    }

    /// Update the entity's Transform directly from the current mouse position.
    #[allow(clippy::too_many_arguments)]
    pub fn update_drag(
        &mut self,
        world: &mut World,
        entity: Entity,
        view_proj: &Mat4,
        cam_pos: Vec3,
        rect: (f32, f32, f32, f32),
        mouse: (f32, f32),
    ) {
        let drag = match &self.drag {
            Some(d) => d.clone(),
            None => return,
        };
        let inv = match view_proj.inverse() {
            Some(m) => m,
            None => return,
        };
        let ndc = crate::picking::ndc_from_viewport(mouse.0, mouse.1, rect.0, rect.1, rect.2, rect.3);
        let ray = crate::picking::ray_from_ndc(&inv, cam_pos, ndc);
        let new_t = match self.mode {
            GizmoMode::Translate => translate::compute_transform(&drag, &ray),
            GizmoMode::Rotate    => rotate::compute_transform(&drag, &ray),
            GizmoMode::Scale     => scale::compute_transform(&drag, &ray),
        };
        if let Some(t) = world.get_mut::<Transform>(entity) {
            *t = new_t;
        }
        let _ = cam_pos;
    }

    /// End the drag. Restores the entity Transform to `initial` and returns
    /// `(initial, final)` so the caller dispatches SetTransform once.
    pub fn end_drag(&mut self, world: &mut World, entity: Entity) -> Option<DragResult> {
        let drag = self.drag.take()?;
        let final_transform = *world.get::<Transform>(entity)?;
        // Roll back so dispatch captures the correct inverse.
        if let Some(t) = world.get_mut::<Transform>(entity) {
            *t = drag.initial_transform;
        }
        Some(DragResult { initial: drag.initial_transform, final_transform })
    }
}

impl Default for GizmoState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default_is_translate() {
        let g = GizmoState::new();
        assert_eq!(g.mode, GizmoMode::Translate);
    }

    #[test]
    fn set_mode_changes_mode_when_not_dragging() {
        let mut g = GizmoState::new();
        g.set_mode(GizmoMode::Rotate);
        assert_eq!(g.mode, GizmoMode::Rotate);
        g.set_mode(GizmoMode::Scale);
        assert_eq!(g.mode, GizmoMode::Scale);
    }
}
