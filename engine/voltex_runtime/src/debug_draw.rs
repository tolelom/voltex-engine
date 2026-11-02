//! [`DebugLines`] — a per-frame builder for immediate-mode debug gizmos
//! (collider bounds, nav paths, raycasts, velocity vectors). Games accumulate
//! colored segments into it from a [`RuntimeApp::set_debug_draw_fn`] callback;
//! the runtime uploads them to `BasicRenderer::set_debug_lines` each frame.

use voltex_math::Vec3;
use voltex_renderer::LineVertex;

/// A growable list of world-space colored line segments for one frame. Helper
/// methods cover the common gizmo shapes; everything decomposes into line
/// segments (2 vertices each).
#[derive(Default, Debug, Clone)]
pub struct DebugLines {
    verts: Vec<LineVertex>,
}

impl DebugLines {
    /// An empty builder.
    pub fn new() -> Self {
        Self { verts: Vec::new() }
    }

    /// Drop all queued segments (the runtime calls this each frame before the
    /// game's `set_debug_draw_fn` callback).
    pub fn clear(&mut self) {
        self.verts.clear();
    }

    /// Number of queued segments.
    pub fn segment_count(&self) -> usize {
        self.verts.len() / 2
    }

    /// The flat endpoint list (2 per segment) for upload.
    pub fn vertices(&self) -> &[LineVertex] {
        &self.verts
    }

    /// A single segment from `a` to `b` in `color` (linear RGB).
    pub fn line(&mut self, a: Vec3, b: Vec3, color: [f32; 3]) {
        self.verts.push(LineVertex::new([a.x, a.y, a.z], color));
        self.verts.push(LineVertex::new([b.x, b.y, b.z], color));
    }

    /// A ray from `origin` along `dir` (not required to be unit) of length
    /// `len`.
    pub fn ray(&mut self, origin: Vec3, dir: Vec3, len: f32, color: [f32; 3]) {
        let d = dir.normalize_or_zero();
        self.line(origin, origin + d * len, color);
    }

    /// The 12 edges of an axis-aligned box spanning `min`..`max`.
    pub fn aabb(&mut self, min: Vec3, max: Vec3, color: [f32; 3]) {
        let c = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        // bottom ring, top ring, vertical pillars
        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];
        for (a, b) in edges {
            self.line(c[a], c[b], color);
        }
    }

    /// Three axis-aligned segments crossing at `center`, each `2 * half`
    /// long — a quick position marker.
    pub fn cross(&mut self, center: Vec3, half: f32, color: [f32; 3]) {
        self.line(
            center - Vec3::new(half, 0.0, 0.0),
            center + Vec3::new(half, 0.0, 0.0),
            color,
        );
        self.line(
            center - Vec3::new(0.0, half, 0.0),
            center + Vec3::new(0.0, half, 0.0),
            color,
        );
        self.line(
            center - Vec3::new(0.0, 0.0, half),
            center + Vec3::new(0.0, 0.0, half),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_adds_two_vertices() {
        let mut d = DebugLines::new();
        d.line(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), [1.0, 0.0, 0.0]);
        assert_eq!(d.segment_count(), 1);
        assert_eq!(d.vertices().len(), 2);
        assert_eq!(d.vertices()[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(d.vertices()[0].color, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn aabb_has_twelve_edges() {
        let mut d = DebugLines::new();
        d.aabb(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0), [0.0, 1.0, 0.0]);
        assert_eq!(d.segment_count(), 12, "a box has 12 edges");
    }

    #[test]
    fn cross_has_three_segments_and_clear_empties() {
        let mut d = DebugLines::new();
        d.cross(Vec3::ZERO, 0.5, [1.0, 1.0, 1.0]);
        assert_eq!(d.segment_count(), 3);
        d.clear();
        assert_eq!(d.segment_count(), 0);
    }

    #[test]
    fn ray_points_along_normalized_dir() {
        let mut d = DebugLines::new();
        d.ray(Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0), 2.0, [1.0, 0.0, 0.0]);
        // normalized (0,5,0) * 2 = (0,2,0)
        assert_eq!(d.vertices()[1].position, [0.0, 2.0, 0.0]);
    }
}
