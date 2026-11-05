//! Old public paths of the modules and items moved to [`crate::experimental`].
//!
//! Deprecated aliases kept so existing code still compiles; nothing in the
//! engine uses them. A module alias (`voltex_editor::floating_panel`, …) warns when the module
//! itself is imported with `use`; type and function aliases warn at every use.

// ── Modules ──────────────────────────────────────────────────────────────

/// Moved to [`experimental::floating_panel`](crate::experimental::floating_panel).
#[deprecated(note = "moved to `voltex_editor::experimental::floating_panel`; not used by the engine")]
pub mod floating_panel {
    pub use crate::experimental::floating_panel::*;
}

// ── Types that were re-exported at the crate root ────────────────────────

/// Moved to [`experimental::floating_panel::FloatingPanel`](crate::experimental::floating_panel::FloatingPanel).
#[deprecated(note = "moved to `voltex_editor::experimental::floating_panel::FloatingPanel`; not used by the engine")]
pub type FloatingPanel = crate::experimental::floating_panel::FloatingPanel;

/// Moved to [`experimental::floating_panel::FloatingPanelManager`](crate::experimental::floating_panel::FloatingPanelManager).
#[deprecated(note = "moved to `voltex_editor::experimental::floating_panel::FloatingPanelManager`; not used by the engine")]
pub type FloatingPanelManager = crate::experimental::floating_panel::FloatingPanelManager;
