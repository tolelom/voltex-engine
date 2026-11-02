//! Deferred rendering pipelines — G-buffer + lighting + SSGI + RT shadow + bloom + tonemap.
#![allow(missing_docs)]

mod bloom;
mod gbuffer;
mod lighting;
mod rt_shadow;
mod ssgi;
mod tonemap;

pub use bloom::{
    bloom_bind_group_layout, create_bloom_downsample_pipeline, create_bloom_upsample_pipeline,
};
pub use gbuffer::{create_gbuffer_pipeline, gbuffer_camera_bind_group_layout};
pub use lighting::{
    create_lighting_pipeline, lighting_gbuffer_bind_group_layout,
    lighting_lights_bind_group_layout, lighting_shadow_bind_group_layout,
};
pub use rt_shadow::{
    create_rt_shadow_pipeline, rt_shadow_data_bind_group_layout,
    rt_shadow_gbuffer_bind_group_layout,
};
pub use ssgi::{create_ssgi_pipeline, ssgi_data_bind_group_layout, ssgi_gbuffer_bind_group_layout};
pub use tonemap::{create_tonemap_pipeline, tonemap_bind_group_layout};
