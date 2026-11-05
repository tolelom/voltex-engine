//! Viewport raycast picking — turns a mouse click inside the 3D viewport into
//! an `Entity` selection.
//!
//! The pipeline is split into pure helpers so each step is independently
//! testable:
//!   1. `ndc_from_viewport` — pixel mouse pos → NDC (-1..1, y up).
//!   2. `ray_from_ndc` — NDC + inverse view-projection + camera pos → world ray.
//!   3. `ray_vs_sphere_t` — analytical sphere intersection, returns smallest
//!      non-negative `t` (or None).
//!   4. `pick_entity_at` — composes the above against `world.query::<Transform>()`
//!      using a default-radius sphere per entity, scaled by max(transform.scale).
//!
//! [`ViewportCursor`] bundles the inputs of steps 1–2 (camera, viewport rect,
//! mouse) so picking and gizmo dragging cast the ray the same way.
//!
//! v1 limitation: we don't store per-entity mesh bounds in ECS yet, so picking
//! uses a unit-sphere approximation (radius * max(scale)). This is fine for the
//! editor_demo's cube grid; future work can switch to AABB once meshes carry
//! bounds. Documented in docs/dev/DEFERRED.md.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::{Mat4, Ray, Vec3, Vec4, AABB};

use crate::dock::Rect;

/// A mouse position over the 3D viewport together with the camera that
/// viewport renders with — everything needed to cast a picking ray.
#[derive(Debug, Clone, Copy)]
pub struct ViewportCursor {
    /// Camera view-projection matrix.
    pub view_proj: Mat4,
    /// Camera world position (the ray origin).
    pub cam_pos: Vec3,
    /// Viewport rectangle in window pixels.
    pub viewport: Rect,
    /// Mouse position in window pixels.
    pub mouse: (f32, f32),
}

impl ViewportCursor {
    /// Bundles the camera, viewport rect and mouse position.
    pub fn new(view_proj: Mat4, cam_pos: Vec3, viewport: Rect, mouse: (f32, f32)) -> Self {
        Self { view_proj, cam_pos, viewport, mouse }
    }

    /// `true` if the mouse lies outside the viewport rect.
    pub fn is_outside_viewport(&self) -> bool {
        let (mouse_x, mouse_y) = self.mouse;
        let Rect { x, y, w, h } = self.viewport;
        mouse_x < x || mouse_x >= x + w || mouse_y < y || mouse_y >= y + h
    }

    /// World-space ray from the camera through the mouse position, or `None`
    /// if the view-projection matrix is singular.
    pub fn world_ray(&self) -> Option<Ray> {
        let inv = self.view_proj.inverse()?;
        let Rect { x, y, w, h } = self.viewport;
        let ndc = ndc_from_viewport(self.mouse.0, self.mouse.1, x, y, w, h);
        Some(ray_from_ndc(&inv, self.cam_pos, ndc))
    }
}

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
pub(crate) fn world_aabb_from_local(local: &AABB, world: &Mat4) -> AABB {
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

/// Old form of [`pick_entity_at`] with the camera, viewport rect and mouse
/// spelled out as separate arguments.
#[deprecated(note = "use `pick_entity_at` with a `ViewportCursor`")]
#[allow(clippy::too_many_arguments)] // reason: legacy signature kept for compatibility
pub fn pick_entity(
    world: &mut World,
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
    let viewport = Rect { x: rect_x, y: rect_y, w: rect_w, h: rect_h };
    let cursor = ViewportCursor::new(*view_proj, cam_pos, viewport, (mouse_x, mouse_y));
    pick_entity_at(world, &cursor, default_radius)
}

/// Walk every entity that has a `Transform` and return the closest one the
/// mouse ray hits. Entities with `Bounds` + `WorldTransform` are tested as a
/// world-space AABB; others fall back to a sphere at `Transform.position` with
/// radius `default_radius * max(scale.x, scale.y, scale.z)`. An AABB that
/// contains the camera only wins when nothing else is hit.
///
/// Returns `None` if the mouse is outside the viewport rect, the matrix is
/// singular, or no entity is hit.
pub fn pick_entity_at(world: &mut World, cursor: &ViewportCursor, default_radius: f32) -> Option<Entity> {
    if cursor.is_outside_viewport() {
        return None;
    }
    let ray = cursor.world_ray()?;
    let cam_pos = cursor.cam_pos;

    // E1: propagate transforms so WorldTransform reflects current state.
    // Caller may have just dispatched a SetTransform that hasn't been propagated.
    voltex_ecs::propagate_transforms(world);

    let mut best: Option<(Entity, f32)> = None;
    // 카메라(레이 원점)를 포함하는 AABB는 t=0으로 모든 클릭을 이기므로
    // 최저 우선순위로 강등 — 다른 히트가 하나도 없을 때만 선택된다.
    // (방/레벨 경계 메시나 크게 스케일된 엔티티 안에서도 내부 오브젝트 선택 가능)
    let mut best_enclosing: Option<(Entity, f32)> = None;
    let entities: Vec<Entity> = world.query::<Transform>().map(|(e, _)| e).collect();
    for entity in entities {
        let hit_t: Option<f32> = if let (Some(bounds), Some(wt)) = (
            world.get::<voltex_ecs::Bounds>(entity),
            world.get::<voltex_ecs::WorldTransform>(entity),
        ) {
            // AABB path: transform local Bounds.0 by WorldTransform → world AABB.
            let world_aabb = world_aabb_from_local(&bounds.0, &wt.0);
            if world_aabb.contains_point(cam_pos) {
                if let Some(t) = ray_vs_aabb(&ray, &world_aabb) {
                    best_enclosing = match best_enclosing {
                        Some((_, best_t)) if best_t <= t => best_enclosing,
                        _ => Some((entity, t)),
                    };
                }
                continue;
            }
            ray_vs_aabb(&ray, &world_aabb)
        } else {
            // Sphere fallback for entities without Bounds.
            let t = match world.get::<Transform>(entity) {
                Some(t) => t,
                None => continue,
            };
            let max_scale = t.scale.x.max(t.scale.y).max(t.scale.z).max(1e-6);
            let radius = default_radius * max_scale;
            ray_vs_sphere_t(&ray, t.position, radius)
        };
        if let Some(t) = hit_t {
            best = match best {
                Some((_, best_t)) if best_t <= t => best,
                _ => Some((entity, t)),
            };
        }
    }
    best.or(best_enclosing).map(|(e, _)| e)
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
        let mut world = World::new();
        let vp = Mat4::IDENTITY;
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, Vec3::ZERO, Rect { x: 10.0, y: 10.0, w: 100.0, h: 100.0 }, (5.0, 5.0)), // mouse to upper-left of the rect
            1.0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn pick_returns_none_on_empty_world() {
        let mut world = World::new();
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        // Click dead-centre, but world has no entities.
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 }, (400.0, 300.0)),
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
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)), // centre
            1.0,
        );
        assert_eq!(result, Some(e));
    }

    #[test]
    fn camera_enclosing_aabb_does_not_steal_every_click() {
        // 카메라를 통째로 감싸는 거대 Bounds 엔티티(방 경계 메시 류)는
        // ray 원점이 내부라 t=0으로 항상 이긴다 — 내부의 일반 엔티티가
        // 있으면 그쪽이 선택돼야 하고, 없을 때만 fallback으로 선택된다.
        let mut world = World::new();
        let cam = OrbitCamera::new();

        let room = world.spawn();
        world.add(room, Transform::new());
        world.add(
            room,
            voltex_ecs::Bounds(voltex_math::AABB::new(
                Vec3::new(-100.0, -100.0, -100.0),
                Vec3::new(100.0, 100.0, 100.0),
            )),
        );

        let inner = world.spawn();
        world.add(inner, Transform::from_position(Vec3::ZERO));

        let vp = cam.view_projection(1.0);
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
            1.0,
        );
        assert_eq!(result, Some(inner), "내부 엔티티가 우선");

        // 내부 엔티티가 없으면 감싸는 엔티티라도 선택된다 (no-hit 아님).
        world.despawn(inner);
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
            1.0,
        );
        assert_eq!(result, Some(room), "fallback으로는 선택 가능");
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
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
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
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
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

    #[test]
    fn pick_uses_aabb_when_bounds_present() {
        use voltex_ecs::Bounds;
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::new());
        world.add(e, Bounds(AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))));

        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)), // dead-centre
            0.1,             // sphere fallback radius too small to hit — must rely on AABB
        );
        assert_eq!(result, Some(e));
    }

    #[test]
    fn pick_falls_back_to_sphere_when_no_bounds() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::new());
        // No Bounds — sphere with default_radius must hit centred entity.
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
            1.0,
        );
        assert_eq!(result, Some(e));
    }

    #[test]
    fn pick_returns_nearest_of_two_overlapping_aabbs() {
        use voltex_ecs::Bounds;
        // Both Bounds intersect the centre ray; the one nearer the camera wins.
        let mut world = World::new();
        let cam = OrbitCamera::new();
        let near_e = world.spawn();
        world.add(near_e, Transform::from_position(Vec3::ZERO));
        world.add(near_e, Bounds(AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))));

        let cam_to_target = (cam.target - cam.position()).normalize();
        let far_pos = cam.target + cam_to_target * 5.0;
        let far_e = world.spawn();
        world.add(far_e, Transform::from_position(far_pos));
        world.add(far_e, Bounds(AABB::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))));

        let vp = cam.view_projection(1.0);
        let result = pick_entity_at(
            &mut world,
            &ViewportCursor::new(vp, cam.position(), Rect { x: 0.0, y: 0.0, w: 800.0, h: 800.0 }, (400.0, 400.0)),
            1.0,
        );
        assert_eq!(result, Some(near_e));
    }
}
