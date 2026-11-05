//! `BasicRenderer` 의 패스들이 같은 컬러 + 깊이 타깃에 여는 렌더 패스.
//!
//! 프레임의 첫 메시 패스만 타깃을 지우고([`begin_clearing_pass`]), 이후 패스는
//! 앞 결과 위에 그린다([`begin_overlay_pass`]).

/// 컬러를 `clear_color`, 깊이를 1.0(가장 먼 값)으로 지우고 시작하는 패스.
pub(crate) fn begin_clearing_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    label: &str,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    clear_color: wgpu::Color,
) -> wgpu::RenderPass<'encoder> {
    begin_scene_pass(
        encoder,
        label,
        color_view,
        depth_view,
        wgpu::LoadOp::Clear(clear_color),
        wgpu::LoadOp::Clear(1.0),
    )
}

/// 기존 컬러·깊이를 유지(Load)한 채 그 위에 그리는 패스.
pub(crate) fn begin_overlay_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    label: &str,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
) -> wgpu::RenderPass<'encoder> {
    begin_scene_pass(
        encoder,
        label,
        color_view,
        depth_view,
        wgpu::LoadOp::Load,
        wgpu::LoadOp::Load,
    )
}

fn begin_scene_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    label: &str,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    color_load: wgpu::LoadOp<wgpu::Color>,
    depth_load: wgpu::LoadOp<f32>,
) -> wgpu::RenderPass<'encoder> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: color_load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: depth_load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    })
}
