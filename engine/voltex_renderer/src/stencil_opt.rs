//! Stencil-buffer optimisation for tiled/clustered light volume culling.
#![allow(missing_docs)]

/// Stencil state for marking pixels inside a light volume.
pub fn light_volume_stencil_mark() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Always,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                pass_op: wgpu::StencilOperation::Keep,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Always,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                pass_op: wgpu::StencilOperation::Keep,
            },
            read_mask: 0xFF,
            write_mask: 0xFF,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Stencil state for rendering only where stencil != 0 (inside light volume).
pub fn light_volume_stencil_test() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::NotEqual,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::NotEqual,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            read_mask: 0xFF,
            write_mask: 0x00,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_stencil_format() {
        let state = light_volume_stencil_mark();
        assert_eq!(state.format, wgpu::TextureFormat::Depth24PlusStencil8);
        assert_eq!(state.depth_write_enabled, Some(false));
    }

    #[test]
    fn test_test_stencil_compare() {
        let state = light_volume_stencil_test();
        assert_eq!(state.stencil.front.compare, wgpu::CompareFunction::NotEqual);
        assert_eq!(state.stencil.write_mask, 0x00); // read-only
    }
}
