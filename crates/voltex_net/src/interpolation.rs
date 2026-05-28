//! Client-side snapshot interpolation buffer. Smooths server updates by
//! interpolating between the two snapshots bracketing the render time.

use std::collections::VecDeque;

use crate::snapshot::{EntityState, Snapshot};

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
            max_snapshots: 32,
        }
    }

    /// Push a new snapshot with its server timestamp.
    pub fn push(&mut self, server_time: f64, snapshot: Snapshot) {
        self.snapshots.push_back((server_time, snapshot));
        // Evict old snapshots beyond the buffer limit
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_f32x3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

/// Interpolate between two angles (in radians) along the shortest arc.
///
/// `EntityState::rotation` holds Euler angles in radians (matching
/// `voltex_ecs::Transform`). A naive component-wise linear lerp of angles
/// takes the long way around the circle whenever the two values straddle the
/// ±π wrap boundary (e.g. ~6.11 rad → ~0.17 rad would pass through π instead
/// of crossing 0). Wrapping the difference into `[-π, π]` first guarantees the
/// interpolation follows the shortest rotation. When the pair does not wrap,
/// this reduces to plain linear interpolation.
fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::PI;
    let diff = (b - a + PI).rem_euclid(2.0 * PI) - PI;
    a + diff * t
}

fn lerp_angles3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    [
        lerp_angle(a[0], b[0], t),
        lerp_angle(a[1], b[1], t),
        lerp_angle(a[2], b[2], t),
    ]
}

fn lerp_entity(a: &EntityState, b: &EntityState, t: f32) -> EntityState {
    EntityState {
        id: a.id,
        position: lerp_f32x3(&a.position, &b.position, t),
        // Rotation is Euler radians — interpolate along the shortest arc so we
        // do not spin the long way around the ±π wrap boundary.
        rotation: lerp_angles3(&a.rotation, &b.rotation, t),
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
                rotation: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        }
    }

    fn make_full_snapshot(
        tick: u32,
        position: [f32; 3],
        rotation: [f32; 3],
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
    fn test_extrapolation_returns_latest() {
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_snapshot(0, 0.0));
        buf.push(1.0, make_snapshot(10, 10.0));

        // render_time beyond all snapshots
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
    fn test_rotation_interpolates_shortest_arc_across_wrap() {
        use std::f32::consts::PI;
        // 6.1 rad (~349.5°) -> 0.17 rad (~9.7°). The shortest arc crosses 0,
        // landing near 0 / 2π at the midpoint. A naive lerp would land near
        // (6.1 + 0.17) / 2 = 3.135 rad (~180°), the long way around.
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_full_snapshot(0, [0.0; 3], [6.1, 6.1, 6.1], [0.0; 3]));
        buf.push(1.0, make_full_snapshot(10, [0.0; 3], [0.17, 0.17, 0.17], [0.0; 3]));

        let result = buf.interpolate(0.5).expect("should interpolate");
        let rot = result.entities[0].rotation;

        // diff = wrap(0.17 - 6.1) = wrap(-5.93) -> ~0.353; midpoint = 6.1 + 0.353*0.5 = ~6.2765
        // Normalize to the circle and confirm it is near 0 / 2π, NOT near PI.
        for (i, &r) in rot.iter().enumerate() {
            let norm = r.rem_euclid(2.0 * PI); // in [0, 2π)
            let dist_to_zero = norm.min(2.0 * PI - norm); // angular distance to 0
            assert!(
                dist_to_zero < 0.05,
                "component {i}: expected midpoint near 0/2π (shortest arc), got {r} (norm {norm}, dist {dist_to_zero})",
            );
            // And explicitly NOT the naive long-way result near PI.
            assert!(
                (norm - PI).abs() > 1.0,
                "component {i}: result {norm} is near PI — naive lerp bug present",
            );
        }
    }

    #[test]
    fn test_rotation_no_wrap_reduces_to_linear() {
        // 0.0 -> 1.0 rad does not straddle the wrap boundary, so shortest-arc
        // interpolation must match plain linear lerp: midpoint = 0.5.
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(0.0, make_full_snapshot(0, [0.0; 3], [0.0, 0.0, 0.0], [0.0; 3]));
        buf.push(1.0, make_full_snapshot(10, [0.0; 3], [1.0, 1.0, 1.0], [0.0; 3]));

        let result = buf.interpolate(0.5).expect("should interpolate");
        for (i, &r) in result.entities[0].rotation.iter().enumerate() {
            assert!(
                (r - 0.5).abs() < 1e-5,
                "component {i}: expected linear midpoint 0.5, got {r}",
            );
        }
    }

    #[test]
    fn test_position_and_velocity_interpolate_all_components() {
        // Vary all three position components and all three velocity components
        // to guard against only X being interpolated.
        let mut buf = InterpolationBuffer::new(0.1);
        buf.push(
            0.0,
            make_full_snapshot(0, [0.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 0.0]),
        );
        buf.push(
            1.0,
            make_full_snapshot(10, [10.0, 20.0, 30.0], [0.0; 3], [4.0, 8.0, 12.0]),
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
