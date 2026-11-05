//! Editor UI capture — exercises the immediate-mode UI + font atlas + alpha
//! pipeline (`voltex_editor::UiRenderer`) that the editor workflow depends on
//! and that has never been visually verified.

use voltex_editor::{ScreenTarget, UiContext, UiRenderer};

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
    let screen = ScreenTarget { view: &target.color_view, width: screen_w, height: height as f32 };
    ui_renderer.render_to(device, queue, &mut encoder, &screen, &ui.draw_list);
    queue.submit(Some(encoder.finish()));

    target.read_rgba8(device, queue)
}

/// Render a UI frame containing an **image widget** sampling a registered
/// magenta/green checker texture, plus a text label (font-atlas path) — the
/// two pipelines must coexist in one frame. Returns RGBA8.
pub fn render_ui_image_widget(gpu: &HeadlessGpu, width: u32, height: u32) -> Vec<u8> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let target = OffscreenTarget::new(device, width, height);

    let mut ui = UiContext::new(width as f32, height as f32);
    let mut ui_renderer = UiRenderer::new(device, queue, CAPTURE_FORMAT, &ui.font);

    // 2x2 magenta/green checker, uploaded the same way editor_demo thumbnails
    // are (GpuTexture::from_rgba → register_texture).
    let px: Vec<u8> = [
        [230u8, 20, 230, 255], // magenta
        [20u8, 230, 20, 255],  // green
        [20u8, 230, 20, 255],
        [230u8, 20, 230, 255],
    ]
    .concat();
    let layout = voltex_renderer::GpuTexture::bind_group_layout(device);
    let tex = voltex_renderer::GpuTexture::from_rgba(device, queue, 2, 2, &px, &layout);
    let slot = ui_renderer.register_texture(device, &tex.view);

    ui.begin_frame(0.0, 0.0, false);
    ui.draw_text("Preview:", 16.0, 8.0, [255, 255, 255, 255]);
    ui.layout.cursor_x = 16.0;
    ui.layout.cursor_y = 28.0;
    ui.image(slot, 128.0, 128.0);
    ui.end_frame();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("UI image capture"),
    });
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI image clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.13, g: 0.14, b: 0.17, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    let screen = ScreenTarget { view: &target.color_view, width: width as f32, height: height as f32 };
    ui_renderer.render_to(device, queue, &mut encoder, &screen, &ui.draw_list);
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

    /// UI image-widget regression: a registered RGBA texture renders with its
    /// actual colors (the `fs_image` pipeline — the font path would show only
    /// the red channel), coexisting with text in the same frame.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn ui_image_widget_renders_registered_texture_colors() {
        let gpu = match HeadlessGpu::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let rgba = render_ui_image_widget(&gpu, 320, 200);

        let (mut magenta, mut green) = (0usize, 0usize);
        for p in rgba.as_chunks::<4>().0.iter() {
            if p[0] > 150 && p[1] < 100 && p[2] > 150 {
                magenta += 1;
            } else if p[0] < 100 && p[1] > 150 && p[2] < 100 {
                green += 1;
            }
        }
        // The 128x128 checker splits roughly in half between the two colors.
        assert!(
            magenta > 2000 && green > 2000,
            "checker colors missing — fs_image path broken? magenta={magenta} green={green}"
        );
    }
}
