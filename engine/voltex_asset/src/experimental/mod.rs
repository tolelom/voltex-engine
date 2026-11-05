//! Experimental asset pieces.
//!
//! Implemented and unit-tested, but not used by the engine runtime yet.
//! APIs here may change or be removed.
//!
//! The runtime loads assets synchronously through [`Vfs`](crate::Vfs) reads,
//! so the background [`loader`] is not on its path. The old paths
//! (`voltex_asset::AssetLoader`, `voltex_asset::loader`, …) still resolve but
//! are deprecated.

pub mod loader;
