//! Client-side snapshot interpolation buffer. Smooths server updates by
//! interpolating between the two snapshots bracketing the render time.

use std::collections::VecDeque;

use crate::snapshot::{EntityState, Snapshot};
use voltex_math::easing::lerp;
use voltex_math::Quat;

/// Buffer capacity used by [`InterpolationBuffer::new`].
const DEFAULT_MAX_SNAPSHOTS: usize = 32;

/// Buffers recent snapshots and interpolates between them for smooth rendering.
pub struct InterpolationBuffer {
    snapshots: VecDeque<(f64, Snapshot)>,
    /// Render delay behind the latest server time (seconds).
    interp_delay: f64,
    /// Maximum number of snapshots to keep in the buffer.
    max_snapshots: usize,
}

impl InterpolationBuffer {
    /// Create a new interpolation buffer with the given delay in seconds.
    pub fn new(interp_delay: f64) -> Self {
        InterpolationBuffer {
            snapshots: VecDeque::new(),
            interp_delay,
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
        }
    }

    /// Push a new snapshot with its server timestamp.
    ///
    /// Snapshots may arrive out of order (UDP); they are kept sorted by
    /// `server_time` so bracketing in [`interpolate`](Self::interpolate)
    /// stays correct. A second snapshot with the same time replaces the first.
    pub fn push(&mut self, server_time: f64, snapshot: Snapshot) {
        let idx = self.snapshots.partition_point(|(t, _)| *t < server_time);
        match self.snapshots.get_mut(idx) {
            Some(slot) if slot.0 == server_time => slot.1 = snapshot,
            _ => self.snapshots.insert(idx, (server_time, snapshot)),
        }
        // Evict the oldest (by server time) beyond the buffer limit
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.pop_front();
        }
    }

    /// Interpolate to produce a snapshot for the given render_time.
    ///
    /// The render_time should be `current_server_time - interp_delay`.
    /// Returns None if there are fewer than 2 snapshots or render_time
    /// is before all buffered snapshots.
    pub fn interpolate(&self, render_time: f64) -> Option<Snapshot> {
        if self.snapshots.len() < 2 {
            return None;
        }

        // Find two bracketing snapshots: the last one <= render_time and the first one > render_time
        let mut before = None;
        let mut after = None;

        for (i, (time, _)) in self.snapshots.iter().enumerate() {
            if *time <= render_time {
                before = Some(i);
            } else {
                after = Some(i);
                break;
            }
        }

        match (before, after) {
            (Some(b), Some(a)) => {
                let (t0, snap0) = &self.snapshots[b];
                let (t1, snap1) = &self.snapshots[a];
                let dt = t1 - t0;
                if dt <= 0.0 {
                    return Some(snap0.clone());
                }
                let alpha = ((render_time - t0) / dt).clamp(0.0, 1.0) as f32;
                Some(lerp_snapshots(snap0, snap1, alpha))
            }
            (Some(b), None) => {
                // render_time is beyond all snapshots — return the latest
                Some(self.snapshots[b].1.clone())
            }
            _ => None,
        }
    }

    /// Get the interpolation delay.
    pub fn delay(&self) -> f64 {
        self.interp_delay
    }
}

fn lerp_f32x3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

/// Spherical linear interpolation between two unit-quaternion wire reps.
/// Picks the shorter arc and stays NaN-safe via `Quat::slerp`.
fn slerp_quat_wire(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
    Quat::slerp(Quat::from_array(*a), Quat::from_array(*b), t).to_array()
}

fn lerp_entity(a: &EntityState, b: &EntityState, t: f32) -> EntityState {
    EntityState {
        id: a.id,
        position: lerp_f32x3(&a.position, &b.position, t),
        // Rotation is a unit quaternion since E3b — slerp picks the shorter
        // arc, sidestepping the prior componentwise-Euler long-way bug.
        rotation: slerp_quat_wire(&a.rotation, &b.rotation, t),
        velocity: lerp_f32x3(&a.velocity, &b.velocity, t),
    }
}

/// Linearly interpolate between two snapshots.
/// Entities are matched by id. Entities only in one snapshot are included as-is.
fn lerp_snapshots(a: &Snapshot, b: &Snapshot, t: f32) -> Snapshot {
    use std::collections::HashMap;

    let a_map: HashMap<u32, &EntityState> = a.entities.iter().map(|e| (e.id, e)).collect();
    let b_map: HashMap<u32, &EntityState> = b.entities.iter().map(|e| (e.id, e)).collect();

    let mut entities = Vec::new();

    // Interpolate matched entities, include a-only entities
    for ea in &a.entities {
        if let Some(eb) = b_map.get(&ea.id) {
            entities.push(lerp_entity(ea, eb, t));
        } else {
            entities.push(ea.clone());
        }
    }

    // Include b-only entities
    for eb in &b.entities {
        if !a_map.contains_key(&eb.id) {
            entities.push(eb.clone());
        }
    }

    // Interpolate tick
    let tick = (a.tick as f64 + (b.tick as f64 - a.tick as f64) * t as f64) as u32;

    Snapshot { tick, entities }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::EntityState;

    fn make_snapshot(tick: u32, x: f32) -> Snapshot {
        Snapshot {
            tick,
            entities: vec![EntityState {
                id: 1,
                position: [x, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        }
    }

    fn make_full_snapshot(
        tick: u32,
        position: [f32; 3],
        rotation: [f32; 4],
        velocity: [f32; 3],
    ) -> Snapshot {
        Snapshot {
            tick,
            entities: vec![EntityState { id: 1, position, rotation, velocity }],
        }
    }

    #[test]
    fn test_exact_match_at_snapshot_time() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(0.1, make_snapshot(1, 10.0));

        let result = buf.interpolate(0.0).expect("should interpolate");
        assert_eq!(result.entities[0].position[0], 0.0);
    }

    #[test]
    fn test_midpoint_interpolation() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(1.0, make_snapshot(10, 10.0));

        let result = buf.interpolate(0.5).expect("should interpolate");
        let x = result.entities[0].position[0];
        assert!(
            (x - 5.0).abs() < 0.001,
            "Expected ~5.0 at midpoint, got {}",
            x
        );
    }

    #[test]
    fn test_interpolation_at_quarter() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(1.0, make_snapshot(10, 100.0));

        let result = buf.interpolate(0.25).unwrap();
        let x = result.entities[0].position[0];
        assert!(
            (x - 25.0).abs() < 0.01,
            "Expected ~25.0 at 0.25, got {}",
            x
        );
    }

    #[test]
    fn test_beyond_latest_clamps_to_latest() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(1.0, make_snapshot(10, 10.0));

        // render_time beyond all snapshots: the buffer does NOT extrapolate —
        // it clamps and returns the latest snapshot unchanged.
        let result = buf.interpolate(2.0).expect("should return latest");
        assert_eq!(result.entities[0].position[0], 10.0);
    }

    #[test]
    fn test_too_few_snapshots_returns_none() {
        let mut buf = InterpolationBuffer::new(0.1);
        assert!(buf.interpolate(0.0).is_none());

        buf.push(0.0, make_snapshot(0, 0.0));
        assert!(buf.interpolate(0.0).is_none());
    }

    #[test]
    fn test_render_time_before_all_snapshots() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(1.0, make_snapshot(10, 10.0));
        buf.push(2.0, make_snapshot(20, 20.0));

        // render_time before the first snapshot
        let result = buf.interpolate(0.0);
        assert!(result.is_none());
    }

    #[test]
    fn rotation_slerps_shorter_arc_when_dot_negative() {
        use voltex_math::Vec3;
        // Two unit quats representing the same physical rotation (q and -q):
        // a 180° flip in sign should not produce a 360° rotation on slerp.
        let qa = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.0); // identity
        let qb_pos = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.4);
        let qb_neg = Quat::new(-qb_pos.x, -qb_pos.y, -qb_pos.z, -qb_pos.w);

        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_full_snapshot(0, [0.0; 3], [qa.x, qa.y, qa.z, qa.w], [0.0; 3]));
        buf.push(1.0, make_full_snapshot(10, [0.0; 3], [qb_neg.x, qb_neg.y, qb_neg.z, qb_neg.w], [0.0; 3]));

        let result = buf.interpolate(0.5).expect("should interpolate");
        let r = result.entities[0].rotation;
        let q = Quat::new(r[0], r[1], r[2], r[3]);
        // Midpoint should be near the half-rotation of the shorter arc
        // (0.2 rad around Y), not 180° around an arbitrary axis.
        let expected = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.2);
        // Either sign is acceptable (q ≡ -q for rotations).
        let matches = (q.x - expected.x).abs() < 1e-3
            && (q.y - expected.y).abs() < 1e-3
            && (q.z - expected.z).abs() < 1e-3
            && (q.w - expected.w).abs() < 1e-3;
        let matches_neg = (q.x + expected.x).abs() < 1e-3
            && (q.y + expected.y).abs() < 1e-3
            && (q.z + expected.z).abs() < 1e-3
            && (q.w + expected.w).abs() < 1e-3;
        assert!(matches || matches_neg, "slerp midpoint {q:?} ≠ ±{expected:?}");
    }

    #[test]
    fn rotation_slerp_at_endpoints_returns_endpoints() {
        let qa = [0.0, 0.0, 0.0, 1.0];
        let qb = [0.1, 0.2, 0.3, 0.9];
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_full_snapshot(0, [0.0; 3], qa, [0.0; 3]));
        buf.push(1.0, make_full_snapshot(10, [0.0; 3], qb, [0.0; 3]));

        let at_a = buf.interpolate(0.0).expect("interp");
        for (i, &expected) in qa.iter().enumerate() {
            assert!((at_a.entities[0].rotation[i] - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn test_position_and_velocity_interpolate_all_components() {
        // Vary all three position components and all three velocity components
        // to guard against only X being interpolated.
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(
            0.0,
            make_full_snapshot(0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0]),
        );
        buf.push(
            1.0,
            make_full_snapshot(
                10,
                [10.0, 20.0, 30.0],
                [0.0, 0.0, 0.0, 1.0],
                [4.0, 8.0, 12.0],
            ),
        );

        let result = buf.interpolate(0.5).expect("should interpolate");
        let e = &result.entities[0];

        assert!((e.position[0] - 5.0).abs() < 1e-4, "pos.x got {}", e.position[0]);
        assert!((e.position[1] - 10.0).abs() < 1e-4, "pos.y got {}", e.position[1]);
        assert!((e.position[2] - 15.0).abs() < 1e-4, "pos.z got {}", e.position[2]);

        assert!((e.velocity[0] - 2.0).abs() < 1e-4, "vel.x got {}", e.velocity[0]);
        assert!((e.velocity[1] - 4.0).abs() < 1e-4, "vel.y got {}", e.velocity[1]);
        assert!((e.velocity[2] - 6.0).abs() < 1e-4, "vel.z got {}", e.velocity[2]);
    }

    #[test]
    fn test_multiple_snapshots_picks_correct_bracket() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(1.0, make_snapshot(1, 10.0));
        buf.push(2.0, make_snapshot(2, 20.0));

        // Should interpolate between snapshot at t=1 and t=2
        let result = buf.interpolate(1.5).unwrap();
        let x = result.entities[0].position[0];
        assert!(
            (x - 15.0).abs() < 0.01,
            "Expected ~15.0, got {}",
            x
        );
    }
}
