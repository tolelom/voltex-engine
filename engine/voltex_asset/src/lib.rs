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
//! - [`AssetLoader`] — background worker thread that reads bytes from disk and
//!   hands typed assets back to the main thread.
//! - [`FileWatcher`] — mtime polling for hot reload triggers.
//! - [`Vfs`] — read-through abstraction over either a filesystem root or an
//!   embedded `.vpkg` archive (see [`vpkg`]).
//!
//! All errors funnel through [`AssetError`] / [`Result`].

#![warn(missing_docs)]

pub mod error;
pub mod handle;
pub mod storage;
pub mod assets;
pub mod watcher;
pub mod loader;
pub mod vpkg;
pub mod vfs;

pub use error::{AssetError, Result};
pub use handle::Handle;
pub use storage::AssetStorage;
pub use assets::Assets;
pub use watcher::FileWatcher;
pub use loader::{AssetLoader, LoadState};
pub use vpkg::{write_vpkg, VpkgEntry, VpkgReader, VPKG_MAGIC, VPKG_VERSION};
pub use vfs::{Vfs, VpkgReaderOwned};
