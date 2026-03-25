use voltex_math::{Vec3, Ray, AABB};

/// Ray vs AABB (slab method). Returns t of nearest intersection, or None.
/// If ray starts inside AABB, returns Some(0.0).
pub fn ray_vs_aabb(ray: &Ray, aabb: &AABB) -> Option<f32> {
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
pub fn ray_vs_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<(f32, Vec3)> {
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
pub fn ray_vs_box(ray: &Ray, center: Vec3, half_extents: Vec3) -> Option<(f32, Vec3)> {
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
}
