//! Viewport raycast picking — turns a mouse click inside the 3D viewport into
//! an `Entity` selection.
//!
//! The pipeline is split into pure helpers so each step is independently
//! testable:
//!   1. `ndc_from_viewport` — pixel mouse pos → NDC (-1..1, y up).
//!   2. `ray_from_ndc` — NDC + inverse view-projection + camera pos → world ray.
//!   3. `ray_vs_sphere_t` — analytical sphere intersection, returns smallest
//!      non-negative `t` (or None).
//!   4. `pick_entity` — composes the above against `world.query::<Transform>()`
//!      using a default-radius sphere per entity, scaled by max(transform.scale).
//!
//! v1 limitation: we don't store per-entity mesh bounds in ECS yet, so picking
//! uses a unit-sphere approximation (radius * max(scale)). This is fine for the
//! editor_demo's cube grid; future work can switch to AABB once meshes carry
//! bounds. Documented in docs/DEFERRED.md.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Mat4, Ray, Vec3, Vec4, AABB};

/// Convert a pixel mouse position to NDC space within a viewport rect.
///
/// The returned NDC has x in -1..1 left→right and y in -1..1 bottom→top
/// (the standard NDC convention; screen-y is flipped). Caller is responsible
/// for first ensuring the mouse is inside the viewport rect.
pub fn ndc_from_viewport(
    mouse_x: f32,
    mouse_y: f32,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
) -> (f32, f32) {
    let rx = (mouse_x - rect_x) / rect_w.max(1e-6);
    let ry = (mouse_y - rect_y) / rect_h.max(1e-6);
    (rx * 2.0 - 1.0, 1.0 - ry * 2.0)
}

/// Construct a world-space ray from an NDC point given the inverse
/// view-projection matrix and the camera world position.
///
/// We unproject the NDC point at depth `1.0` (wgpu's far plane) to get a world
/// point on the far plane, then build the ray from camera position toward it.
/// This avoids needing to unproject at two depths and subtracting.
pub fn ray_from_ndc(view_proj_inv: &Mat4, cam_pos: Vec3, ndc: (f32, f32)) -> Ray {
    let clip = Vec4::new(ndc.0, ndc.1, 1.0, 1.0);
    let world = view_proj_inv.mul_vec4(clip);
    let w = if world.w.abs() < 1e-8 { 1.0 } else { world.w };
    let far = Vec3::new(world.x / w, world.y / w, world.z / w);
    Ray::new(cam_pos, far - cam_pos)
}

/// Smallest non-negative `t` along the ray at which it hits the sphere, or
/// `None` if the ray misses (or only intersects behind the origin).
pub fn ray_vs_sphere_t(ray: &Ray, center: Vec3, radius: f32) -> Option<f32> {
    let oc = ray.origin - center;
    let b = ray.direction.dot(oc);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t1 = -b - sq;
    let t2 = -b + sq;
    if t1 >= 0.0 {
        Some(t1)
    } else if t2 >= 0.0 {
        Some(t2)
    } else {
        None
    }
}

/// Ray vs axis-aligned bounding box (slab method).
/// Returns the smallest non-negative `t` (clamped to 0 if ray starts inside),
/// or `None` if the ray misses or only intersects behind the origin.
///
/// voltex_physics has an equivalent — this is duplicated here to avoid a
/// voltex_editor → voltex_physics dependency.
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
            let inv = 1.0 / d[i];
            let mut t1 = (bmin[i] - o[i]) * inv;
            let mut t2 = (bmax[i] - o[i]) * inv;
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

/// Transform a mesh-local AABB into world space by transforming the 8 corners
/// and rebuilding an axis-aligned AABB. Loose for rotated meshes (OBB picking
/// is out of scope per E1 spec).
pub fn world_aabb_from_local(local: &AABB, world: &Mat4) -> AABB {
    let corners = [
        Vec3::new(local.min.x, local.min.y, local.min.z),
        Vec3::new(local.max.x, local.min.y, local.min.z),
        Vec3::new(local.min.x, local.max.y, local.min.z),
        Vec3::new(local.max.x, local.max.y, local.min.z),
        Vec3::new(local.min.x, local.min.y, local.max.z),
        Vec3::new(local.max.x, local.min.y, local.max.z),
        Vec3::new(local.min.x, local.max.y, local.max.z),
        Vec3::new(local.max.x, local.max.y, local.max.z),
    ];
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for c in &corners {
        let w = *world * Vec4::new(c.x, c.y, c.z, 1.0);
        let inv_w = if w.w.abs() < 1e-8 { 1.0 } else { 1.0 / w.w };
        let p = Vec3::new(w.x * inv_w, w.y * inv_w, w.z * inv_w);
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    AABB::new(min, max)
}

/// Walk every entity that has a `Transform` and return the closest hit using
/// a sphere approximation. `default_radius` is the unit sphere size; each
/// entity's effective radius is `default_radius * max(scale.x, scale.y, scale.z)`.
///
/// Returns `None` if the mouse is outside the viewport rect, the matrix is
/// singular, or no entity is hit.
pub fn pick_entity(
    world: &World,
    view_proj: &Mat4,
    cam_pos: Vec3,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
    mouse_x: f32,
    mouse_y: f32,
    default_radius: f32,
) -> Option<Entity> {
    if mouse_x < rect_x
        || mouse_x >= rect_x + rect_w
        || mouse_y < rect_y
        || mouse_y >= rect_y + rect_h
    {
        return None;
    }
    let inv = view_proj.inverse()?;
    let ndc = ndc_from_viewport(mouse_x, mouse_y, rect_x, rect_y, rect_w, rect_h);
    let ray = ray_from_ndc(&inv, cam_pos, ndc);

    let mut best: Option<(Entity, f32)> = None;
    for (entity, t) in world.query::<Transform>() {
        let s = t.scale.x.max(t.scale.y).max(t.scale.z).max(1e-6);
        let r = default_radius * s;
        if let Some(hit_t) = ray_vs_sphere_t(&ray, t.position, r) {
            best = match best {
                Some((_, bt)) if bt <= hit_t => best,
                _ => Some((entity, hit_t)),
            };
        }
    }
    best.map(|(e, _)| e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OrbitCamera;
    use voltex_ecs::World;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn ndc_center_is_zero() {
        let (x, y) = ndc_from_viewport(50.0, 30.0, 0.0, 0.0, 100.0, 60.0);
        assert!(approx(x, 0.0, 1e-5));
        assert!(approx(y, 0.0, 1e-5));
    }

    #[test]
    fn ndc_corners() {
        // top-left → (-1, +1)
        let (x, y) = ndc_from_viewport(0.0, 0.0, 0.0, 0.0, 100.0, 60.0);
        assert!(approx(x, -1.0, 1e-5));
        assert!(approx(y, 1.0, 1e-5));
        // bottom-right → (+1, -1) (using w-1 / h-1 sentinel to land *inside* the rect)
        let (x, y) = ndc_from_viewport(100.0, 60.0, 0.0, 0.0, 100.0, 60.0);
        assert!(approx(x, 1.0, 1e-5));
        assert!(approx(y, -1.0, 1e-5));
    }

    #[test]
    fn ndc_offset_rect() {
        let (x, y) = ndc_from_viewport(75.0, 30.0, 50.0, 20.0, 50.0, 20.0);
        // mouse halfway in x (centred) and halfway in y → (0, 0)
        assert!(approx(x, 0.0, 1e-5));
        assert!(approx(y, 0.0, 1e-5));
    }

    #[test]
    fn ray_through_center_points_at_target() {
        // Use the default OrbitCamera; the ray fired at NDC (0, 0) should point
        // from camera position toward the camera's target (which is the origin).
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let inv = vp.inverse().expect("vp invertible");
        let ray = ray_from_ndc(&inv, cam.position(), (0.0, 0.0));

        let expected_dir = (cam.target - cam.position()).normalize();
        // Tolerance is loose because inverse(perspective) is mildly
        // ill-conditioned; pick_selects_centered_entity verifies the math
        // works end-to-end in practice.
        assert!(approx(ray.direction.x, expected_dir.x, 1e-2),
            "x: {} vs {}", ray.direction.x, expected_dir.x);
        assert!(approx(ray.direction.y, expected_dir.y, 1e-2),
            "y: {} vs {}", ray.direction.y, expected_dir.y);
        assert!(approx(ray.direction.z, expected_dir.z, 1e-2),
            "z: {} vs {}", ray.direction.z, expected_dir.z);
    }

    #[test]
    fn sphere_hit_straight_on() {
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        let t = ray_vs_sphere_t(&ray, Vec3::new(0.0, 0.0, -5.0), 1.0);
        // First intersection should be at t = 4 (just before the sphere center).
        assert!(t.is_some());
        assert!(approx(t.unwrap(), 4.0, 1e-4));
    }

    #[test]
    fn sphere_miss_to_the_side() {
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        let t = ray_vs_sphere_t(&ray, Vec3::new(10.0, 0.0, -5.0), 1.0);
        assert!(t.is_none());
    }

    #[test]
    fn sphere_behind_origin_is_none() {
        // Sphere is BEHIND the ray (ray points -Z, sphere at +Z).
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        let t = ray_vs_sphere_t(&ray, Vec3::new(0.0, 0.0, 5.0), 1.0);
        assert!(t.is_none());
    }

    #[test]
    fn sphere_origin_inside_returns_forward_hit() {
        // If the ray origin is inside the sphere, t1 < 0 < t2 — we want t2.
        let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        let t = ray_vs_sphere_t(&ray, Vec3::ZERO, 1.0);
        assert!(t.is_some());
        assert!(approx(t.unwrap(), 1.0, 1e-4));
    }

    #[test]
    fn pick_returns_none_when_mouse_outside_rect() {
        let world = World::new();
        let vp = Mat4::IDENTITY;
        let result = pick_entity(
            &world, &vp, Vec3::ZERO,
            10.0, 10.0, 100.0, 100.0,
            5.0, 5.0,   // mouse to upper-left of the rect
            1.0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn pick_returns_none_on_empty_world() {
        let world = World::new();
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        // Click dead-centre, but world has no entities.
        let result = pick_entity(
            &world, &vp, cam.position(),
            0.0, 0.0, 800.0, 600.0,
            400.0, 300.0,
            1.0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn pick_selects_centered_entity() {
        // Camera looks at origin from default OrbitCamera. Spawn a unit-sized
        // entity at the origin and click the centre of the viewport — should
        // pick that entity.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::new());

        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let result = pick_entity(
            &world, &vp, cam.position(),
            0.0, 0.0, 800.0, 800.0,
            400.0, 400.0,   // centre
            1.0,
        );
        assert_eq!(result, Some(e));
    }

    #[test]
    fn pick_selects_nearest_when_multiple_under_cursor() {
        // Two entities on the camera's view axis. The closer one should win.
        let mut world = World::new();
        let cam = OrbitCamera::new();

        // Place 'near' entity at camera target (origin) and 'far' entity further
        // along the camera forward direction (past the target). Click centre →
        // ray passes through both → near must be returned.
        let near_e = world.spawn();
        world.add(near_e, Transform::from_position(Vec3::ZERO));

        let cam_to_target = (cam.target - cam.position()).normalize();
        let far_pos = cam.target + cam_to_target * 10.0;
        let far_e = world.spawn();
        world.add(far_e, Transform::from_position(far_pos));

        let vp = cam.view_projection(1.0);
        let result = pick_entity(
            &world, &vp, cam.position(),
            0.0, 0.0, 800.0, 800.0,
            400.0, 400.0,
            1.0,
        );
        assert_eq!(result, Some(near_e));
    }

    #[test]
    fn pick_misses_entity_off_to_the_side() {
        // Single entity, but it sits at (10, 0, 0) — well outside the radius of
        // the click ray.
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(10.0, 0.0, 0.0)));

        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let result = pick_entity(
            &world, &vp, cam.position(),
            0.0, 0.0, 800.0, 800.0,
            400.0, 400.0,
            0.5,   // small radius
        );
        assert_ne!(result, Some(e));
    }

    #[test]
    fn aabb_hit_front_face() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 4.0, 1e-4));
    }

    #[test]
    fn aabb_miss_parallel() {
        let ray = Ray::new(Vec3::new(-5.0, 5.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(ray_vs_aabb(&ray, &aabb).is_none());
    }

    #[test]
    fn aabb_inside_returns_zero() {
        let ray = Ray::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let aabb = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let t = ray_vs_aabb(&ray, &aabb).unwrap();
        assert!(approx(t, 0.0, 1e-4));
    }

    #[test]
    fn world_aabb_translation_only() {
        let local = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let m = Mat4::translation(10.0, 20.0, 30.0);
        let world = world_aabb_from_local(&local, &m);
        assert!(approx(world.min.x, 9.0, 1e-4));
        assert!(approx(world.max.z, 31.0, 1e-4));
    }

    #[test]
    fn world_aabb_rotation_loosens() {
        // 45° rotation around Y expands the X-Z footprint of a unit AABB.
        let local = AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let m = Mat4::rotation_y(std::f32::consts::FRAC_PI_4);
        let world = world_aabb_from_local(&local, &m);
        // After 45° rotation, the corners of a unit cube reach sqrt(2) ≈ 1.414 in x or z.
        assert!(world.max.x > 1.3 && world.max.x < 1.5,
            "expected ~sqrt(2), got {}", world.max.x);
        assert!(world.max.z > 1.3 && world.max.z < 1.5);
    }
}
