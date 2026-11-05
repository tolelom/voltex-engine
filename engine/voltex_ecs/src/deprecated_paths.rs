//! Old public paths of the modules and items moved to [`crate::experimental`].
//!
//! Deprecated aliases kept so existing code still compiles; nothing in the
//! engine uses them. A module alias (`voltex_ecs::scheduler`, …) warns when the module
//! itself is imported with `use`; type and function aliases warn at every use.

// ── Modules ──────────────────────────────────────────────────────────────

/// Moved to [`experimental::archetype`](crate::experimental::archetype).
#[deprecated(note = "moved to `voltex_ecs::experimental::archetype`; not used by the engine")]
pub mod archetype {
    pub use crate::experimental::archetype::*;
}

/// Moved to [`experimental::dependency_scheduler`](crate::experimental::dependency_scheduler).
#[deprecated(note = "moved to `voltex_ecs::experimental::dependency_scheduler`; not used by the engine")]
pub mod dependency_scheduler {
    pub use crate::experimental::dependency_scheduler::*;
}

/// Moved to [`experimental::scheduler`](crate::experimental::scheduler).
#[deprecated(note = "moved to `voltex_ecs::experimental::scheduler`; not used by the engine")]
pub mod scheduler {
    pub use crate::experimental::scheduler::*;
}

// ── Types that were re-exported at the crate root ────────────────────────

/// Moved to [`experimental::archetype::Archetype`](crate::experimental::archetype::Archetype).
#[deprecated(note = "moved to `voltex_ecs::experimental::archetype::Archetype`; not used by the engine")]
pub type Archetype = crate::experimental::archetype::Archetype;

/// Moved to [`experimental::archetype::ArchetypeRegistry`](crate::experimental::archetype::ArchetypeRegistry).
#[deprecated(note = "moved to `voltex_ecs::experimental::archetype::ArchetypeRegistry`; not used by the engine")]
pub type ArchetypeRegistry = crate::experimental::archetype::ArchetypeRegistry;

/// Moved to [`experimental::archetype::ArchetypeSignature`](crate::experimental::archetype::ArchetypeSignature).
#[deprecated(note = "moved to `voltex_ecs::experimental::archetype::ArchetypeSignature`; not used by the engine")]
pub type ArchetypeSignature = crate::experimental::archetype::ArchetypeSignature;

/// Moved to [`experimental::dependency_scheduler::DependencyScheduler`](crate::experimental::dependency_scheduler::DependencyScheduler).
#[deprecated(note = "moved to `voltex_ecs::experimental::dependency_scheduler::DependencyScheduler`; not used by the engine")]
pub type DependencyScheduler = crate::experimental::dependency_scheduler::DependencyScheduler;

/// Moved to [`experimental::dependency_scheduler::SystemAccess`](crate::experimental::dependency_scheduler::SystemAccess).
#[deprecated(note = "moved to `voltex_ecs::experimental::dependency_scheduler::SystemAccess`; not used by the engine")]
pub type SystemAccess = crate::experimental::dependency_scheduler::SystemAccess;

/// Moved to [`experimental::scheduler::Scheduler`](crate::experimental::scheduler::Scheduler).
#[deprecated(note = "moved to `voltex_ecs::experimental::scheduler::Scheduler`; not used by the engine")]
pub type Scheduler = crate::experimental::scheduler::Scheduler;

// ── Traits that were re-exported at the crate root ───────────────────────
// A trait cannot be aliased, so the re-export carries `#[deprecated]`. rustc
// does not warn on deprecated re-exports; the attribute only marks the docs.

#[deprecated(note = "moved to `voltex_ecs::experimental::scheduler::System`; not used by the engine")]
pub use crate::experimental::scheduler::System;
