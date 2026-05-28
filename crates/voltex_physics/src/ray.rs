//! Geometric ray-vs-shape primitives (sphere, box, capsule, triangle, plane,
//! AABB). Used by the broadphase + narrowphase + raycasting layers.

use voltex_math::{Vec3, Ray, AABB};

/// Ray vs AABB (slab method). Returns t of nearest intersection, or None.
/// If ray starts inside AABB, returns Some(0.0).
pub(crate) fn ray_vs_aabb(ray: &Ray, aabb: &AABB) -> Option<f32> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;

    let o = [ray.origin.x, ray.origin.y, ray.origin.z];
    let d = [ray.direction.x, ray.direction.y, ray.direction.z];
    let bmin = [aabb.min.x, aabb.min.y, aabb.min.z];
    let bmax = [aabb.max.x, aabb.max.y, aabb.max.z];

    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < bmin[i] || o[i] > bmax[i] {
                return None;
            }
        } else {
            let inv_d = 1.0 / d[i];
            let mut t1 = (bmin[i] - o[i]) * inv_d;
            let mut t2 = (bmax[i] - o[i]) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return None;
            }
        }
    }

    if t_max < 0.0 {
        return None;
    }

    Some(t_min.max(0.0))
}

/// Ray vs Sphere. Returns (t, normal) or None.
pub(crate) fn ray_vs_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<(f32, Vec3)> {
    let oc = ray.origin - center;
    let a = ray.direction.dot(ray.direction);
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let mut t = (-b - sqrt_d) / (2.0 * a);

    if t < 0.0 {
        t = (-b + sqrt_d) / (2.0 * a);
        if t < 0.0 {
            return None;
        }
    }

    let point = ray.at(t);
    let normal = (point - center).normalize();
    Some((t, normal))
}

/// Ray vs axis-aligned Box. Returns (t, normal) or None.
pub(crate) fn ray_vs_box(ray: &Ray, center: Vec3, half_extents: Vec3) -> Option<(f32, Vec3)> {
    let aabb = AABB::from_center_half_extents(center, half_extents);
    let t = ray_vs_aabb(ray, &aabb)?;

    if t == 0.0 {
        // Ray starts inside box
        let bmin = aabb.min;
        let bmax = aabb.max;
        let p = ray.origin;

        let faces: [(f32, Vec3); 6] = [
            (p.x - bmin.x, Vec3::new(-1.0, 0.0, 0.0)),
            (bmax.x - p.x, Vec3::new(1.0, 0.0, 0.0)),
            (p.y - bmin.y, Vec3::new(0.0, -1.0, 0.0)),
            (bmax.y - p.y, Vec3::new(0.0, 1.0, 0.0)),
            (p.z - bmin.z, Vec3::new(0.0, 0.0, -1.0)),
            (bmax.z - p.z, Vec3::new(0.0, 0.0, 1.0)),
        ];

        let mut min_dist = f32::INFINITY;
        let mut normal = Vec3::Y;
        for (dist, n) in &faces {
            if *dist < min_dist {
                min_dist = *dist;
                normal = *n;
            }
        }
        return Some((0.0, normal));
    }

    let hit = ray.at(t);
    let rel = hit - center;
    let hx = half_extents.x;
    let hy = half_extents.y;
    let hz = half_extents.z;

    let normal = if (rel.x - hx).abs() < 1e-4 {
        Vec3::X
    } else if (rel.x + hx).abs() < 1e-4 {
        Vec3::new(-1.0, 0.0, 0.0)
    } else if (rel.y - hy).abs() < 1e-4 {
        Vec3::Y
    } else if (rel.y + hy).abs() < 1e-4 {
        Vec3::new(0.0, -1.0, 0.0)
    } else if (rel.z - hz).abs() < 1e-4 {
        Vec3::Z
    } else {
        Vec3::new(0.0, 0.0, -1.0)
    };

    Some((t, normal))
}

/// Ray vs Capsule (Y-axis aligned). Returns (t, normal) or None.
/// Capsule = cylinder of given half_height along Y + hemisphere caps of given radius.
pub(crate) fn ray_vs_capsule(ray: &Ray, center: Vec3, radius: f32, half_height: f32) -> Option<(f32, Vec3)> {
    // Test the two hemisphere caps
    let top_center = Vec3::new(center.x, center.y + half_height, center.z);
    let bot_center = Vec3::new(center.x, center.y - half_height, center.z);

    let mut best: Option<(f32, Vec3)> = None;

    // Test infinite cylinder (ignore Y component for the 2D circle test)
    // Ray in XZ plane relative to capsule center
    let ox = ray.origin.x - center.x;
    let oz = ray.origin.z - center.z;
    let dx = ray.direction.x;
    let dz = ray.direction.z;

    let a = dx * dx + dz * dz;
    let b = 2.0 * (ox * dx + oz * dz);
    let c = ox * ox + oz * oz - radius * radius;

    if a > 1e-10 {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sqrt_d = disc.sqrt();
            for sign in [-1.0f32, 1.0] {
                let t = (-b + sign * sqrt_d) / (2.0 * a);
                if t >= 0.0 {
                    let hit_y = ray.origin.y + ray.direction.y * t;
                    // Check if hit is within the cylinder portion
                    if hit_y >= center.y - half_height && hit_y <= center.y + half_height {
                        let hit = ray.at(t);
                        let normal = Vec3::new(hit.x - center.x, 0.0, hit.z - center.z).normalize();
                        if best.is_none() || t < best.unwrap().0 {
                            best = Some((t, normal));
                        }
                    }
                }
            }
        }
    }

    // Test top hemisphere
    if let Some((t, normal)) = ray_vs_sphere(ray, top_center, radius) {
        let hit = ray.at(t);
        if hit.y >= center.y + half_height
            && (best.is_none() || t < best.unwrap().0) {
                best = Some((t, normal));
            }
    }

    // Test bottom hemisphere
    if let Some((t, normal)) = ray_vs_sphere(ray, bot_center, radius) {
        let hit = ray.at(t);
        if hit.y <= center.y - half_height
            && (best.is_none() || t < best.unwrap().0) {
                best = Some((t, normal));
            }
    }

    best
}

/// Ray vs Triangle (Möller–Trumbore algorithm).
/// Returns (t, normal) where normal is the triangle face normal.
pub fn ray_vs_triangle(ray: &Ray, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<(f32, Vec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if a.abs() < 1e-8 {
        return None; // Ray is parallel to triangle
    }

    let f = 1.0 / a;
    let s = ray.origin - v0;
    let u = f * s.dot(h);

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);

    if t < 0.0 {
        return None; // Triangle is behind the ray
    }

    let normal = edge1.cross(edge2).normalize();
    Some((t, normal))
}

/// Ray vs infinite plane intersection.
/// plane_normal: unit normal of the plane
/// plane_distance: signed distance from origin (plane equation: dot(normal, point) = distance)
/// Returns distance t along ray, or None if parallel/behind.
pub fn ray_vs_plane(ray: &Ray, plane_normal: Vec3, plane_distance: f32) -> Option<f32> {
    let denom = plane_normal.dot(ray.direction);
    if denom.abs() < 1e-8 { return None; } // parallel
    let t = (plane_distance - plane_normal.dot(ray.origin)) / denom;
    if t < 0.0 { None } else { Some(t) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn approx_vec(a: Vec3, b: Vec3) -> bool {
        approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z)
    }

    #[test]
    fn test_aabb_hit() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 4.0));
    }

    #[test]
    fn test_aabb_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        assert!(ray_vs_aabb(&ray, &aabb).is_none());
    }

    #[test]
    fn test_aabb_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE);
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 0.0));
    }

    #[test]
    fn test_sphere_hit() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let (t, normal) = ray_vs_sphere(&ray, Vec3::ZERO, 1.0).unwrap();
        assert!(approx(t, 4.0));
        assert!(approx_vec(normal, Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        assert!(ray_vs_sphere(&ray, Vec3::ZERO, 1.0).is_none());
    }

    #[test]
    fn test_sphere_tangent() {
        let ray = Ray::new(Vec3::new(-5.0, 1.0, 0.0), Vec3::X);
        assert!(ray_vs_sphere(&ray, Vec3::ZERO, 1.0).is_some());
    }

    #[test]
    fn test_sphere_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let (t, _normal) = ray_vs_sphere(&ray, Vec3::ZERO, 2.0).unwrap();
        assert!(approx(t, 2.0));
    }

    #[test]
    fn test_box_hit_face() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        let (t, normal) = ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).unwrap();
        assert!(approx(t, 4.0));
        assert!(approx_vec(normal, Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_box_miss() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        assert!(ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).is_none());
    }

    #[test]
    fn test_box_inside() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let (t, _normal) = ray_vs_box(&ray, Vec3::ZERO, Vec3::ONE).unwrap();
        assert!(approx(t, 0.0));
    }

    // --- ray_vs_triangle tests ---

    #[test]
    fn test_triangle_hit() {
        let v0 = Vec3::new(-1.0, -1.0, 5.0);
        let v1 = Vec3::new(1.0, -1.0, 5.0);
        let v2 = Vec3::new(0.0, 1.0, 5.0);
        let ray = Ray::new(Vec3::ZERO, Vec3::Z);
        let (t, normal) = ray_vs_triangle(&ray, v0, v1, v2).unwrap();
        assert!(approx(t, 5.0));
        // Normal should point toward -Z (facing the ray)
        assert!(normal.z.abs() > 0.9);
    }

    #[test]
    fn test_triangle_miss() {
        let v0 = Vec3::new(-1.0, -1.0, 5.0);
        let v1 = Vec3::new(1.0, -1.0, 5.0);
        let v2 = Vec3::new(0.0, 1.0, 5.0);
        // Ray pointing away
        let ray = Ray::new(Vec3::new(10.0, 10.0, 0.0), Vec3::Z);
        assert!(ray_vs_triangle(&ray, v0, v1, v2).is_none());
    }

    #[test]
    fn test_triangle_behind_ray() {
        let v0 = Vec3::new(-1.0, -1.0, -5.0);
        let v1 = Vec3::new(1.0, -1.0, -5.0);
        let v2 = Vec3::new(0.0, 1.0, -5.0);
        let ray = Ray::new(Vec3::ZERO, Vec3::Z);
        assert!(ray_vs_triangle(&ray, v0, v1, v2).is_none());
    }

    #[test]
    fn test_triangle_parallel() {
        let v0 = Vec3::new(0.0, 0.0, 5.0);
        let v1 = Vec3::new(1.0, 0.0, 5.0);
        let v2 = Vec3::new(0.0, 0.0, 6.0);
        // Ray parallel to triangle (in XZ plane)
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        assert!(ray_vs_triangle(&ray, v0, v1, v2).is_none());
    }

    #[test]
    fn test_triangle_edge_hit() {
        // Ray hitting exactly on an edge
        let v0 = Vec3::new(-1.0, 0.0, 5.0);
        let v1 = Vec3::new(1.0, 0.0, 5.0);
        let v2 = Vec3::new(0.0, 2.0, 5.0);
        // Hit on the midpoint of v0-v1 edge
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::Z);
        let result = ray_vs_triangle(&ray, v0, v1, v2);
        assert!(result.is_some());
        let (t, _) = result.unwrap();
        assert!(approx(t, 5.0));
    }

    // --- ray_vs_plane tests ---

    #[test]
    fn test_ray_vs_plane_hit() {
        let ray = Ray { origin: Vec3::new(0.0, 5.0, 0.0), direction: Vec3::new(0.0, -1.0, 0.0) };
        let t = ray_vs_plane(&ray, Vec3::new(0.0, 1.0, 0.0), 0.0).unwrap();
        assert!((t - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_ray_vs_plane_parallel() {
        let ray = Ray { origin: Vec3::new(0.0, 5.0, 0.0), direction: Vec3::new(1.0, 0.0, 0.0) };
        assert!(ray_vs_plane(&ray, Vec3::new(0.0, 1.0, 0.0), 0.0).is_none());
    }

    #[test]
    fn test_ray_vs_plane_behind() {
        let ray = Ray { origin: Vec3::new(0.0, 5.0, 0.0), direction: Vec3::new(0.0, 1.0, 0.0) };
        assert!(ray_vs_plane(&ray, Vec3::new(0.0, 1.0, 0.0), 0.0).is_none()); // plane below, ray going up
    }
}
