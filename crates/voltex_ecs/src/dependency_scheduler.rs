//! Dependency-aware scheduler. Each system declares the components it reads
//! and writes; the scheduler groups non-conflicting systems for parallel
//! execution.

use std::any::TypeId;
use std::collections::HashSet;

/// Describes what components a system reads and writes.
#[derive(Debug, Clone)]
pub struct SystemAccess {
    /// Human-readable system name (for logging/debugging).
    pub name: String,
    /// Component types this system reads.
    pub reads: HashSet<TypeId>,
    /// Component types this system writes (exclusive).
    pub writes: HashSet<TypeId>,
}

impl SystemAccess {
    /// Creates an access spec with the given name and empty read/write sets.
    pub fn new(name: &str) -> Self {
        SystemAccess { name: name.to_string(), reads: HashSet::new(), writes: HashSet::new() }
    }

    /// Builder: declares a read on component `T`.
    pub fn read<T: 'static>(mut self) -> Self { self.reads.insert(TypeId::of::<T>()); self }
    /// Builder: declares a write on component `T`.
    pub fn write<T: 'static>(mut self) -> Self { self.writes.insert(TypeId::of::<T>()); self }

    /// Check if this system conflicts with another (write-read or write-write on same type).
    pub fn conflicts_with(&self, other: &SystemAccess) -> bool {
        // My writes vs their reads/writes
        for w in &self.writes {
            if other.reads.contains(w) || other.writes.contains(w) { return true; }
        }
        // Their writes vs my reads
        for w in &other.writes {
            if self.reads.contains(w) { return true; }
        }
        false
    }
}

/// Dependency scheduler: builds execution order with parallelism groups.
pub struct DependencyScheduler {
    systems: Vec<SystemAccess>,
}

/// A group of systems that can execute in parallel (indices into
/// [`DependencyScheduler::add`]'s insertion order).
pub type ParallelGroup = Vec<usize>;

impl Default for DependencyScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyScheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self { DependencyScheduler { systems: Vec::new() } }

    /// Registers a system. Returns the system's index in the schedule.
    pub fn add(&mut self, access: SystemAccess) -> usize {
        let idx = self.systems.len();
        self.systems.push(access);
        idx
    }

    /// Build execution schedule: groups of non-conflicting systems.
    pub fn build_schedule(&self) -> Vec<ParallelGroup> {
        let mut schedule: Vec<ParallelGroup> = Vec::new();
        let mut placed = vec![false; self.systems.len()];

        loop {
            let mut group = Vec::new();
            for (i, slot) in placed.iter_mut().enumerate() {
                if *slot { continue; }
                // Check if this system conflicts with any already in this group
                let conflicts = group.iter().any(|&j: &usize| self.systems[i].conflicts_with(&self.systems[j]));
                if !conflicts {
                    group.push(i);
                    *slot = true;
                }
            }
            if group.is_empty() { break; }
            schedule.push(group);
        }

        schedule
    }

    /// Number of registered systems.
    pub fn system_count(&self) -> usize { self.systems.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_conflict() {
        let a = SystemAccess::new("A").read::<u32>();
        let b = SystemAccess::new("B").read::<u32>();
        assert!(!a.conflicts_with(&b)); // both read only
    }

    #[test]
    fn test_write_read_conflict() {
        let a = SystemAccess::new("A").write::<u32>();
        let b = SystemAccess::new("B").read::<u32>();
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn test_write_write_conflict() {
        let a = SystemAccess::new("A").write::<u32>();
        let b = SystemAccess::new("B").write::<u32>();
        assert!(a.conflicts_with(&b));
    }

    #[test]
    fn test_schedule_parallel() {
        let mut sched = DependencyScheduler::new();
        sched.add(SystemAccess::new("A").read::<u32>());
        sched.add(SystemAccess::new("B").read::<u32>());
        sched.add(SystemAccess::new("C").read::<f32>());
        let groups = sched.build_schedule();
        assert_eq!(groups.len(), 1); // all can run in parallel
        assert_eq!(groups[0].len(), 3);
    }

    #[test]
    fn test_schedule_sequential() {
        let mut sched = DependencyScheduler::new();
        sched.add(SystemAccess::new("A").write::<u32>());
        sched.add(SystemAccess::new("B").write::<u32>());
        let groups = sched.build_schedule();
        assert_eq!(groups.len(), 2); // must be sequential
    }

    #[test]
    fn test_schedule_mixed() {
        let mut sched = DependencyScheduler::new();
        sched.add(SystemAccess::new("Physics").write::<u32>().read::<f32>());
        sched.add(SystemAccess::new("Render").read::<u32>());
        sched.add(SystemAccess::new("Audio").read::<f64>().write::<f64>());
        let groups = sched.build_schedule();
        // Physics writes u32, Render reads u32 -> conflict
        // Audio is independent -> can run with either
        assert!(groups.len() >= 2);
    }

    #[test]
    fn test_empty_schedule() {
        let sched = DependencyScheduler::new();
        assert!(sched.build_schedule().is_empty());
    }
}
