//! GJK + EPA distance / penetration solver for convex shapes.

use voltex_math::Vec3;
use crate::collider::Collider;

/// Returns the farthest point on a convex collider in a given direction.
fn support(collider: &Collider, position: Vec3, direction: Vec3) -> Vec3 {
    match collider {
        Collider::Sphere { radius } => {
            let len = direction.length();
            if len < 1e-10 {
                return position;
            }
            position + direction * (*radius / len)
        }
        Collider::Box { half_extents } => {
            Vec3::new(
                position.x + if direction.x >= 0.0 { half_extents.x } else { -half_extents.x },
                position.y + if direction.y >= 0.0 { half_extents.y } else { -half_extents.y },
                position.z + if direction.z >= 0.0 { half_extents.z } else { -half_extents.z },
            )
        }
        Collider::Capsule { radius, half_height } => {
            let base = if direction.y >= 0.0 {
                Vec3::new(position.x, position.y + *half_height, position.z)
            } else {
                Vec3::new(position.x, position.y - *half_height, position.z)
            };
            let len = direction.length();
            if len < 1e-10 {
                return base;
            }
            base + direction * (*radius / len)
        }
        Collider::ConvexHull(hull) => {
            position + hull.support(direction)
        }
    }
}

/// Minkowski difference support: support_A(dir) - support_B(-dir)
fn support_minkowski(a: &Collider, pos_a: Vec3, b: &Collider, pos_b: Vec3, dir: Vec3) -> Vec3 {
    support(a, pos_a, dir) - support(b, pos_b, -dir)
}

/// GJK: returns Some(simplex) if shapes overlap, None if separated.
pub fn gjk(a: &Collider, pos_a: Vec3, b: &Collider, pos_b: Vec3) -> Option<Vec<Vec3>> {
    let mut dir = pos_b - pos_a;
    if dir.length_squared() < 1e-10 {
        dir = Vec3::X;
    }

    let mut simplex: Vec<Vec3> = Vec::new();
    let s = support_minkowski(a, pos_a, b, pos_b, dir);
    simplex.push(s);
    dir = -s;

    for _ in 0..64 {
        if dir.length_squared() < 1e-20 {
            return build_tetrahedron(&simplex, a, pos_a, b, pos_b);
        }

        let new_point = support_minkowski(a, pos_a, b, pos_b, dir);

        // If the new point didn't pass the origin, no intersection
        if new_point.dot(dir) < 0.0 {
            return None;
        }

        simplex.push(new_point);

        if process_simplex(&mut simplex, &mut dir) {
            return build_tetrahedron(&simplex, a, pos_a, b, pos_b);
        }
    }

    if simplex.len() >= 4 {
        Some(simplex)
    } else {
        None
    }
}

fn build_tetrahedron(
    simplex: &[Vec3],
    a: &Collider, pos_a: Vec3,
    b: &Collider, pos_b: Vec3,
) -> Option<Vec<Vec3>> {
    let mut pts: Vec<Vec3> = simplex.to_vec();
    if pts.len() >= 4 {
        return Some(pts[..4].to_vec());
    }
    let dirs = [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z];
    for &d in &dirs {
        if pts.len() >= 4 { break; }
        let p = support_minkowski(a, pos_a, b, pos_b, d);
        if !pts.iter().any(|s| (*s - p).length_squared() < 1e-10) {
            pts.push(p);
        }
    }
    if pts.len() >= 4 {
        Some(pts[..4].to_vec())
    } else {
        Some(pts)
    }
}

fn process_simplex(simplex: &mut Vec<Vec3>, dir: &mut Vec3) -> bool {
    match simplex.len() {
        2 => process_line(simplex, dir),
        3 => process_triangle(simplex, dir),
        4 => process_tetrahedron(simplex, dir),
        _ => false,
    }
}

// Simplex is [B, A] where A is the most recently added point
fn process_line(simplex: &mut Vec<Vec3>, dir: &mut Vec3) -> bool {
    let a = simplex[1];
    let b = simplex[0];
    let ab = b - a;
    let ao = -a;

    if ab.dot(ao) > 0.0 {
        // Origin is in the region of the line AB
        // Direction perpendicular to AB toward origin
        let t = ab.cross(ao).cross(ab);
        if t.length_squared() < 1e-20 {
            // Origin is on the line — use any perpendicular
            *dir = perpendicular_to(ab);
        } else {
            *dir = t;
        }
    } else {
        // Origin is behind A, closest to A alone
        *simplex = vec![a];
        *dir = ao;
    }
    false
}

// Simplex is [C, B, A] where A is the most recently added
fn process_triangle(simplex: &mut Vec<Vec3>, dir: &mut Vec3) -> bool {
    let a = simplex[2];
    let b = simplex[1];
    let c = simplex[0];

    let ab = b - a;
    let ac = c - a;
    let ao = -a;
    let abc = ab.cross(ac); // triangle normal

    // Check if origin is outside edge AB (on the side away from C)
    let ab_normal = ab.cross(abc); // perpendicular to AB, pointing away from triangle interior
    if ab_normal.dot(ao) > 0.0 {
        // Origin is outside AB edge
        if ab.dot(ao) > 0.0 {
            *simplex = vec![b, a];
            *dir = ab.cross(ao).cross(ab);
            if dir.length_squared() < 1e-20 {
                *dir = perpendicular_to(ab);
            }
        } else {
            *simplex = vec![a];
            *dir = ao;
        }
        return false;
    }

    // Check if origin is outside edge AC (on the side away from B)
    let ac_normal = abc.cross(ac); // perpendicular to AC, pointing away from triangle interior
    if ac_normal.dot(ao) > 0.0 {
        // Origin is outside AC edge
        if ac.dot(ao) > 0.0 {
            *simplex = vec![c, a];
            *dir = ac.cross(ao).cross(ac);
            if dir.length_squared() < 1e-20 {
                *dir = perpendicular_to(ac);
            }
        } else {
            *simplex = vec![a];
            *dir = ao;
        }
        return false;
    }

    // Origin is within the triangle prism — check above or below
    if abc.dot(ao) > 0.0 {
        // Origin is above the triangle
        *dir = abc;
    } else {
        // Origin is below the triangle — flip winding
        *simplex = vec![b, c, a];
        *dir = -abc;
    }
    false
}

// Simplex is [D, C, B, A] where A is the most recently added
fn process_tetrahedron(simplex: &mut Vec<Vec3>, dir: &mut Vec3) -> bool {
    let a = simplex[3];
    let b = simplex[2];
    let c = simplex[1];
    let d = simplex[0];

    let ab = b - a;
    let ac = c - a;
    let ad = d - a;
    let ao = -a;

    // Face normals, oriented outward from the tetrahedron
    let abc = ab.cross(ac);
    let acd = ac.cross(ad);
    let adb = ad.cross(ab);

    // Ensure normals point outward: ABC normal should point away from D
    let abc_out = if abc.dot(ad) < 0.0 { abc } else { -abc };
    let acd_out = if acd.dot(ab) < 0.0 { acd } else { -acd };
    let adb_out = if adb.dot(ac) < 0.0 { adb } else { -adb };

    if abc_out.dot(ao) > 0.0 {
        // Origin above face ABC — reduce to triangle ABC
        *simplex = vec![c, b, a];
        *dir = abc_out;
        return false;
    }
    if acd_out.dot(ao) > 0.0 {
        // Origin above face ACD — reduce to triangle ACD
        *simplex = vec![d, c, a];
        *dir = acd_out;
        return false;
    }
    if adb_out.dot(ao) > 0.0 {
        // Origin above face ADB — reduce to triangle ADB
        *simplex = vec![b, d, a];
        *dir = adb_out;
        return false;
    }

    // Origin is inside the tetrahedron
    true
}

fn perpendicular_to(v: Vec3) -> Vec3 {
    let candidate = if v.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    v.cross(candidate)
}

// ==================== EPA ====================

const EPA_MAX_ITER: usize = 64;
const EPA_TOLERANCE: f32 = 0.0001;

#[derive(Clone)]
struct EpaFace {
    indices: [usize; 3],
    normal: Vec3,
    distance: f32,
}

/// EPA: given a simplex containing the origin, find penetration info.
/// Returns (normal, depth, point_on_a, point_on_b).
pub fn epa(
    a: &Collider, pos_a: Vec3,
    b: &Collider, pos_b: Vec3,
    simplex: &[Vec3],
) -> (Vec3, f32, Vec3, Vec3) {
    let mut polytope: Vec<Vec3> = simplex.to_vec();

    if polytope.len() < 4 {
        let diff = pos_b - pos_a;
        let len = diff.length();
        let normal = if len > 1e-8 { diff * (1.0 / len) } else { Vec3::Y };
        return (normal, 0.0, pos_a, pos_b);
    }

    // Build initial 4 faces of tetrahedron
    let mut faces: Vec<EpaFace> = Vec::new();
    let face_indices: [[usize; 3]; 4] = [
        [0, 1, 2],
        [0, 3, 1],
        [0, 2, 3],
        [1, 3, 2],
    ];

    for indices in &face_indices {
        let mut face = make_face(&polytope, *indices);
        // Ensure outward-pointing normal (away from origin)
        if face.distance < 0.0 {
            face.indices.swap(0, 1);
            face.normal = -face.normal;
            face.distance = -face.distance;
        }
        if face.normal.length_squared() > 1e-10 {
            faces.push(face);
        }
    }

    let mut closest_face = EpaFace {
        indices: [0, 0, 0],
        normal: Vec3::Y,
        distance: f32::MAX,
    };

    for _ in 0..EPA_MAX_ITER {
        if faces.is_empty() {
            break;
        }

        // Find closest face
        let mut min_dist = f32::MAX;
        let mut min_idx = 0;
        for (i, face) in faces.iter().enumerate() {
            if face.distance < min_dist {
                min_dist = face.distance;
                min_idx = i;
            }
        }
        closest_face = faces[min_idx].clone();

        let new_point = support_minkowski(a, pos_a, b, pos_b, closest_face.normal);
        let new_dist = new_point.dot(closest_face.normal);

        if new_dist - min_dist < EPA_TOLERANCE {
            break;
        }

        // Add new point
        let new_idx = polytope.len();
        polytope.push(new_point);

        // Find and remove visible faces, collect horizon edges
        let mut edges: Vec<[usize; 2]> = Vec::new();
        faces.retain(|face| {
            let v = polytope[face.indices[0]];
            if face.normal.dot(new_point - v) > 0.0 {
                for i in 0..3 {
                    let edge = [face.indices[i], face.indices[(i + 1) % 3]];
                    let rev_idx = edges.iter().position(|e| e[0] == edge[1] && e[1] == edge[0]);
                    if let Some(idx) = rev_idx {
                        edges.remove(idx);
                    } else {
                        edges.push(edge);
                    }
                }
                false
            } else {
                true
            }
        });

        // Create new faces from horizon edges
        for edge in &edges {
            let mut face = make_face(&polytope, [edge[0], edge[1], new_idx]);
            if face.distance < 0.0 {
                face.indices.swap(0, 1);
                face.normal = -face.normal;
                face.distance = -face.distance;
            }
            if face.normal.length_squared() > 1e-10 {
                faces.push(face);
            }
        }
    }

    let normal = closest_face.normal;
    let depth = closest_face.distance.max(0.0);

    let point_on_a = support(a, pos_a, normal);
    let point_on_b = support(b, pos_b, -normal);

    (normal, depth, point_on_a, point_on_b)
}

fn make_face(polytope: &[Vec3], indices: [usize; 3]) -> EpaFace {
    let a = polytope[indices[0]];
    let b = polytope[indices[1]];
    let c = polytope[indices[2]];

    let ab = b - a;
    let ac = c - a;
    let mut normal = ab.cross(ac);
    let len = normal.length();
    if len > 1e-10 {
        normal = normal * (1.0 / len);
    } else {
        normal = Vec3::Y;
    }

    let distance = normal.dot(a);

    EpaFace {
        indices,
        normal,
        distance,
    }
}

/// Combined GJK + EPA: returns (normal, depth, point_on_a, point_on_b) or None.
pub fn gjk_epa(
    a: &Collider, pos_a: Vec3,
    b: &Collider, pos_b: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let simplex = gjk(a, pos_a, b, pos_b)?;
    Some(epa(a, pos_a, b, pos_b, &simplex))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn test_gjk_spheres_overlapping() {
        let a = Collider::Sphere { radius: 1.0 };
        let b = Collider::Sphere { radius: 1.0 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(1.5, 0.0, 0.0);

        let result = gjk_epa(&a, pos_a, &b, pos_b);
        assert!(result.is_some());
        let (normal, depth, _pa, _pb) = result.unwrap();
        assert!(normal.x.abs() > 0.5);
        assert!(approx(depth, 0.5, 0.1));
    }

    #[test]
    fn test_gjk_spheres_separated() {
        let a = Collider::Sphere { radius: 1.0 };
        let b = Collider::Sphere { radius: 1.0 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(5.0, 0.0, 0.0);

        let result = gjk(&a, pos_a, &b, pos_b);
        assert!(result.is_none());
    }

    #[test]
    fn test_gjk_capsule_vs_sphere_overlap() {
        let a = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let b = Collider::Sphere { radius: 1.0 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(1.0, 0.0, 0.0);

        let result = gjk_epa(&a, pos_a, &b, pos_b);
        assert!(result.is_some());
        let (_normal, depth, _pa, _pb) = result.unwrap();
        assert!(depth > 0.0);
    }

    #[test]
    fn test_gjk_capsule_vs_sphere_separated() {
        let a = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let b = Collider::Sphere { radius: 0.5 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(5.0, 0.0, 0.0);

        let result = gjk(&a, pos_a, &b, pos_b);
        assert!(result.is_none());
    }

    #[test]
    fn test_gjk_capsule_vs_box_overlap() {
        let a = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let b = Collider::Box { half_extents: Vec3::ONE };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(1.0, 0.0, 0.0);

        let result = gjk_epa(&a, pos_a, &b, pos_b);
        assert!(result.is_some());
        let (_normal, depth, _pa, _pb) = result.unwrap();
        assert!(depth > 0.0);
    }

    #[test]
    fn test_gjk_capsule_vs_capsule_overlap() {
        let a = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let b = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(0.5, 0.0, 0.0);

        let result = gjk_epa(&a, pos_a, &b, pos_b);
        assert!(result.is_some());
        let (_normal, depth, _pa, _pb) = result.unwrap();
        assert!(depth > 0.0);
    }

    #[test]
    fn test_gjk_capsule_vs_capsule_separated() {
        let a = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let b = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let pos_a = Vec3::ZERO;
        let pos_b = Vec3::new(5.0, 0.0, 0.0);

        let result = gjk(&a, pos_a, &b, pos_b);
        assert!(result.is_none());
    }

    #[test]
    fn test_support_sphere() {
        let c = Collider::Sphere { radius: 2.0 };
        let p = support(&c, Vec3::ZERO, Vec3::X);
        assert!(approx(p.x, 2.0, 1e-5));
        assert!(approx(p.y, 0.0, 1e-5));
        assert!(approx(p.z, 0.0, 1e-5));
    }

    #[test]
    fn test_support_box() {
        let c = Collider::Box { half_extents: Vec3::new(1.0, 2.0, 3.0) };
        let p = support(&c, Vec3::ZERO, Vec3::new(1.0, -1.0, 1.0));
        assert!(approx(p.x, 1.0, 1e-5));
        assert!(approx(p.y, -2.0, 1e-5));
        assert!(approx(p.z, 3.0, 1e-5));
    }

    #[test]
    fn test_support_capsule() {
        let c = Collider::Capsule { radius: 0.5, half_height: 1.0 };
        let p = support(&c, Vec3::ZERO, Vec3::Y);
        assert!(approx(p.y, 1.5, 1e-5));
    }
}
