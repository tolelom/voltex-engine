//! 화면 공간 compute 패스(SSR·TAA·DoF·RT AO 등)가 공유하는 디스패치 규칙.

/// 화면 공간 compute 셰이더의 workgroup 한 변 길이.
/// 각 .wgsl 의 `@workgroup_size(16, 16)` 과 같아야 한다.
pub(crate) const SCREEN_WORKGROUP_SIZE: u32 = 16;

/// `width`×`height` 픽셀을 빠짐없이 덮도록 16×16 workgroup 을 디스패치한다.
pub(crate) fn dispatch_screen(cpass: &mut wgpu::ComputePass<'_>, width: u32, height: u32) {
    cpass.dispatch_workgroups(
        width.div_ceil(SCREEN_WORKGROUP_SIZE),
        height.div_ceil(SCREEN_WORKGROUP_SIZE),
        1,
    );
}
