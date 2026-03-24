use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window as WinitWindow, WindowAttributes};

pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
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

pub struct VoltexWindow {
    pub handle: Arc<WinitWindow>,
    pub vsync: bool,
}

impl VoltexWindow {
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

    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.handle.inner_size();
        (size.width, size.height)
    }

    pub fn request_redraw(&self) {
        self.handle.request_redraw();
    }
}
