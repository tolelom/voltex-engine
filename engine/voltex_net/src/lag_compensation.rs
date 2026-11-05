//! Server-side history buffer for lag compensation. Records past entity
//! positions so the server can "rewind" the world to the client's view when
//! validating hit registration.

use std::collections::VecDeque;

/// History entries closer together than this (seconds) are not interpolated.
const MIN_TIMESTAMP_GAP: f32 = 1e-6;

/// A timestamped snapshot of entity positions for lag compensation.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Server tick this snapshot was taken on.
    pub tick: u64,
    /// Wall-clock timestamp in seconds (server local).
    pub timestamp: f32,
    /// `(position, entity_id)` pairs at this tick.
    pub positions: Vec<([f32; 3], u32)>,
}

/// Stores recent world state history for server-side lag compensation.
pub struct LagCompensation {
    history: VecDeque<HistoryEntry>,
    /// Maximum history duration in milliseconds (older entries are pruned).
    pub max_history_ms: f32,
}

impl LagCompensation {
    /// Creates an empty buffer with the given retention window in milliseconds.
    pub fn new(max_history_ms: f32) -> Self {
        LagCompensation { history: VecDeque::new(), max_history_ms }
    }

    /// Record current world state.
    pub fn record(&mut self, entry: HistoryEntry) {
        self.history.push_back(entry);
        // Prune old entries
        let cutoff = self.history.back().map(|e| e.timestamp - self.max_history_ms / 1000.0).unwrap_or(0.0);
        while let Some(front) = self.history.front() {
            if front.timestamp < cutoff {
                self.history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Find the closest history entry to the given timestamp.
    pub fn rewind(&self, timestamp: f32) -> Option<&HistoryEntry> {
        let mut best: Option<&HistoryEntry> = None;
        let mut best_diff = f32::MAX;
        for entry in &self.history {
            let diff = (entry.timestamp - timestamp).abs();
            if diff < best_diff {
                best_diff = diff;
                best = Some(entry);
            }
        }
        best
    }

    /// Interpolate between two closest entries at the given timestamp.
    pub fn rewind_interpolated(&self, timestamp: f32) -> Option<Vec<([f32; 3], u32)>> {
        if self.history.len() < 2 { return self.rewind(timestamp).map(|e| e.positions.clone()); }

        // Find the two entries bracketing the timestamp
        let mut before: Option<&HistoryEntry> = None;
        let mut after: Option<&HistoryEntry> = None;
        for entry in &self.history {
            if entry.timestamp <= timestamp {
                before = Some(entry);
            } else {
                after = Some(entry);
                break;
            }
        }

        match (before, after) {
            (Some(before), Some(after)) => {
                let t = if (after.timestamp - before.timestamp).abs() > MIN_TIMESTAMP_GAP {
                    (timestamp - before.timestamp) / (after.timestamp - before.timestamp)
                } else { 0.0 };
                // Pair entities by id — list order changes whenever something
                // spawns or despawns between ticks. An entity missing from the
                // later tick keeps its earlier position.
                let later: std::collections::HashMap<u32, [f32; 3]> =
                    after.positions.iter().map(|(position, id)| (*id, *position)).collect();
                let result = before
                    .positions
                    .iter()
                    .map(|(pos_before, id)| match later.get(id) {
                        Some(pos_after) => (
                            [
                                pos_before[0] + (pos_after[0] - pos_before[0]) * t,
                                pos_before[1] + (pos_after[1] - pos_before[1]) * t,
                                pos_before[2] + (pos_after[2] - pos_before[2]) * t,
                            ],
                            *id,
                        ),
                        None => (*pos_before, *id),
                    })
                    .collect();
                Some(result)
            }
            _ => self.rewind(timestamp).map(|e| e.positions.clone()),
        }
    }

    /// Number of history entries currently retained.
    pub fn history_len(&self) -> usize { self.history.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_pairs_entities_by_id_not_list_position() {
        // Entity 2 spawned between the two ticks and was listed first; the
        // index-based pairing lerped entity 1 toward entity 2's position.
        let mut lc = LagCompensation::new(1000.0);
        lc.record(HistoryEntry { tick: 1, timestamp: 0.0, positions: vec![([0.0, 0.0, 0.0], 1)] });
        lc.record(HistoryEntry {
            tick: 2,
            timestamp: 0.1,
            positions: vec![([100.0, 0.0, 0.0], 2), ([10.0, 0.0, 0.0], 1)],
        });
        let at = lc.rewind_interpolated(0.05).unwrap();
        let one = at.iter().find(|(_, id)| *id == 1).expect("entity 1 present");
        assert!((one.0[0] - 5.0).abs() < 1e-4, "entity 1 should be halfway to x=10, got {:?}", one.0);
    }

    #[test]
    fn interpolation_keeps_entities_missing_from_the_later_tick() {
        // Despawned before the later tick: report its last known position
        // rather than silently dropping it from hit validation.
        let mut lc = LagCompensation::new(1000.0);
        lc.record(HistoryEntry { tick: 1, timestamp: 0.0, positions: vec![([1.0, 0.0, 0.0], 1), ([3.0, 0.0, 0.0], 2)] });
        lc.record(HistoryEntry { tick: 2, timestamp: 0.1, positions: vec![([2.0, 0.0, 0.0], 2)] });
        let at = lc.rewind_interpolated(0.05).unwrap();
        assert_eq!(at.len(), 2, "{at:?}");
        let one = at.iter().find(|(_, id)| *id == 1).unwrap();
        assert_eq!(one.0, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_record_and_rewind() {
        let mut lc = LagCompensation::new(200.0);
        lc.record(HistoryEntry { tick: 1, timestamp: 0.0, positions: vec![([1.0, 0.0, 0.0], 0)] });
        lc.record(HistoryEntry { tick: 2, timestamp: 0.016, positions: vec![([2.0, 0.0, 0.0], 0)] });
        let entry = lc.rewind(0.0).unwrap();
        assert_eq!(entry.tick, 1);
    }

    #[test]
    fn test_prune_old() {
        let mut lc = LagCompensation::new(100.0); // 100ms retention window
        for i in 0..20 {
            lc.record(HistoryEntry { tick: i, timestamp: i as f32 * 0.016, positions: vec![] });
        }
        // Newest entry: tick 19 @ t = 19 * 0.016 = 0.304.
        // cutoff = 0.304 - 0.1 = 0.204. record() prunes entries with
        // timestamp < cutoff from the front. timestamps are i * 0.016:
        //   tick 12 @ 0.192 < 0.204  -> pruned
        //   tick 13 @ 0.208 >= 0.204 -> retained
        // So ticks 13..=19 survive: exactly 7 entries.
        assert_eq!(lc.history_len(), 7, "expected exactly 7 retained entries after pruning");

        // Oldest retained entry is tick 13; the just-recorded newest is tick 19.
        assert_eq!(lc.history.front().unwrap().tick, 13, "oldest retained tick should be 13");
        assert_eq!(lc.history.back().unwrap().tick, 19, "newest tick should be retained");

        // The previously-oldest tick (0) must be gone.
        assert!(
            lc.history.iter().all(|e| e.tick >= 13),
            "all retained entries should have tick >= 13 (oldest pruned)",
        );
    }

    #[test]
    fn test_rewind_closest() {
        let mut lc = LagCompensation::new(500.0);
        lc.record(HistoryEntry { tick: 1, timestamp: 0.0, positions: vec![] });
        lc.record(HistoryEntry { tick: 2, timestamp: 0.1, positions: vec![] });
        lc.record(HistoryEntry { tick: 3, timestamp: 0.2, positions: vec![] });
        let entry = lc.rewind(0.09).unwrap();
        assert_eq!(entry.tick, 2); // closest to 0.1
    }

    #[test]
    fn test_rewind_interpolated() {
        let mut lc = LagCompensation::new(500.0);
        lc.record(HistoryEntry { tick: 1, timestamp: 0.0, positions: vec![([0.0, 0.0, 0.0], 0)] });
        lc.record(HistoryEntry { tick: 2, timestamp: 0.1, positions: vec![([10.0, 0.0, 0.0], 0)] });
        let interp = lc.rewind_interpolated(0.05).unwrap();
        assert!((interp[0].0[0] - 5.0).abs() < 0.1); // midpoint
    }

    #[test]
    fn test_empty_history() {
        let lc = LagCompensation::new(200.0);
        assert!(lc.rewind(0.0).is_none());
    }
}
