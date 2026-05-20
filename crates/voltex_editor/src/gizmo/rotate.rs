//! Rotate gizmo — Task 7.

use voltex_ecs::Transform;
use voltex_math::{Ray, Vec3};

use super::DragState;

pub(crate) fn try_begin(_origin: Vec3, _ray: &Ray, _cam_pos: Vec3, _initial: Transform) -> Option<DragState> {
    None
}

pub(crate) fn compute_transform(drag: &DragState, _ray: &Ray) -> Transform {
    drag.initial_transform
}
