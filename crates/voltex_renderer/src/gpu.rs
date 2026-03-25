use std::sync::Arc;
use winit::window::Window;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
    pub depth_view: wgpu::TextureView,
}

impl GpuContext {
    pub fn new(window: Arc<Window>) -> Self {
        pollster::block_on(Self::new_async(window, wgpu::Features::empty()))
    }

    /// Create a GpuContext requesting additional device features (e.g. ray tracing).
    pub fn new_with_features(window: Arc<Window>, features: wgpu::Features) -> Self {
        pollster::block_on(Self::new_async(window, features))
    }

    async fn new_async(window: Arc<Window>, extra_features: wgpu::Features) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        // When extra features are requested (e.g. ray tracing), use adapter limits
        // so RT-specific limits (max_acceleration_structures_per_shader_stage, etc.) are satisfied.
        let required_limits = if extra_features.is_empty() {
            wgpu::Limits::default()
        } else {
            adapter.limits()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Voltex Device"),
                required_features: extra_features,
                required_limits,
                memory_hints: Default::default(),
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = create_depth_texture(&device, config.width, config.height);

        Self {
            surface,
            device,
            queue,
            config,
            surface_format,
            depth_view,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = create_depth_texture(&self.device, width, height);
        }
    }
}
