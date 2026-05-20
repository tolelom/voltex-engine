//! Rotate gizmo — ring plane intersection + atan2 angle delta.

use voltex_ecs::Transform;
use voltex_math::{Ray, Vec3};

use super::{DragState, GizmoAxis};

const RING_RADIUS: f32 = 1.2;
const RING_TOLERANCE: f32 = 0.15;

fn ring_plane_normal(axis: GizmoAxis) -> Vec3 {
    axis.dir()
}

fn ortho_basis(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.y.abs() < 0.9 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u).normalize();
    (u, v)
}

fn ray_plane_hit(ray: &Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let denom = normal.dot(ray.direction);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + ray.direction * t)
}

fn angle_on_ring(p: Vec3, origin: Vec3, axis: GizmoAxis) -> f32 {
    let (u, v) = ortho_basis(axis.dir());
    let rel = p - origin;
    rel.dot(v).atan2(rel.dot(u))
}

/// Test all three rings; return the axis whose hit lies in the donut
/// (radius ± tolerance), with the smallest ray-t.
pub fn hit_axis(origin: Vec3, ray: &Ray) -> Option<(GizmoAxis, Vec3)> {
    let mut best: Option<(GizmoAxis, f32, Vec3)> = None;
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let n = ring_plane_normal(axis);
        let hit = match ray_plane_hit(ray, origin, n) {
            Some(h) => h,
            None => continue,
        };
        let r = (hit - origin).length();
        if (r - RING_RADIUS).abs() > RING_TOLERANCE {
            continue;
        }
        let t = (hit - ray.origin).length();
        best = match best {
            Some((_, bt, _)) if bt <= t => best,
            _ => Some((axis, t, hit)),
        };
    }
    best.map(|(a, _, h)| (a, h))
}

pub(crate) fn try_begin(origin: Vec3, ray: &Ray, _cam_pos: Vec3, initial: Transform) -> Option<DragState> {
    let (axis, hit) = hit_axis(origin, ray)?;
    let anchor = angle_on_ring(hit, origin, axis);
    Some(DragState {
        axis,
        initial_transform: initial,
        gizmo_origin: origin,
        plane_normal: ring_plane_normal(axis),
        anchor,
    })
}

pub(crate) fn compute_transform(drag: &DragState, ray: &Ray) -> Transform {
    let hit = match ray_plane_hit(ray, drag.gizmo_origin, drag.plane_normal) {
        Some(h) => h,
        None => return drag.initial_transform,
    };
    let cur = angle_on_ring(hit, drag.gizmo_origin, drag.axis);
    let mut delta = cur - drag.anchor;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    let mut t = drag.initial_transform;
    let idx = drag.axis.index();
    let mut rot = [t.rotation.x, t.rotation.y, t.rotation.z];
    rot[idx] += delta;
    t.rotation = Vec3::new(rot[0], rot[1], rot[2]);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn hit_x_ring_in_yz_plane() {
        // Ray fired down +X must hit the YZ ring (X axis ring) at radius ~RING_RADIUS.
        let ray = Ray::new(Vec3::new(-5.0, 0.0, RING_RADIUS), Vec3::new(1.0, 0.0, 0.0));
        let result = hit_axis(Vec3::ZERO, &ray);
        assert!(result.is_some());
        let (axis, _) = result.unwrap();
        assert_eq!(axis, GizmoAxis::X);
    }

    #[test]
    fn angle_delta_drives_rotation() {
        // Y-axis DragState anchored at angle 0.
        // ortho_basis(Y) helper=(1,0,0), u = Y × X = (0,0,-1), v = Y × u = (-1,0,0).
        // Angle 0 corresponds to point (u_dir) = (0, 0, -RING_RADIUS).
        // Angle π/2 corresponds to (v_dir) = (-RING_RADIUS, 0, 0).
        let drag = DragState {
            axis: GizmoAxis::Y,
            initial_transform: Transform::new(),
            gizmo_origin: Vec3::ZERO,
            plane_normal: GizmoAxis::Y.dir(),
            anchor: 0.0,
        };
        // Ray hits Y=0 plane at v_dir position (-RING_RADIUS, 0, 0) → angle π/2.
        let ray = Ray::new(Vec3::new(-RING_RADIUS, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let t = compute_transform(&drag, &ray);
        assert!(
            approx(t.rotation.y, std::f32::consts::FRAC_PI_2, 1e-3),
            "rotation.y: {}",
            t.rotation.y
        );
    }
}
