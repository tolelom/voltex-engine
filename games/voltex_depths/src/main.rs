mod combat;
mod components;
mod level;

use voltex_runtime::{RuntimeApp, RuntimeConfig, RuntimeResult};

fn main() -> RuntimeResult<()> {
    env_logger::init();
    RuntimeApp::new(
        RuntimeConfig::new()
            .with_window_title("Voltex Depths")
            .with_window_size(1280, 720),
    )?
    .run()
}
