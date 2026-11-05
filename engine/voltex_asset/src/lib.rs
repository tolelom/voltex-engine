//! # voltex_asset
//!
//! Asset management primitives shared across the engine:
//!
//! - [`Handle<T>`] — generation-checked id for asset references; safe across
//!   slot reuse.
//! - [`AssetStorage<T>`] — single-type slab with ref counting + `replace_in_place`
//!   for hot reload.
//! - [`Assets`] — type-erased aggregate over many `AssetStorage<T>` keyed by
//!   `TypeId`.
//! - [`FileWatcher`] — mtime polling for hot reload triggers.
//! - [`Vfs`] — read-through abstraction over either a filesystem root or an
//!   embedded `.vpkg` archive (see [`vpkg`]).
//! - [`experimental`] — background `AssetLoader`; implemented but not used by
//!   the engine runtime.
//!
//! All errors funnel through [`AssetError`] / [`Result`].

#![warn(missing_docs)]

pub mod error;
pub mod handle;
pub mod storage;
pub mod assets;
pub mod watcher;
pub mod vpkg;
pub mod vfs;
// Not used by the engine runtime yet
pub mod experimental;

pub use error::{AssetError, Result};
pub use handle::Handle;
pub use storage::AssetStorage;
pub use assets::Assets;
pub use watcher::FileWatcher;
pub use vpkg::{write_vpkg, VpkgEntry, VpkgReader, VPKG_MAGIC, VPKG_VERSION};
pub use vfs::{vfs_key, StaticVpkgReader, Vfs};
#[allow(deprecated)]
pub use vfs::VpkgReaderOwned;

// Old paths of modules/items moved to `experimental` (deprecated). Nothing in
// the engine uses them.
mod deprecated_paths;
#[allow(deprecated)]
pub use deprecated_paths::*;
