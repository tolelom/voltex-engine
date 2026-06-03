#![warn(missing_docs)]

//! voltex_runtime — standard bootstrap + main loop + default system wiring.
//!
//! ```ignore
//! use voltex_runtime::{RuntimeApp, RuntimeConfig};
//!
//! fn main() -> voltex_runtime::RuntimeResult<()> {
//!     RuntimeApp::new(
//!         RuntimeConfig::new()
//!             .with_window_title("My Game")
//!             .with_scene_builder(|world, _registry| {
//!                 // populate the World
//!             }),
//!     )?
//!     .run()
//! }
//! ```

pub mod app;
pub mod audio_sync;
pub mod config;
pub mod crash;
pub mod debug_draw;
pub mod error;
pub(crate) mod handler;
pub mod hud;
pub mod log;

pub use app::RuntimeApp;
pub use audio_sync::audio_sync_system;
pub use config::RuntimeConfig;
pub use crash::{crash_filename, format_crash_report, install_panic_hook};
pub use debug_draw::DebugLines;
pub use error::{RuntimeError, RuntimeResult};
pub use hud::Hud;
pub use voltex_asset::Vfs;
