//! Experimental renderer pieces.
//!
//! Implemented and unit-tested, but not used by the engine renderer yet.
//! APIs here may change or be removed.
//!
//! The old top-level paths (`voltex_renderer::light_probes`,
//! `voltex_renderer::ForwardPass`, …) still resolve but are deprecated.

// ── 디퍼드 / 포워드 / 인스턴싱 ──────────────────────────────────────────
pub mod forward_pass;
pub mod gbuffer_compress;
pub mod instancing;
pub mod light_volumes;
pub mod stencil_opt;

// ── 라이트 프로브 ───────────────────────────────────────────────────────
pub mod light_probes;

// ── 레이 트레이싱 ──────────────────────────────────────────────────────
pub mod blas_update;
pub mod rt_fallback;
pub mod soft_rt_shadow;

// ── 화면 공간 효과 / 포스트 프로세스 ────────────────────────────────────
pub mod bilateral_bloom;
pub mod bilateral_blur;
pub mod half_res_ssgi;
pub mod temporal_accum;
