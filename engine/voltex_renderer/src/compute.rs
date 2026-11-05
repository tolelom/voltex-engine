//! 화면 공간 compute 패스(SSR·TAA·DoF·RT AO 등)가 공유하는 디스패치 규칙과 인자 묶음.

/// 화면 공간 compute 셰이더의 workgroup 한 변 길이.
/// 각 .wgsl 의 `@workgroup_size(16, 16)` 과 같아야 한다.
pub(crate) const SCREEN_WORKGROUP_SIZE: u32 = 16;

/// compute 패스 하나를 기록하는 데 쓰는 GPU 핸들 — 바인드 그룹 생성(`device`),
/// 파라미터 업로드(`queue`), 패스 기록(`encoder`). 여러 패스에 이어서 넘겨도 된다.
pub struct ComputeContext<'a> {
    /// 바인드 그룹을 만든다.
    pub device: &'a wgpu::Device,
    /// 패스 파라미터 uniform 을 올린다.
    pub queue: &'a wgpu::Queue,
    /// compute 패스가 기록되는 곳.
    pub encoder: &'a mut wgpu::CommandEncoder,
}

/// compute 패스가 결과를 쓰는 storage 텍스처와 그 픽셀 크기.
/// 크기가 디스패치 범위(`width`×`height` 를 덮는 workgroup 수)를 정한다.
#[derive(Clone, Copy)]
pub struct ScreenOutput<'a> {
    /// 결과를 쓰는 storage 텍스처 뷰.
    pub view: &'a wgpu::TextureView,
    /// 출력 너비(픽셀).
    pub width: u32,
    /// 출력 높이(픽셀).
    pub height: u32,
}

/// `width`×`height` 픽셀을 빠짐없이 덮도록 16×16 workgroup 을 디스패치한다.
pub(crate) fn dispatch_screen(cpass: &mut wgpu::ComputePass<'_>, width: u32, height: u32) {
    cpass.dispatch_workgroups(
        width.div_ceil(SCREEN_WORKGROUP_SIZE),
        height.div_ceil(SCREEN_WORKGROUP_SIZE),
        1,
    );
}
