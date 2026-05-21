#![allow(missing_docs)]

/// Check if a wgpu adapter supports features needed for RT.
pub struct RtCapabilities {
    pub supports_compute: bool,
    pub supports_storage_textures: bool,
    pub supports_timestamp_query: bool,
}

impl RtCapabilities {
    /// Evaluate capabilities (simplified — real check would use adapter.features()).
    pub fn evaluate(max_storage_buffers: u32, max_compute_workgroup_size: u32) -> Self {
        RtCapabilities {
            supports_compute: max_compute_workgroup_size >= 256,
            supports_storage_textures: max_storage_buffers >= 4,
            supports_timestamp_query: false, // opt-in feature
        }
    }

    pub fn can_use_rt(&self) -> bool { self.supports_compute && self.supports_storage_textures }
}

/// Fallback rendering mode when RT is not available.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    Full,           // All RT features
    NoRtShadows,    // Use shadow maps instead
    NoRtReflections,// Use SSR instead
    Minimal,        // Shadow maps + no reflections
}

pub fn select_render_mode(caps: &RtCapabilities) -> RenderMode {
    if caps.can_use_rt() { RenderMode::Full }
    else if caps.supports_compute { RenderMode::NoRtShadows }
    else { RenderMode::Minimal }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_full_caps() {
        let c = RtCapabilities::evaluate(8, 256);
        assert!(c.can_use_rt());
        assert_eq!(select_render_mode(&c), RenderMode::Full);
    }
    #[test]
    fn test_no_compute() {
        let c = RtCapabilities::evaluate(8, 64);
        assert!(!c.can_use_rt());
        assert_eq!(select_render_mode(&c), RenderMode::Minimal);
    }
    #[test]
    fn test_limited_storage() {
        let c = RtCapabilities::evaluate(2, 256);
        assert!(!c.can_use_rt());
        assert_eq!(select_render_mode(&c), RenderMode::NoRtShadows);
    }
}
