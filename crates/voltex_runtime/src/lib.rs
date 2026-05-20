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
pub mod error;
pub(crate) mod handler;

pub use app::RuntimeApp;
pub use config::RuntimeConfig;
pub use error::{RuntimeError, RuntimeResult};
