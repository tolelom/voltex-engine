//! Narrowphase contact generation between specific shape pairs (sphere-sphere,
//! sphere-box, capsule-sphere, etc.). Each function returns the contact tuple
//! `(normal_A_to_B, depth, point_on_a, point_on_b)` or `None`.

use voltex_math::Vec3;

/// Returns (normal A→B, depth, point_on_a, point_on_b) or None if no collision.
pub(crate) fn sphere_vs_sphere(
    pos_a: Vec3, radius_a: f32,
    pos_b: Vec3, radius_b: f32,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let diff = pos_b - pos_a;
    let dist_sq = diff.length_squared();
    let sum_r = radius_a + radius_b;

    if dist_sq > sum_r * sum_r {
        return None;
    }

    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-8 {
        diff * (1.0 / dist)
    } else {
        Vec3::Y
    };

    let depth = sum_r - dist;
    let point_on_a = pos_a + normal * radius_a;
    let point_on_b = pos_b - normal * radius_b;

    Some((normal, depth, point_on_a, point_on_b))
}

/// Sphere-vs-Box (AABB) contact. Returns `(normal A→B, depth, point_on_a, point_on_b)`.
pub(crate) fn sphere_vs_box(
    sphere_pos: Vec3, radius: f32,
    box_pos: Vec3, half_extents: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let bmin = box_pos - half_extents;
    let bmax = box_pos + half_extents;

    let closest = Vec3::new(
        sphere_pos.x.clamp(bmin.x, bmax.x),
        sphere_pos.y.clamp(bmin.y, bmax.y),
        sphere_pos.z.clamp(bmin.z, bmax.z),
    );

    let diff = sphere_pos - closest;
    let dist_sq = diff.length_squared();

    // Sphere center outside box
    if dist_sq > 1e-8 {
        let dist = dist_sq.sqrt();
        if dist > radius {
            return None;
        }
        let normal = diff * (-1.0 / dist); // sphere→box direction
        let depth = radius - dist;
        let point_on_a = sphere_pos + normal * radius; // sphere surface toward box
        let point_on_b = closest;
        return Some((normal, depth, point_on_a, point_on_b));
    }

    // Sphere center inside box — find nearest face
    let dx_min = sphere_pos.x - bmin.x;
    let dx_max = bmax.x - sphere_pos.x;
    let dy_min = sphere_pos.y - bmin.y;
    let dy_max = bmax.y - sphere_pos.y;
    let dz_min = sphere_pos.z - bmin.z;
    let dz_max = bmax.z - sphere_pos.z;

    let mut min_dist = dx_min;
    let mut normal = Vec3::new(-1.0, 0.0, 0.0);
    let mut closest_face = Vec3::new(bmin.x, sphere_pos.y, sphere_pos.z);

    if dx_max < min_dist {
        min_dist = dx_max;
        normal = Vec3::new(1.0, 0.0, 0.0);
        closest_face = Vec3::new(bmax.x, sphere_pos.y, sphere_pos.z);
    }
    if dy_min < min_dist {
        min_dist = dy_min;
        normal = Vec3::new(0.0, -1.0, 0.0);
        closest_face = Vec3::new(sphere_pos.x, bmin.y, sphere_pos.z);
    }
    if dy_max < min_dist {
        min_dist = dy_max;
        normal = Vec3::new(0.0, 1.0, 0.0);
        closest_face = Vec3::new(sphere_pos.x, bmax.y, sphere_pos.z);
    }
    if dz_min < min_dist {
        min_dist = dz_min;
        normal = Vec3::new(0.0, 0.0, -1.0);
        closest_face = Vec3::new(sphere_pos.x, sphere_pos.y, bmin.z);
    }
    if dz_max < min_dist {
        min_dist = dz_max;
        normal = Vec3::new(0.0, 0.0, 1.0);
        closest_face = Vec3::new(sphere_pos.x, sphere_pos.y, bmax.z);
    }

    let depth = min_dist + radius;
    let point_on_a = sphere_pos + normal * radius;
    let point_on_b = closest_face;

    Some((normal, depth, point_on_a, point_on_b))
}

/// Box-vs-Box (AABB) contact. Returns `(normal A→B, depth, point_on_a, point_on_b)`.
pub(crate) fn box_vs_box(
    pos_a: Vec3, half_a: Vec3,
    pos_b: Vec3, half_b: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let diff = pos_b - pos_a;

    let overlap_x = (half_a.x + half_b.x) - diff.x.abs();
    if overlap_x < 0.0 { return None; }

    let overlap_y = (half_a.y + half_b.y) - diff.y.abs();
    if overlap_y < 0.0 { return None; }

    let overlap_z = (half_a.z + half_b.z) - diff.z.abs();
    if overlap_z < 0.0 { return None; }

    let (normal, depth) = if overlap_x <= overlap_y && overlap_x <= overlap_z {
        let sign = if diff.x >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(sign, 0.0, 0.0), overlap_x)
    } else if overlap_y <= overlap_z {
        let sign = if diff.y >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(0.0, sign, 0.0), overlap_y)
    } else {
        let sign = if diff.z >= 0.0 { 1.0 } else { -1.0 };
        (Vec3::new(0.0, 0.0, sign), overlap_z)
    };

    let point_on_a = pos_a + Vec3::new(
        normal.x * half_a.x,
        normal.y * half_a.y,
        normal.z * half_a.z,
    );
    let point_on_b = pos_b - Vec3::new(
        normal.x * half_b.x,
        normal.y * half_b.y,
        normal.z * half_b.z,
    );

    Some((normal, depth, point_on_a, point_on_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 1e-5 }
    fn approx_vec(a: Vec3, b: Vec3) -> bool { approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z) }

    // sphere_vs_sphere tests
    #[test]
    fn test_sphere_sphere_separated() {
        let r = sphere_vs_sphere(Vec3::ZERO, 1.0, Vec3::new(5.0, 0.0, 0.0), 1.0);
        assert!(r.is_none());
    }

    #[test]
    fn test_sphere_sphere_overlapping() {
        let r = sphere_vs_sphere(Vec3::ZERO, 1.0, Vec3::new(1.5, 0.0, 0.0), 1.0);
        let (normal, depth, pa, pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.5));
        assert!(approx_vec(pa, Vec3::new(1.0, 0.0, 0.0)));
        assert!(approx_vec(pb, Vec3::new(0.5, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_sphere_touching() {
        let r = sphere_vs_sphere(Vec3::ZERO, 1.0, Vec3::new(2.0, 0.0, 0.0), 1.0);
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.0));
    }

    #[test]
    fn test_sphere_sphere_coincident() {
        let r = sphere_vs_sphere(Vec3::ZERO, 1.0, Vec3::ZERO, 1.0);
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx(depth, 2.0));
    }

    // sphere_vs_box tests
    #[test]
    fn test_sphere_box_separated() {
        let r = sphere_vs_box(Vec3::new(5.0, 0.0, 0.0), 1.0, Vec3::ZERO, Vec3::ONE);
        assert!(r.is_none());
    }

    #[test]
    fn test_sphere_box_face_overlap() {
        let r = sphere_vs_box(Vec3::new(1.5, 0.0, 0.0), 1.0, Vec3::ZERO, Vec3::ONE);
        let (normal, depth, _pa, pb) = r.unwrap();
        assert!(approx(normal.x, -1.0));
        assert!(approx(depth, 0.5));
        assert!(approx(pb.x, 1.0));
    }

    #[test]
    fn test_sphere_box_center_inside() {
        let r = sphere_vs_box(Vec3::ZERO, 0.5, Vec3::ZERO, Vec3::ONE);
        assert!(r.is_some());
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(depth > 0.0);
    }

    // box_vs_box tests
    #[test]
    fn test_box_box_separated() {
        let r = box_vs_box(Vec3::ZERO, Vec3::ONE, Vec3::new(5.0, 0.0, 0.0), Vec3::ONE);
        assert!(r.is_none());
    }

    #[test]
    fn test_box_box_overlapping() {
        let r = box_vs_box(Vec3::ZERO, Vec3::ONE, Vec3::new(1.5, 0.0, 0.0), Vec3::ONE);
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::X));
        assert!(approx(depth, 0.5));
    }

    #[test]
    fn test_box_box_touching() {
        let r = box_vs_box(Vec3::ZERO, Vec3::ONE, Vec3::new(2.0, 0.0, 0.0), Vec3::ONE);
        let (_normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx(depth, 0.0));
    }

    #[test]
    fn test_box_box_y_axis() {
        let r = box_vs_box(Vec3::ZERO, Vec3::ONE, Vec3::new(0.0, 1.5, 0.0), Vec3::ONE);
        let (normal, depth, _pa, _pb) = r.unwrap();
        assert!(approx_vec(normal, Vec3::Y));
        assert!(approx(depth, 0.5));
    }
}
