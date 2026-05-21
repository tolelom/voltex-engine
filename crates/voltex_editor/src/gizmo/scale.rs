//! Scale gizmo — same hit detection as translate; drag delta becomes a
//! per-axis scale multiplier.

use voltex_ecs::Transform;
use voltex_math::{Ray, Vec3, AABB};

use super::{DragState, GizmoAxis};

const AXIS_LENGTH: f32 = 1.5;
const HANDLE_HALF: f32 = 0.12;
const HANDLE_OFFSET: f32 = AXIS_LENGTH - HANDLE_HALF;
const AXIS_THICKNESS: f32 = 0.08;
const AXIS_BASE_OFFSET: f32 = 0.15;

fn handle_local_aabb(axis: GizmoAxis) -> AABB {
    let dir = axis.dir();
    let center = dir * HANDLE_OFFSET;
    AABB::new(
        center - Vec3::new(HANDLE_HALF, HANDLE_HALF, HANDLE_HALF),
        center + Vec3::new(HANDLE_HALF, HANDLE_HALF, HANDLE_HALF),
    )
}

fn axis_local_aabb(axis: GizmoAxis) -> AABB {
    match axis {
        GizmoAxis::X => AABB::new(
            Vec3::new(AXIS_BASE_OFFSET, -AXIS_THICKNESS, -AXIS_THICKNESS),
            Vec3::new(AXIS_LENGTH, AXIS_THICKNESS, AXIS_THICKNESS),
        ),
        GizmoAxis::Y => AABB::new(
            Vec3::new(-AXIS_THICKNESS, AXIS_BASE_OFFSET, -AXIS_THICKNESS),
            Vec3::new(AXIS_THICKNESS, AXIS_LENGTH, AXIS_THICKNESS),
        ),
        GizmoAxis::Z => AABB::new(
            Vec3::new(-AXIS_THICKNESS, -AXIS_THICKNESS, AXIS_BASE_OFFSET),
            Vec3::new(AXIS_THICKNESS, AXIS_THICKNESS, AXIS_LENGTH),
        ),
    }
}

fn pick_drag_plane_normal(axis: GizmoAxis, gizmo_origin: Vec3, cam_pos: Vec3) -> Vec3 {
    let cam_dir = (cam_pos - gizmo_origin).normalize();
    let (n1, n2) = match axis {
        GizmoAxis::X => (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        GizmoAxis::Y => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        GizmoAxis::Z => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    };
    if cam_dir.dot(n1).abs() >= cam_dir.dot(n2).abs() { n1 } else { n2 }
}

fn ray_plane_hit(ray: &Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = normal.dot(ray.direction);
    if denom.abs() < 1e-6 { return None; }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 { return None; }
    Some(ray.origin + ray.direction * t)
}

/// Picks which scale handle the ray hits, if any.
pub fn hit_axis(gizmo_origin: Vec3, ray: &Ray) -> Option<GizmoAxis> {
    let mut best: Option<(GizmoAxis, f32)> = None;
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let bar = axis_local_aabb(axis);
        let bar_w = AABB::new(bar.min + gizmo_origin, bar.max + gizmo_origin);
        if let Some(t) = crate::picking::ray_vs_aabb(ray, &bar_w) {
            best = match best {
                Some((_, bt)) if bt <= t => best,
                _ => Some((axis, t)),
            };
        }
        let h = handle_local_aabb(axis);
        let h_w = AABB::new(h.min + gizmo_origin, h.max + gizmo_origin);
        if let Some(t) = crate::picking::ray_vs_aabb(ray, &h_w) {
            best = match best {
                Some((_, bt)) if bt <= t => best,
                _ => Some((axis, t)),
            };
        }
    }
    best.map(|(a, _)| a)
}

pub(crate) fn try_begin(origin: Vec3, ray: &Ray, cam_pos: Vec3, initial: Transform) -> Option<DragState> {
    let axis = hit_axis(origin, ray)?;
    let plane_normal = pick_drag_plane_normal(axis, origin, cam_pos);
    let hit = ray_plane_hit(ray, origin, plane_normal)?;
    let anchor = (hit - origin).dot(axis.dir());
    Some(DragState {
        axis,
        initial_transform: initial,
        gizmo_origin: origin,
        plane_normal,
        anchor,
    })
}

pub(crate) fn compute_transform(drag: &DragState, ray: &Ray) -> Transform {
    let hit = match ray_plane_hit(ray, drag.gizmo_origin, drag.plane_normal) {
        Some(h) => h,
        None => return drag.initial_transform,
    };
    let current = (hit - drag.gizmo_origin).dot(drag.axis.dir());
    let safe_anchor = if drag.anchor.abs() < 1e-3 {
        drag.anchor.signum().max(1.0) * 1e-3
    } else {
        drag.anchor
    };
    let factor = (current / safe_anchor).max(0.01);
    let mut t = drag.initial_transform;
    let idx = drag.axis.index();
    let mut s = [t.scale.x, t.scale.y, t.scale.z];
    s[idx] *= factor;
    t.scale = Vec3::new(s[0], s[1], s[2]);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool { (a - b).abs() <= tol }

    #[test]
    fn hit_x_axis_via_handle() {
        // Ray hits the X scale handle (small cube at axis tip).
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let axis = hit_axis(Vec3::ZERO, &ray);
        assert_eq!(axis, Some(GizmoAxis::X));
    }

    #[test]
    fn drag_scales_by_ratio() {
        // Anchor at X = 1.0. Current at X = 2.0 → factor = 2 → scale doubles.
        let drag = DragState {
            axis: GizmoAxis::X,
            initial_transform: Transform::new(),
            gizmo_origin: Vec3::ZERO,
            plane_normal: Vec3::new(0.0, 1.0, 0.0),
            anchor: 1.0,
        };
        let ray = Ray::new(Vec3::new(2.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let t = compute_transform(&drag, &ray);
        assert!(approx(t.scale.x, 2.0, 1e-3), "scale.x: {}", t.scale.x);
        assert!(approx(t.scale.y, 1.0, 1e-3));
        assert!(approx(t.scale.z, 1.0, 1e-3));
    }
}
