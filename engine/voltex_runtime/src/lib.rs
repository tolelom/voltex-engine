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
mod asset_io;
pub mod asset_paths;
pub mod audio_paths;
pub mod audio_sync;
pub mod config;
pub mod crash;
pub mod debug_draw;
pub mod error;
pub(crate) mod handler;
pub mod hud;
pub mod lifetime;
pub mod log;
pub mod time;
mod utc_time;

pub use app::RuntimeApp;
pub use asset_paths::{resolve_asset_paths, resolve_asset_paths_with, AssetPathCache};
pub use audio_paths::{resolve_audio_paths, resolve_audio_paths_with, AudioClipCache, AudioPath};
pub use time::{Time, Timer, TimerMode, Tween};
pub use audio_sync::{audio_sync_system, AudioVoiceTracker};
pub use config::RuntimeConfig;
pub use crash::{
    crash_context_snapshot, crash_filename, format_crash_report,
    format_crash_report_with_context, install_panic_hook, set_crash_fps, set_crash_gpu_adapter,
    set_crash_scene, CrashContext,
};
pub use debug_draw::DebugLines;
pub use lifetime::{update_lifetimes, Lifetime};
pub use error::{RuntimeError, RuntimeResult};
pub use hud::Hud;
pub use voltex_asset::Vfs;
