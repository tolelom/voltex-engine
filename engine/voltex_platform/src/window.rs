//! winit window wrapper.

use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window as WinitWindow, WindowAttributes};

/// Window creation parameters. Used by [`VoltexWindow::new`] and stored
/// on `RuntimeConfig` (in `voltex_runtime`) for the lazy window creation
/// that happens on winit's first `resumed` event.
pub struct WindowConfig {
    /// OS window title bar text.
    pub title: String,
    /// Initial inner width in logical pixels.
    pub width: u32,
    /// Initial inner height in logical pixels.
    pub height: u32,
    /// Open as borderless fullscreen on the primary monitor.
    pub fullscreen: bool,
    /// Requested vsync. Only copied onto [`VoltexWindow::vsync`] — no
    /// renderer reads it yet (`GpuContext` picks the surface's first
    /// supported present mode on its own).
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Voltex Engine".to_string(),
            width: 1280,
            height: 720,
            fullscreen: false,
            vsync: true,
        }
    }
}

/// Thin owning wrapper around a `winit::window::Window`. Held in an
/// `Arc` so the renderer can grab a copy for its surface bind.
pub struct VoltexWindow {
    /// Underlying winit window handle.
    pub handle: Arc<WinitWindow>,
    /// `vsync` flag copied from the [`WindowConfig`] used at construction.
    pub vsync: bool,
}

impl VoltexWindow {
    /// Construct using winit's `ActiveEventLoop` (only valid inside
    /// an event callback — typically `ApplicationHandler::resumed`).
    /// Panics if window creation fails.
    pub fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Self {
        let mut attrs = WindowAttributes::default()
            .with_title(&config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(config.width, config.height));

        if config.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let window = event_loop.create_window(attrs).expect("Failed to create window");
        Self {
            handle: Arc::new(window),
            vsync: config.vsync,
        }
    }

    /// Current inner (client-area) size in physical pixels, as
    /// `(width, height)`.
    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.handle.inner_size();
        (size.width, size.height)
    }

    /// Ask winit to dispatch a `RedrawRequested` event next iteration.
    pub fn request_redraw(&self) {
        self.handle.request_redraw();
    }
}
