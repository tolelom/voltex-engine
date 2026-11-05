//! Temporal accumulation compute pass: blends the current frame into a
//! history texture (`blend_factor`, default 0.1).
#![allow(missing_docs)]

use crate::compute::{ComputeContext, ScreenOutput};

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TemporalParams {
    blend_factor: f32,
    _pad: [f32; 3],
}

pub struct TemporalAccumulation {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    pub blend_factor: f32,
}

impl TemporalAccumulation {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Temporal Accumulation Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("temporal_accum.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Temporal Accum BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Temporal Accum PL"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Temporal Accum Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Temporal Params"),
            size: std::mem::size_of::<TemporalParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        TemporalAccumulation {
            pipeline,
            bind_group_layout,
            params_buffer,
            blend_factor: 0.1,
        }
    }

    /// 입력 텍스처를 읽어 `output` 에 결과를 쓰는 compute 패스를 기록한다.
    pub fn encode(
        &self,
        gpu: &mut ComputeContext<'_>,
        current_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
        output: ScreenOutput<'_>,
    ) {
        let params = TemporalParams {
            blend_factor: self.blend_factor,
            _pad: [0.0; 3],
        };
        gpu.queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Temporal Accum BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(current_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(output.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut cpass = gpu.encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Temporal Accum Pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        crate::compute::dispatch_screen(&mut cpass, output.width, output.height);
    }

    /// 옛 인자 나열형. [`Self::encode`] 를 쓴다.
    #[deprecated(note = "use `encode` (ComputeContext + ScreenOutput)")]
    // reason: 역호환용 옛 시그니처를 그대로 둔다.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        current_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        self.encode(
            &mut ComputeContext {
                device,
                queue,
                encoder,
            },
            current_view,
            history_view,
            ScreenOutput {
                view: output_view,
                width,
                height,
            },
        );
    }
}

/// Pure CPU temporal blend (for testing).
pub fn temporal_blend(history: f32, current: f32, factor: f32) -> f32 {
    history * (1.0 - factor) + current * factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_factor_zero() {
        let result = temporal_blend(0.5, 1.0, 0.0);
        assert!((result - 0.5).abs() < 1e-6); // all history
    }

    #[test]
    fn test_blend_factor_one() {
        let result = temporal_blend(0.5, 1.0, 1.0);
        assert!((result - 1.0).abs() < 1e-6); // all current
    }

    #[test]
    fn test_blend_factor_half() {
        let result = temporal_blend(0.0, 1.0, 0.5);
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_blend_default() {
        // Default factor = 0.1: 90% history + 10% current
        let result = temporal_blend(0.8, 0.2, 0.1);
        let expected = 0.8 * 0.9 + 0.2 * 0.1;
        assert!((result - expected).abs() < 1e-6);
    }
}
