//! Experimental editor pieces.
//!
//! Implemented and unit-tested, but not used by the editor (`editor_demo`)
//! yet. APIs here may change or be removed.
//!
//! The editor lays panels out with [`DockTree`](crate::DockTree) only; nothing
//! creates floating panels. The old paths (`voltex_editor::FloatingPanel`,
//! `voltex_editor::floating_panel`, …) still resolve but are deprecated.

pub mod floating_panel;
