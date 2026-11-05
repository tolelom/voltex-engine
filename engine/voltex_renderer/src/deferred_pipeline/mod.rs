//! Deferred rendering pipelines — G-buffer + lighting + SSGI + RT shadow + bloom + tonemap.
#![allow(missing_docs)]

// 파일명 `*_pipeline` 은 같은 이름의 최상위 리소스 모듈(crate::bloom, crate::ssgi …)과
// 구분하려는 것 — 여기에는 파이프라인/바인드 그룹 레이아웃 생성 함수만 있다.
mod bloom_pipeline;
mod gbuffer_pipeline;
mod lighting_pipeline;
mod rt_shadow_pipeline;
mod ssgi_pipeline;
mod tonemap_pipeline;

pub use bloom_pipeline::{
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
};
pub use gbuffer_pipeline::{create_gbuffer_pipeline, gbuffer_camera_bind_group_layout};
pub use lighting_pipeline::{
    create_lighting_pipeline, lighting_gbuffer_bind_group_layout,
    lighting_lights_bind_group_layout, lighting_shadow_bind_group_layout,
};
pub use rt_shadow_pipeline::{
    create_rt_shadow_pipeline, rt_shadow_data_bind_group_layout,
    rt_shadow_gbuffer_bind_group_layout,
};
pub use ssgi_pipeline::{create_ssgi_pipeline, ssgi_data_bind_group_layout, ssgi_gbuffer_bind_group_layout};
pub use tonemap_pipeline::{create_tonemap_pipeline, tonemap_bind_group_layout};
