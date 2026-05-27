//! Surface-less wgpu device/queue for offscreen rendering.
//!
//! Unlike [`voltex_renderer::GpuContext`], this requests an adapter with no
//! compatible surface, so it works in headless / windowless contexts (CI,
//! verification harnesses). It renders into textures, never a swapchain.

/// A windowless GPU device + queue.
pub struct HeadlessGpu {
    /// The wgpu device used for resource creation.
    pub device: wgpu::Device,
    /// Submission queue paired with [`device`](Self::device).
    pub queue: wgpu::Queue,
    /// Human-readable adapter name (backend + GPU), for diagnostics.
    pub adapter_name: String,
}

impl HeadlessGpu {
    /// Acquire a headless device. Returns `Err` with a diagnostic string when
    /// no GPU adapter is available (e.g. a CI box with no GPU), so callers can
    /// skip gracefully rather than panic.
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("no GPU adapter available (headless): {e}"))?;

        let info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Voltex Headless Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("request_device failed: {e}"))?;

        Ok(Self { device, queue, adapter_name })
    }
}
