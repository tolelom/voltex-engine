#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};

pub const SSGI_OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const SSGI_KERNEL_SIZE: usize = 64;

const NOISE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const NOISE_DIM: u32 = 4; // 4x4 = 16 noise samples

/// Uniform buffer for the SSGI pass.
///
/// projection and view are column-major 4x4 matrices stored as flat [f32; 16].
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SsgiUniform {
    pub projection: [f32; 16],
    pub view: [f32; 16],
    pub radius: f32,
    pub bias: f32,
    pub intensity: f32,
    pub indirect_strength: f32,
}

impl Default for SsgiUniform {
    fn default() -> Self {
        // Identity matrices for projection and view.
        #[rustfmt::skip]
        let identity = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0_f32,
        ];
        Self {
            projection: identity,
            view: identity,
            radius: 0.5,
            bias: 0.025,
            intensity: 1.5,
            indirect_strength: 0.5,
        }
    }
}

/// GPU resources needed for one SSGI pass.
pub struct SsgiResources {
    pub output_view: wgpu::TextureView,
    pub kernel_buffer: wgpu::Buffer,
    pub noise_view: wgpu::TextureView,
    pub noise_sampler: wgpu::Sampler,
    pub uniform_buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl SsgiResources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
        let output_view = create_ssgi_output(device, width, height);

        // Hemisphere kernel buffer.
        let kernel_data = generate_kernel(SSGI_KERNEL_SIZE);
        // Flatten [f32; 4] array to bytes.
        let kernel_bytes: Vec<u8> = kernel_data
            .iter()
            .flat_map(|v| v.iter().flat_map(|f| f.to_ne_bytes()).collect::<Vec<u8>>())
            .collect();
        let kernel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSGI Kernel Buffer"),
            size: kernel_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&kernel_buffer, 0, &kernel_bytes);

        // Noise texture.
        let (noise_view, noise_sampler) = create_noise_texture(device, queue);

        // Uniform buffer with default values.
        let uniform = SsgiUniform::default();
        let uniform_bytes = bytemuck::bytes_of(&uniform);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSGI Uniform Buffer"),
            size: uniform_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buffer, 0, uniform_bytes);

        Self {
            output_view,
            kernel_buffer,
            noise_view,
            noise_sampler,
            uniform_buffer,
            width,
            height,
        }
    }

    /// Recreate only the output texture when the window is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.output_view = create_ssgi_output(device, width, height);
        self.width = width;
        self.height = height;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Simple deterministic hash → [0, 1) float.
pub fn pseudo_random(seed: u32) -> f32 {
    let mut x = seed;
    x = x.wrapping_add(0x9e3779b9);
    x = x.wrapping_mul(0x6c62272e);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}

/// Generate `count` hemisphere samples (z >= 0) biased towards center.
/// Each sample is stored as [x, y, z, 0.0] (w=0 unused / padding).
pub fn generate_kernel(count: usize) -> Vec<[f32; 4]> {
    let mut samples = Vec::with_capacity(count);
    let mut seed = 0u32;

    for i in 0..count {
        // Random point on hemisphere: z in [0, 1], xy distributed on disk.
        let r1 = pseudo_random(seed);
        seed = seed.wrapping_add(1);
        let r2 = pseudo_random(seed);
        seed = seed.wrapping_add(1);

        // Spherical coordinates: phi in [0, 2π), cos_theta in [0, 1] (upper hemisphere).
        let phi = r1 * 2.0 * std::f32::consts::PI;
        let cos_theta = r2;
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();

        let x = phi.cos() * sin_theta;
        let y = phi.sin() * sin_theta;
        let z = cos_theta; // z >= 0 (hemisphere)

        // Accelerating interpolation — scale samples closer to origin.
        let scale = (i + 1) as f32 / count as f32;
        let scale = lerp(0.1, 1.0, scale * scale);

        samples.push([x * scale, y * scale, z * scale, 0.0]);
    }

    samples
}

/// Generate 16 (4×4) random rotation vectors for SSGI noise.
/// Each entry is [cos(angle), sin(angle), 0.0, 0.0] (xy rotation in tangent space).
pub fn generate_noise_data() -> Vec<[f32; 4]> {
    let mut data = Vec::with_capacity(16);
    let mut seed = 1337u32;

    for _ in 0..16 {
        let angle = pseudo_random(seed) * 2.0 * std::f32::consts::PI;
        seed = seed.wrapping_add(7);
        // Normalize: (cos, sin) is already unit length.
        data.push([angle.cos(), angle.sin(), 0.0, 0.0]);
    }

    data
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// Create the SSGI output render target (RGBA16Float, screen-sized).
pub fn create_ssgi_output(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSGI Output Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SSGI_OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Create the 4×4 SSGI noise texture (Rgba32Float) and its sampler.
pub fn create_noise_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let noise_data = generate_noise_data();
    // Flatten [f32; 4] → bytes.
    let noise_bytes: Vec<u8> = noise_data
        .iter()
        .flat_map(|v| v.iter().flat_map(|f| f.to_ne_bytes()).collect::<Vec<u8>>())
        .collect();

    let extent = wgpu::Extent3d {
        width: NOISE_DIM,
        height: NOISE_DIM,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSGI Noise Texture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: NOISE_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Each texel is 4 × 4 bytes = 16 bytes (Rgba32Float).
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &noise_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(NOISE_DIM * 16), // 4 components × 4 bytes × NOISE_DIM
            rows_per_image: Some(NOISE_DIM),
        },
        extent,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Wrap/repeat so the noise tiles across the screen.
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("SSGI Noise Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    (view, sampler)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_hemisphere() {
        let kernel = generate_kernel(SSGI_KERNEL_SIZE);
        assert_eq!(kernel.len(), SSGI_KERNEL_SIZE);
        for sample in &kernel {
            // z component must be >= 0 (upper hemisphere).
            assert!(
                sample[2] >= 0.0,
                "kernel sample z={} is negative",
                sample[2]
            );
            // w component is always 0 (padding).
            assert_eq!(sample[3], 0.0);
            // Sample must be within the unit sphere (scale <= 1).
            let len =
                (sample[0] * sample[0] + sample[1] * sample[1] + sample[2] * sample[2]).sqrt();
            assert!(len <= 1.0 + 1e-5, "sample length {} > 1.0", len);
        }
    }

    #[test]
    fn test_noise_data() {
        let noise = generate_noise_data();
        assert_eq!(noise.len(), 16);
        for entry in &noise {
            // xy should form a unit vector (cos²+sin²=1).
            let len = (entry[0] * entry[0] + entry[1] * entry[1]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "noise xy length {} != 1.0", len);
            // z and w are always 0.
            assert_eq!(entry[2], 0.0);
            assert_eq!(entry[3], 0.0);
        }
    }

    #[test]
    fn test_pseudo_random_in_unit_range() {
        // x is a u32, divided by u32::MAX, so the result is always in [0, 1].
        for seed in 0u32..1000 {
            let v = pseudo_random(seed);
            assert!(
                (0.0..=1.0).contains(&v),
                "pseudo_random({seed}) = {v} out of [0, 1]"
            );
        }
    }

    #[test]
    fn test_pseudo_random_deterministic() {
        // Same seed always yields the same value (pure function).
        for seed in [0u32, 1, 42, 1337, u32::MAX] {
            assert_eq!(pseudo_random(seed), pseudo_random(seed));
        }
    }

    #[test]
    fn test_pseudo_random_differs_across_seeds() {
        // Distinct seeds should hash to distinct outputs (no trivial collisions
        // for small consecutive seeds).
        let a = pseudo_random(0);
        let b = pseudo_random(1);
        let c = pseudo_random(2);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn test_noise_data_length_and_components() {
        // generate_noise_data() builds exactly NOISE_DIM*NOISE_DIM = 16 entries,
        // each [cos(angle), sin(angle), 0.0, 0.0].
        let noise = generate_noise_data();
        assert_eq!(noise.len(), 16);
        for entry in &noise {
            // All components finite.
            for &c in entry {
                assert!(c.is_finite(), "noise component {c} is not finite");
            }
            // cos/sin are bounded to [-1, 1].
            assert!(
                (-1.0..=1.0).contains(&entry[0]),
                "cos out of range: {}",
                entry[0]
            );
            assert!(
                (-1.0..=1.0).contains(&entry[1]),
                "sin out of range: {}",
                entry[1]
            );
            // (cos, sin) is a unit vector.
            let len = (entry[0] * entry[0] + entry[1] * entry[1]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "xy length {len} != 1.0");
            // z and w are padding (always 0).
            assert_eq!(entry[2], 0.0);
            assert_eq!(entry[3], 0.0);
        }
    }

    #[test]
    fn test_uniform_defaults() {
        let u = SsgiUniform::default();
        assert!((u.radius - 0.5).abs() < f32::EPSILON);
        assert!((u.bias - 0.025).abs() < f32::EPSILON);
        assert!((u.intensity - 1.5).abs() < f32::EPSILON);
        assert!((u.indirect_strength - 0.5).abs() < f32::EPSILON);
        // Identity diagonal elements.
        assert_eq!(u.projection[0], 1.0);
        assert_eq!(u.projection[5], 1.0);
        assert_eq!(u.projection[10], 1.0);
        assert_eq!(u.projection[15], 1.0);
    }
}
