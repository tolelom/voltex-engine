//! Deferred rendering pipelines — G-buffer + lighting + SSGI + RT shadow + bloom + tonemap.
#![allow(missing_docs)]

mod gbuffer;
mod lighting;
mod ssgi;
mod rt_shadow;
mod bloom;
mod tonemap;

pub use gbuffer::{gbuffer_camera_bind_group_layout, create_gbuffer_pipeline};
pub use lighting::{
    lighting_gbuffer_bind_group_layout,
    lighting_lights_bind_group_layout,
    lighting_shadow_bind_group_layout,
    create_lighting_pipeline,
};
pub use ssgi::{
    ssgi_gbuffer_bind_group_layout,
    ssgi_data_bind_group_layout,
    create_ssgi_pipeline,
};
pub use rt_shadow::{
    rt_shadow_gbuffer_bind_group_layout,
    rt_shadow_data_bind_group_layout,
    create_rt_shadow_pipeline,
};
pub use bloom::{
    bloom_bind_group_layout,
    create_bloom_downsample_pipeline,
    create_bloom_upsample_pipeline,
};
pub use tonemap::{tonemap_bind_group_layout, create_tonemap_pipeline};
