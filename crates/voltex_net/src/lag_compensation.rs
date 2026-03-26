use std::collections::VecDeque;

/// A timestamped snapshot of entity positions for lag compensation.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub tick: u64,
    pub timestamp: f32,   // seconds
    pub positions: Vec<([f32; 3], u32)>,  // (position, entity_id)
}

/// Stores recent world state history for server-side lag compensation.
pub struct LagCompensation {
    history: VecDeque<HistoryEntry>,
    pub max_history_ms: f32,  // max history duration (e.g., 200ms)
}

impl LagCompensation {
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
            (Some(b), Some(a)) => {
                let t = if (a.timestamp - b.timestamp).abs() > 1e-6 {
                    (timestamp - b.timestamp) / (a.timestamp - b.timestamp)
                } else { 0.0 };
                // Interpolate positions
                let mut result = Vec::new();
                for (i, (pos_b, id)) in b.positions.iter().enumerate() {
                    if let Some((pos_a, _)) = a.positions.get(i) {
                        let lerped = [
                            pos_b[0] + (pos_a[0] - pos_b[0]) * t,
                            pos_b[1] + (pos_a[1] - pos_b[1]) * t,
                            pos_b[2] + (pos_a[2] - pos_b[2]) * t,
                        ];
                        result.push((lerped, *id));
                    }
                }
                Some(result)
            }
            _ => self.rewind(timestamp).map(|e| e.positions.clone()),
        }
    }

    pub fn history_len(&self) -> usize { self.history.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut lc = LagCompensation::new(100.0); // 100ms
        for i in 0..20 {
            lc.record(HistoryEntry { tick: i, timestamp: i as f32 * 0.016, positions: vec![] });
        }
        // At t=0.304, cutoff = 0.304 - 0.1 = 0.204
        // Entries before t=0.204 should be pruned
        assert!(lc.history_len() < 20);
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
