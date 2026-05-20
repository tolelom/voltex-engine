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
