//! Host-side draw command buffer. Records vertices + draw commands that
//! [`UiRenderer`](crate::UiRenderer) flushes to wgpu each frame.
//!
//! Vertex / command field schemas are GPU-facing structs whose layout the
//! wgpu pipeline depends on. We allow `missing_docs` here and keep the
//! narrative on the entry types ([`DrawVertex`], [`DrawCommand`], [`DrawList`]).
#![allow(missing_docs)]

use bytemuck::{Pod, Zeroable};
use crate::bitmap_font::FontAtlas;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct DrawVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// A scissor rectangle for content clipping, in pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub struct DrawCommand {
    pub index_offset: u32,
    pub index_count: u32,
    /// Optional scissor rect for clipping. None means no clipping.
    pub scissor: Option<ScissorRect>,
    /// Texture slot from [`UiRenderer::register_texture`](crate::UiRenderer::register_texture)
    /// to sample, or `None` for the font atlas (text / solid rects).
    pub texture: Option<u32>,
}

pub struct DrawList {
    pub vertices: Vec<DrawVertex>,
    pub indices: Vec<u16>,
    pub commands: Vec<DrawCommand>,
    scissor_stack: Vec<ScissorRect>,
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawList {
    pub fn new() -> Self {
        DrawList {
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: Vec::new(),
            scissor_stack: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.scissor_stack.clear();
    }

    /// Push a scissor rect onto the stack. All subsequent draw commands will
    /// be clipped to this rectangle until `pop_scissor` is called.
    pub fn push_scissor(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.scissor_stack.push(ScissorRect { x, y, w, h });
    }

    /// Pop the current scissor rect from the stack.
    pub fn pop_scissor(&mut self) {
        self.scissor_stack.pop();
    }

    /// Returns the current scissor rect (top of stack), or None.
    fn current_scissor(&self) -> Option<ScissorRect> {
        self.scissor_stack.last().copied()
    }

    /// Add a solid-color rectangle. UV is (0,0) for solid color rendering.
    pub fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
        self.add_rect_uv(x, y, w, h, 0.0, 0.0, 0.0, 0.0, color);
    }

    /// Add a textured quad with explicit UV coordinates.
    #[allow(clippy::too_many_arguments)]
    pub fn add_rect_uv(
        &mut self,
        x: f32, y: f32, w: f32, h: f32,
        u0: f32, v0: f32, u1: f32, v1: f32,
        color: [u8; 4],
    ) {
        let index_offset = self.indices.len() as u32;
        let base_vertex = self.vertices.len() as u16;

        // 4 vertices: top-left, top-right, bottom-right, bottom-left
        self.vertices.push(DrawVertex { position: [x,     y    ], uv: [u0, v0], color });
        self.vertices.push(DrawVertex { position: [x + w, y    ], uv: [u1, v0], color });
        self.vertices.push(DrawVertex { position: [x + w, y + h], uv: [u1, v1], color });
        self.vertices.push(DrawVertex { position: [x,     y + h], uv: [u0, v1], color });

        // 2 triangles = 6 indices
        self.indices.push(base_vertex);
        self.indices.push(base_vertex + 1);
        self.indices.push(base_vertex + 2);
        self.indices.push(base_vertex);
        self.indices.push(base_vertex + 2);
        self.indices.push(base_vertex + 3);

        self.commands.push(DrawCommand {
            index_offset,
            index_count: 6,
            scissor: self.current_scissor(),
            texture: None,
        });
    }

    /// Add a quad sampling a *registered* UI texture (full 0..1 UV, white
    /// tint). `texture_slot` comes from `UiRenderer::register_texture`; an
    /// unregistered slot falls back to the font atlas at render time.
    pub fn add_image(&mut self, x: f32, y: f32, w: f32, h: f32, texture_slot: u32) {
        self.add_rect_uv(x, y, w, h, 0.0, 0.0, 1.0, 1.0, [255, 255, 255, 255]);
        if let Some(cmd) = self.commands.last_mut() {
            cmd.texture = Some(texture_slot);
        }
    }

    /// Add text at the given position. One quad per character, using glyph UVs from the atlas.
    pub fn add_text(&mut self, font: &FontAtlas, text: &str, x: f32, y: f32, color: [u8; 4]) {
        let gw = font.glyph_width as f32;
        let gh = font.glyph_height as f32;
        let mut cx = x;
        for ch in text.chars() {
            let uv = font.glyph_uv_rect(ch);
            self.add_rect_uv(cx, y, gw, gh, uv.u0, uv.v0, uv.u1, uv.v1, color);
            cx += gw;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmap_font::FontAtlas;

    #[test]
    fn test_add_rect_vertex_index_count() {
        let mut dl = DrawList::new();
        dl.add_rect(0.0, 0.0, 100.0, 50.0, [255, 0, 0, 255]);
        assert_eq!(dl.vertices.len(), 4);
        assert_eq!(dl.indices.len(), 6);
        assert_eq!(dl.commands.len(), 1);
        assert_eq!(dl.commands[0].index_count, 6);
        assert_eq!(dl.commands[0].index_offset, 0);
    }

    #[test]
    fn add_image_records_texture_slot_and_full_uv() {
        let mut dl = DrawList::new();
        dl.add_rect(0.0, 0.0, 10.0, 10.0, [255, 0, 0, 255]);
        dl.add_image(0.0, 20.0, 64.0, 64.0, 3);

        assert_eq!(dl.commands.len(), 2);
        assert_eq!(dl.commands[0].texture, None, "rects stay on the font path");
        assert_eq!(dl.commands[1].texture, Some(3), "image carries its slot");
        // Image quad spans the full 0..1 UV range (vertices 4..8).
        assert_eq!(dl.vertices[4].uv, [0.0, 0.0]);
        assert_eq!(dl.vertices[6].uv, [1.0, 1.0]);
        assert_eq!(dl.vertices[4].color, [255, 255, 255, 255], "white tint");
    }

    #[test]
    fn test_add_text_char_count() {
        let font = FontAtlas::generate();
        let mut dl = DrawList::new();
        let text = "Hello";
        dl.add_text(&font, text, 0.0, 0.0, [255, 255, 255, 255]);
        // 5 chars => 5 quads => 5*4=20 vertices, 5*6=30 indices
        assert_eq!(dl.vertices.len(), 5 * 4);
        assert_eq!(dl.indices.len(), 5 * 6);
        assert_eq!(dl.commands.len(), 5);
    }

    #[test]
    fn test_clear() {
        let mut dl = DrawList::new();
        dl.add_rect(0.0, 0.0, 50.0, 50.0, [0, 0, 0, 255]);
        dl.clear();
        assert!(dl.vertices.is_empty());
        assert!(dl.indices.is_empty());
        assert!(dl.commands.is_empty());
    }
}
