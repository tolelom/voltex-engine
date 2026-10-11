//! GPU context — owns the wgpu surface/device/queue + the depth attachment.

use std::sync::Arc;
use winit::window::Window;

use crate::error::{RendererError, Result};

/// Depth attachment format the renderer uses everywhere.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// wgpu surface + device + queue + surface config + depth attachment, all
/// constructed against a single window.
pub struct GpuContext {
    /// Window-backed surface to present into.
    pub surface: wgpu::Surface<'static>,
    /// The wgpu device used for resource creation.
    pub device: wgpu::Device,
    /// Submission queue paired with [`device`](Self::device).
    pub queue: wgpu::Queue,
    /// Current surface configuration (size/format/present mode).
    pub config: wgpu::SurfaceConfiguration,
    /// Surface texture format (sRGB-preferred when supported).
    pub surface_format: wgpu::TextureFormat,
    /// Adapter the device was created on (name, backend, driver) — for
    /// logs and crash reports.
    pub adapter_info: wgpu::AdapterInfo,
    /// Depth attachment view, sized to the surface.
    pub depth_view: wgpu::TextureView,
}

impl GpuContext {
    /// Creates a default GpuContext (no extra features). Blocks on adapter +
    /// device acquisition via pollster.
    ///
    /// Returns [`RendererError::GpuInit`] instead of panicking when no surface,
    /// adapter, or device can be acquired (headless CI, unsupported GPU), so
    /// callers can decide how to handle GPU absence.
    pub fn new(window: Arc<Window>) -> Result<Self> {
        pollster::block_on(Self::new_async(window, wgpu::Features::empty()))
    }

    /// Create a GpuContext requesting additional device features (e.g. ray tracing).
    ///
    /// Returns [`RendererError::GpuInit`] on failure — see [`new`](Self::new).
    pub fn new_with_features(window: Arc<Window>, features: wgpu::Features) -> Result<Self> {
        pollster::block_on(Self::new_async(window, features))
    }

    async fn new_async(window: Arc<Window>, extra_features: wgpu::Features) -> Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| RendererError::GpuInit(format!("failed to create surface: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| RendererError::GpuInit(format!("no suitable GPU adapter found: {e}")))?;

        // When extra features are requested (e.g. ray tracing), use adapter limits
        // so RT-specific limits (max_acceleration_structures_per_shader_stage, etc.) are satisfied.
        let required_limits = if extra_features.is_empty() {
            wgpu::Limits::default()
        } else {
            adapter.limits()
        };

        let experimental = if extra_features.is_empty() {
            wgpu::ExperimentalFeatures::disabled()
        } else {
            // Safety: we acknowledge experimental features may have bugs
            unsafe { wgpu::ExperimentalFeatures::enabled() }
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Voltex Device"),
                required_features: extra_features,
                required_limits,
                memory_hints: Default::default(),
                experimental_features: experimental,
                ..Default::default()
            })
            .await
            .map_err(|e| RendererError::GpuInit(format!("failed to create device: {e}")))?;

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

        Ok(Self {
            surface,
            device,
            queue,
            config,
            surface_format,
            adapter_info: adapter.get_info(),
            depth_view,
        })
    }

    /// Reconfigures the surface for a new window size and rebuilds the
    /// depth attachment. No-op when either dimension is zero.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = create_depth_texture(&self.device, width, height);
        }
    }
}
