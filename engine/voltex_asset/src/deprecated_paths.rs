//! Old public paths of the modules and items moved to [`crate::experimental`].
//!
//! Deprecated aliases kept so existing code still compiles; nothing in the
//! engine uses them. A module alias (`voltex_asset::loader`, …) warns when the module
//! itself is imported with `use`; type and function aliases warn at every use.

// ── Modules ──────────────────────────────────────────────────────────────

/// Moved to [`experimental::loader`](crate::experimental::loader).
#[deprecated(note = "moved to `voltex_asset::experimental::loader`; not used by the engine")]
pub mod loader {
    pub use crate::experimental::loader::*;
}

// ── Types that were re-exported at the crate root ────────────────────────

/// Moved to [`experimental::loader::AssetLoader`](crate::experimental::loader::AssetLoader).
#[deprecated(note = "moved to `voltex_asset::experimental::loader::AssetLoader`; not used by the engine")]
pub type AssetLoader = crate::experimental::loader::AssetLoader;

/// Moved to [`experimental::loader::LoadState`](crate::experimental::loader::LoadState).
#[deprecated(note = "moved to `voltex_asset::experimental::loader::LoadState`; not used by the engine")]
pub type LoadState = crate::experimental::loader::LoadState;
