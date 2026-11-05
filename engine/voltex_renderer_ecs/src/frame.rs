//! [`BasicRenderer::render_frame`](crate::BasicRenderer::render_frame) 의 인자 묶음 —
//! 어디에 그릴지([`FrameTarget`])와 어떻게 볼지([`FrameView`]).

use voltex_math::{Mat4, Vec3};
use voltex_renderer::LightUniform;

/// 한 프레임을 그릴 컬러·깊이 타깃. 첫 메시 패스가 컬러를 `clear_color`, 깊이를
/// 1.0 으로 지우고, 이후 패스(스킨드·타일맵·스프라이트·디버그 라인)는 그 위에 그린다.
#[derive(Clone, Copy)]
pub struct FrameTarget<'a> {
    /// 컬러 렌더 타깃 (offscreen 뷰포트 또는 swapchain 뷰).
    pub color_view: &'a wgpu::TextureView,
    /// `DEPTH_FORMAT` 깊이 타깃.
    pub depth_view: &'a wgpu::TextureView,
    /// 프레임 시작 시 컬러를 지우는 색.
    pub clear_color: wgpu::Color,
}

/// 프레임을 보는 카메라와 장면의 주 방향광.
#[derive(Clone, Copy, Debug)]
pub struct FrameView {
    /// 카메라 view-projection 행렬 (프러스텀 컬링에도 쓴다).
    pub view_proj: Mat4,
    /// 카메라 월드 위치 (스페큘러·스프라이트 정렬용).
    pub camera_pos: Vec3,
    /// 방향광 (그림자 맵 방향도 여기서 정해진다).
    pub light: LightUniform,
}
