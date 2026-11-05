//! 옛 공개 경로 — [`crate::experimental`] 로 옮긴 모듈·항목.
//!
//! 기존 코드가 계속 컴파일되도록 남겨 둔 deprecated 별칭이다. 엔진 안에서는
//! 쓰지 않는다. 모듈 별칭(`voltex_renderer::light_probes` 등)은 `use` 로 모듈
//! 자체를 가져올 때 경고가 나고, 타입·함수 별칭은 쓰는 곳마다 경고가 난다.

// ── 모듈 ────────────────────────────────────────────────────────────────

/// Moved to [`experimental::bilateral_bloom`](crate::experimental::bilateral_bloom).
#[deprecated(note = "moved to `voltex_renderer::experimental::bilateral_bloom`; not used by the engine")]
pub mod bilateral_bloom {
    pub use crate::experimental::bilateral_bloom::*;
}

/// Moved to [`experimental::bilateral_blur`](crate::experimental::bilateral_blur).
#[deprecated(note = "moved to `voltex_renderer::experimental::bilateral_blur`; not used by the engine")]
pub mod bilateral_blur {
    pub use crate::experimental::bilateral_blur::*;
}

/// Moved to [`experimental::blas_update`](crate::experimental::blas_update).
#[deprecated(note = "moved to `voltex_renderer::experimental::blas_update`; not used by the engine")]
pub mod blas_update {
    pub use crate::experimental::blas_update::*;
}

/// Moved to [`experimental::forward_pass`](crate::experimental::forward_pass).
#[deprecated(note = "moved to `voltex_renderer::experimental::forward_pass`; not used by the engine")]
pub mod forward_pass {
    pub use crate::experimental::forward_pass::*;
}

/// Moved to [`experimental::gbuffer_compress`](crate::experimental::gbuffer_compress).
#[deprecated(note = "moved to `voltex_renderer::experimental::gbuffer_compress`; not used by the engine")]
pub mod gbuffer_compress {
    pub use crate::experimental::gbuffer_compress::*;
}

/// Moved to [`experimental::half_res_ssgi`](crate::experimental::half_res_ssgi).
#[deprecated(note = "moved to `voltex_renderer::experimental::half_res_ssgi`; not used by the engine")]
pub mod half_res_ssgi {
    pub use crate::experimental::half_res_ssgi::*;
}

/// Moved to [`experimental::instancing`](crate::experimental::instancing).
#[deprecated(note = "moved to `voltex_renderer::experimental::instancing`; not used by the engine")]
pub mod instancing {
    pub use crate::experimental::instancing::*;
}

/// Moved to [`experimental::light_probes`](crate::experimental::light_probes).
#[deprecated(note = "moved to `voltex_renderer::experimental::light_probes`; not used by the engine")]
pub mod light_probes {
    pub use crate::experimental::light_probes::*;
}

/// Moved to [`experimental::light_volumes`](crate::experimental::light_volumes).
#[deprecated(note = "moved to `voltex_renderer::experimental::light_volumes`; not used by the engine")]
pub mod light_volumes {
    pub use crate::experimental::light_volumes::*;
}

/// Moved to [`experimental::rt_fallback`](crate::experimental::rt_fallback).
#[deprecated(note = "moved to `voltex_renderer::experimental::rt_fallback`; not used by the engine")]
pub mod rt_fallback {
    pub use crate::experimental::rt_fallback::*;
}

/// Moved to [`experimental::soft_rt_shadow`](crate::experimental::soft_rt_shadow).
#[deprecated(note = "moved to `voltex_renderer::experimental::soft_rt_shadow`; not used by the engine")]
pub mod soft_rt_shadow {
    pub use crate::experimental::soft_rt_shadow::*;
}

/// Moved to [`experimental::stencil_opt`](crate::experimental::stencil_opt).
#[deprecated(note = "moved to `voltex_renderer::experimental::stencil_opt`; not used by the engine")]
pub mod stencil_opt {
    pub use crate::experimental::stencil_opt::*;
}

/// Moved to [`experimental::temporal_accum`](crate::experimental::temporal_accum).
#[deprecated(note = "moved to `voltex_renderer::experimental::temporal_accum`; not used by the engine")]
pub mod temporal_accum {
    pub use crate::experimental::temporal_accum::*;
}

// ── 루트 재export 였던 타입 ─────────────────────────────────────────────

/// Moved to [`experimental::bilateral_blur::BilateralBlur`](crate::experimental::bilateral_blur::BilateralBlur).
#[deprecated(note = "moved to `voltex_renderer::experimental::bilateral_blur::BilateralBlur`; not used by the engine")]
pub type BilateralBlur = crate::experimental::bilateral_blur::BilateralBlur;

/// Moved to [`experimental::blas_update::BlasTracker`](crate::experimental::blas_update::BlasTracker).
#[deprecated(note = "moved to `voltex_renderer::experimental::blas_update::BlasTracker`; not used by the engine")]
pub type BlasTracker = crate::experimental::blas_update::BlasTracker;

/// Moved to [`experimental::forward_pass::ForwardPass`](crate::experimental::forward_pass::ForwardPass).
#[deprecated(note = "moved to `voltex_renderer::experimental::forward_pass::ForwardPass`; not used by the engine")]
pub type ForwardPass = crate::experimental::forward_pass::ForwardPass;

/// Moved to [`experimental::instancing::InstanceBuffer`](crate::experimental::instancing::InstanceBuffer).
#[deprecated(note = "moved to `voltex_renderer::experimental::instancing::InstanceBuffer`; not used by the engine")]
pub type InstanceBuffer = crate::experimental::instancing::InstanceBuffer;

/// Moved to [`experimental::instancing::InstanceData`](crate::experimental::instancing::InstanceData).
#[deprecated(note = "moved to `voltex_renderer::experimental::instancing::InstanceData`; not used by the engine")]
pub type InstanceData = crate::experimental::instancing::InstanceData;

/// Moved to [`experimental::light_probes::LightProbe`](crate::experimental::light_probes::LightProbe).
#[deprecated(note = "moved to `voltex_renderer::experimental::light_probes::LightProbe`; not used by the engine")]
pub type LightProbe = crate::experimental::light_probes::LightProbe;

/// Moved to [`experimental::light_probes::LightProbeGrid`](crate::experimental::light_probes::LightProbeGrid).
#[deprecated(note = "moved to `voltex_renderer::experimental::light_probes::LightProbeGrid`; not used by the engine")]
pub type LightProbeGrid = crate::experimental::light_probes::LightProbeGrid;

/// Moved to [`experimental::light_volumes::LightVolume`](crate::experimental::light_volumes::LightVolume).
#[deprecated(note = "moved to `voltex_renderer::experimental::light_volumes::LightVolume`; not used by the engine")]
pub type LightVolume = crate::experimental::light_volumes::LightVolume;

/// Moved to [`experimental::rt_fallback::RenderMode`](crate::experimental::rt_fallback::RenderMode).
#[deprecated(note = "moved to `voltex_renderer::experimental::rt_fallback::RenderMode`; not used by the engine")]
pub type RenderMode = crate::experimental::rt_fallback::RenderMode;

/// Moved to [`experimental::rt_fallback::RtCapabilities`](crate::experimental::rt_fallback::RtCapabilities).
#[deprecated(note = "moved to `voltex_renderer::experimental::rt_fallback::RtCapabilities`; not used by the engine")]
pub type RtCapabilities = crate::experimental::rt_fallback::RtCapabilities;

/// Moved to [`experimental::temporal_accum::TemporalAccumulation`](crate::experimental::temporal_accum::TemporalAccumulation).
#[deprecated(note = "moved to `voltex_renderer::experimental::temporal_accum::TemporalAccumulation`; not used by the engine")]
pub type TemporalAccumulation = crate::experimental::temporal_accum::TemporalAccumulation;

// ── 루트 재export 였던 함수 ─────────────────────────────────────────────

/// Moved to [`experimental::forward_pass::sort_transparent_back_to_front`](crate::experimental::forward_pass::sort_transparent_back_to_front).
#[deprecated(
    note = "moved to `voltex_renderer::experimental::forward_pass::sort_transparent_back_to_front`; not used by the engine"
)]
#[inline]
pub fn sort_transparent_back_to_front(items: &mut [(usize, [f32; 3])], camera_pos: [f32; 3]) {
    crate::experimental::forward_pass::sort_transparent_back_to_front(items, camera_pos)
}

/// Moved to [`experimental::instancing::create_instanced_pipeline`](crate::experimental::instancing::create_instanced_pipeline).
#[deprecated(
    note = "moved to `voltex_renderer::experimental::instancing::create_instanced_pipeline`; not used by the engine"
)]
#[inline]
pub fn create_instanced_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    crate::experimental::instancing::create_instanced_pipeline(device, format, camera_light_layout, texture_layout)
}
