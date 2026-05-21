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
pub mod config;
pub mod crash;
pub mod error;
pub(crate) mod handler;

pub use app::RuntimeApp;
pub use config::RuntimeConfig;
pub use crash::{crash_filename, format_crash_report, install_panic_hook};
pub use error::{RuntimeError, RuntimeResult};
pub use voltex_asset::Vfs;
