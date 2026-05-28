//! Editor UI capture — exercises the immediate-mode UI + font atlas + alpha
//! pipeline (`voltex_editor::UiRenderer`) that the editor workflow depends on
//! and that has never been visually verified.

use voltex_editor::{UiContext, UiRenderer};

use crate::headless::HeadlessGpu;
use crate::target::{OffscreenTarget, CAPTURE_FORMAT};

/// Render a representative editor panel — toolbar buttons, text labels, a
/// checkbox and a slider — through the real `UiRenderer`. Uses the built-in
/// bitmap font (always available; TTF is optional and OS-dependent). Returns
/// RGBA8.
pub fn render_editor_ui(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let mut ui = UiContext::new(width as f32, height as f32);
    let mut ui_renderer = UiRenderer::new(device, queue, CAPTURE_FORMAT, &ui.font);

    // Build one frame of UI (no live mouse interaction).
    ui.begin_frame(0.0, 0.0, false);

    let screen_w = width as f32;
    ui.begin_panel("Toolbar", 0.0, 0.0, screen_w, 32.0);
    let _ = ui.button("Play");
    ui.same_line();
    let _ = ui.button("Spawn");
    ui.same_line();
    let _ = ui.button("Save");
    ui.same_line();
    let _ = ui.button("Load");
    ui.end_panel();

    // Free-floating labels + widgets to exercise glyph rendering across the
    // ASCII range and the controls' quads.
    ui.draw_text("Voltex Editor", 16.0, 56.0, [255, 255, 255, 255]);
    ui.draw_text("Hierarchy", 16.0, 84.0, [200, 220, 255, 255]);
    ui.draw_text("0123456789 .,:;!?", 16.0, 108.0, [180, 255, 180, 255]);
    ui.draw_text("The quick brown fox", 16.0, 132.0, [255, 220, 160, 255]);

    ui.begin_panel("Inspector", 16.0, 160.0, 260.0, 160.0);
    let _ = ui.checkbox("Visible", true);
    let _ = ui.slider("Scale", 1.5, 0.0, 4.0);
    let _ = ui.button("Add Component");
    ui.end_panel();

    ui.end_frame();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("UI capture"),
    });
    // UiRenderer uses LoadOp::Load (overlay), so clear the target first.
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.13,
                        g: 0.14,
                        b: 0.17,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    ui_renderer.render(
        device,
        queue,
        &mut encoder,
        &target.color_view,
        &ui.draw_list,
        screen_w,
        height as f32,
    );
    queue.submit(Some(encoder.finish()));

    target.read_rgba8(device, queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageStats;

    #[test]
    #[ignore]
    fn editor_ui_renders_text() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_editor_ui(&gpu, 640, 400);
        let stats = ImageStats::analyze(&rgba);
        // Text + widgets should produce many distinct colors over the flat bg.
        assert!(!stats.looks_blank(), "UI frame is blank: {stats:?}");
        assert!(
            stats.distinct_colors > 3,
            "no widgets/text drawn: {stats:?}"
        );
    }
}
