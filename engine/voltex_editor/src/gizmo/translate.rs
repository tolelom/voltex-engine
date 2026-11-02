//! Translate gizmo — axis-aligned thin AABB hit detection +
//! drag-plane projection for axis-locked motion.

use voltex_ecs::Transform;
use voltex_math::{Ray, Vec3, AABB};

use super::{DragState, GizmoAxis};

const AXIS_LENGTH: f32 = 1.5;
const AXIS_THICKNESS: f32 = 0.08;
const AXIS_BASE_OFFSET: f32 = 0.15;

/// Local-space AABB for axis A (centred at origin). Convert to world by
/// translating by `gizmo_origin`.
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

/// Pick the most camera-facing plane that contains `axis`. Returns the plane
/// normal (orthogonal to axis).
pub(crate) fn pick_drag_plane_normal(axis: GizmoAxis, gizmo_origin: Vec3, cam_pos: Vec3) -> Vec3 {
    let cam_dir = (cam_pos - gizmo_origin).normalize();
    let (n1, n2) = match axis {
        GizmoAxis::X => (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        GizmoAxis::Y => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        GizmoAxis::Z => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    };
    let d1 = cam_dir.dot(n1).abs();
    let d2 = cam_dir.dot(n2).abs();
    if d1 >= d2 { n1 } else { n2 }
}

/// Intersect `ray` with the plane through `origin` with `normal`. Returns the
/// hit point, or None if parallel / behind ray.
fn ray_plane_hit(ray: &Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = normal.dot(ray.direction);
    if denom.abs() < 1e-6 { return None; }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 { return None; }
    Some(ray.origin + ray.direction * t)
}

/// Test the ray against each axis's thin AABB (translated by gizmo_origin).
/// Returns the axis with the nearest hit, or None.
pub(crate) fn hit_axis(gizmo_origin: Vec3, ray: &Ray) -> Option<GizmoAxis> {
    let mut best: Option<(GizmoAxis, f32)> = None;
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let local = axis_local_aabb(axis);
        let world = AABB::new(local.min + gizmo_origin, local.max + gizmo_origin);
        if let Some(t) = crate::picking::ray_vs_aabb(ray, &world) {
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
    let delta = current - drag.anchor;
    let mut t = drag.initial_transform;
    t.position = t.position + drag.axis.dir() * delta;
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool { (a - b).abs() <= tol }

    #[test]
    fn drag_plane_with_camera_on_gizmo_origin_is_a_unit_axis() {
        // cam_dir is NaN here; every NaN comparison is false so the second
        // candidate wins. Pinned so a refactor can't start returning NaN.
        let n = pick_drag_plane_normal(GizmoAxis::X, Vec3::ZERO, Vec3::ZERO);
        assert!(approx(n.length(), 1.0, 1e-6), "{n:?}");
    }

    #[test]
    fn hit_x_axis_straight_on() {
        // Gizmo at origin. Ray fired down +X axis from -X side hits the X arrow.
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let axis = hit_axis(Vec3::ZERO, &ray);
        assert_eq!(axis, Some(GizmoAxis::X));
    }

    #[test]
    fn drag_plane_normal_prefers_facing() {
        // Camera looks down +Y. For axis X, both Y and Z planes are candidates;
        // Y normal dot=1, Z normal dot=0 → pick Y.
        let n = pick_drag_plane_normal(GizmoAxis::X, Vec3::ZERO, Vec3::new(0.0, 10.0, 0.0));
        assert!(approx(n.y, 1.0, 1e-4));
    }

    #[test]
    fn drag_along_axis_moves_position() {
        // X-axis DragState: plane_normal = Y, anchor at origin.
        let drag = DragState {
            axis: GizmoAxis::X,
            initial_transform: Transform::new(),
            gizmo_origin: Vec3::ZERO,
            plane_normal: Vec3::new(0.0, 1.0, 0.0),
            anchor: 0.0,
        };
        // Current ray: shifted so it hits (1, 0, 0) on the Y=0 plane.
        let current_ray = Ray::new(Vec3::new(1.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let t = compute_transform(&drag, &current_ray);
        assert!(approx(t.position.x, 1.0, 1e-4), "x: {}", t.position.x);
        assert!(approx(t.position.y, 0.0, 1e-4));
        assert!(approx(t.position.z, 0.0, 1e-4));
    }
}
