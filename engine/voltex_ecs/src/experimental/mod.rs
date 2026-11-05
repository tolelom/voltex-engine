//! Experimental ECS pieces.
//!
//! Implemented and unit-tested, but not used by the engine runtime yet.
//! APIs here may change or be removed.
//!
//! [`World`](crate::World) stores components in
//! [`SparseSet`](crate::SparseSet)s and `RuntimeApp` keeps its own system
//! lists, so neither the archetype store nor the schedulers below are on the
//! engine's path. The old top-level paths (`voltex_ecs::Scheduler`,
//! `voltex_ecs::archetype`, …) still resolve but are deprecated.

pub mod archetype;
pub mod dependency_scheduler;
pub mod scheduler;
