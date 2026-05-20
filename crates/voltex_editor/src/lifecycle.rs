//! Lifecycle hooks — callbacks fired by EditorSession on Edit↔Play mode
//! changes. Cycles after E2 (script systems, audio, etc.) register callbacks
//! here to attach/detach simulation state without touching EditorSession.

use voltex_ecs::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// Fired right after the editor transitions Edit → Play, with the
    /// snapshot already captured and the mode already set to Play.
    PlayStart,
    /// Fired right before the editor transitions Play → Edit, before
    /// World::clear() so the callback can still read live state if needed.
    PlayStop,
}

/// A registered lifecycle callback. Boxed so EditorSession can hold a
/// heterogeneous list. Closure receives a mutable World reference so the
/// callback may add/remove components (RigidBody, Collider, etc.).
pub type LifecycleCallback = Box<dyn FnMut(LifecycleEvent, &mut World) + 'static>;
