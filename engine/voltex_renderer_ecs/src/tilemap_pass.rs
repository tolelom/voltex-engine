//! `BasicRenderer`'s tilemap pass: every visible [`Tilemap`] is expanded into
//! one shared vertex stream and drawn with one texture bind + one indexed draw
//! per map. Runs after meshes and before sprites so sprites composite on top.

use std::cell::Cell;

use voltex_ecs::{World, WorldTransform};
use voltex_renderer::{
    create_tilemap_pipeline, tilemap_quad_indices, tilemap_uniform_bind_group_layout,
    GpuTexture, TilemapUniform, TilemapVertex,
};

use crate::assets::{RenderAssets, TextureHandle};
use crate::frame::{FrameTarget, FrameView};
use crate::tilemap::{tilemap_vertices, Tilemap};

/// Max tile quads drawn per frame across all tilemaps (fixed-size buffers —
/// `render()` has no `Device` to grow them).
pub(crate) const MAX_TILE_QUADS: usize = 16_384;

/// Two triangles per tile quad (see `tilemap_quad_indices`).
const INDICES_PER_QUAD: u32 = 6;

/// One map's slice of the shared vertex stream.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TilemapBatch {
    pub texture: Option<TextureHandle>,
    pub first_quad: u32,
    pub quad_count: u32,
}

/// All tilemaps of a frame packed into one vertex stream.
pub(crate) struct PackedTilemaps {
    /// Quads (4 vertices each) of every in-budget map, back to back.
    pub vertices: Vec<TilemapVertex>,
    /// One batch per non-empty, in-budget map.
    pub batches: Vec<TilemapBatch>,
    /// Quads requested before the cap — compare with the cap to warn on overflow.
    pub requested_quads: usize,
}

/// Concatenate per-map quad streams up to `cap_quads`. Maps past the cap are
/// truncated, not reordered.
pub(crate) fn pack_tilemap_batches(
    maps: Vec<(Option<TextureHandle>, Vec<TilemapVertex>)>,
    cap_quads: usize,
) -> PackedTilemaps {
    let requested: usize = maps.iter().map(|(_, v)| v.len() / 4).sum();
    let mut vertices = Vec::with_capacity(requested.min(cap_quads) * 4);
    let mut batches = Vec::new();
    for (texture, verts) in maps {
        let used = vertices.len() / 4;
        let take = (verts.len() / 4).min(cap_quads - used);
        if take == 0 {
            continue;
        }
        vertices.extend_from_slice(&verts[..take * 4]);
        batches.push(TilemapBatch {
            texture,
            first_quad: used as u32,
            quad_count: take as u32,
        });
    }
    PackedTilemaps {
        vertices,
        batches,
        requested_quads: requested,
    }
}

pub(crate) struct TilemapPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    overflow_warned: Cell<bool>,
    /// 텍스처 없는 항목에 바인딩하는 `BasicRenderer` 의 흰색 1×1 (같은 GPU 리소스를 공유).
    default_texture_bind_group: wgpu::BindGroup,
}

impl TilemapPass {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        texture_layout: &wgpu::BindGroupLayout,
        default_texture: &GpuTexture,
    ) -> Self {
        let uniform_layout = tilemap_uniform_bind_group_layout(device);
        let pipeline = create_tilemap_pipeline(device, format, &uniform_layout, texture_layout);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tilemap Uniform"),
            size: std::mem::size_of::<TilemapUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tilemap Uniform BG"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tilemap Vertices"),
            size: (MAX_TILE_QUADS * 4 * std::mem::size_of::<TilemapVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices: Vec<u32> = (0..MAX_TILE_QUADS as u32)
            .flat_map(tilemap_quad_indices)
            .collect();
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tilemap Indices"),
            size: (indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));
        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            overflow_warned: Cell::new(false),
            default_texture_bind_group: default_texture.bind_group.clone(),
        }
    }

    /// 보이는 `Tilemap` 을 텍스처별 배치로 묶어 `target` 위에 그린다
    /// (텍스처 없는 맵은 흰색 1×1).
    pub(crate) fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget<'_>,
        view: &FrameView,
        world: &World,
        assets: &RenderAssets,
    ) {
        let maps: Vec<(Option<TextureHandle>, Vec<TilemapVertex>)> = world
            .query2::<WorldTransform, Tilemap>()
            .filter(|(e, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, map)| (map.texture, tilemap_vertices(map, &wt.0)))
            .collect();
        if maps.is_empty() {
            return;
        }
        let PackedTilemaps {
            vertices,
            batches,
            requested_quads,
        } = pack_tilemap_batches(maps, MAX_TILE_QUADS);
        crate::basic_renderer::warn_overflow_once(
            &self.overflow_warned,
            "tile quads",
            requested_quads,
            MAX_TILE_QUADS,
        );
        if batches.is_empty() {
            return;
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&TilemapUniform { view_proj: view.view_proj.cols }),
        );
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let mut pass = crate::scene_pass::begin_overlay_pass(
            encoder,
            "BasicRenderer Tilemap Pass",
            target.color_view,
            target.depth_view,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for batch in &batches {
            let texture_bind_group = batch
                .texture
                .and_then(|h| assets.texture(h))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture_bind_group);
            pass.set_bind_group(1, texture_bind_group, &[]);
            let first_index = batch.first_quad * INDICES_PER_QUAD;
            pass.draw_indexed(first_index..first_index + batch.quad_count * INDICES_PER_QUAD, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quads(n: usize) -> Vec<TilemapVertex> {
        vec![
            TilemapVertex { position: [0.0; 3], uv: [0.0; 2], color: [1.0; 4] };
            n * 4
        ]
    }

    #[test]
    fn packs_maps_back_to_back_one_batch_each() {
        let packed = pack_tilemap_batches(vec![(None, quads(3)), (None, quads(2))], 100);
        assert_eq!(packed.vertices.len(), 20);
        assert_eq!(packed.requested_quads, 5);
        assert_eq!(
            packed.batches.iter().map(|b| (b.first_quad, b.quad_count)).collect::<Vec<_>>(),
            vec![(0, 3), (3, 2)]
        );
    }

    #[test]
    fn empty_maps_get_no_batch() {
        let packed = pack_tilemap_batches(vec![(None, quads(0)), (None, quads(1))], 100);
        assert_eq!(packed.vertices.len(), 4);
        assert_eq!(packed.batches.len(), 1);
        assert_eq!(packed.batches[0].first_quad, 0);
    }

    #[test]
    fn cap_truncates_the_overflowing_map_and_drops_later_ones() {
        let packed =
            pack_tilemap_batches(vec![(None, quads(3)), (None, quads(4)), (None, quads(2))], 5);
        assert_eq!(packed.requested_quads, 9, "overflow is still reported in full");
        assert_eq!(packed.vertices.len(), 5 * 4);
        assert_eq!(
            packed.batches.iter().map(|b| (b.first_quad, b.quad_count)).collect::<Vec<_>>(),
            vec![(0, 3), (3, 2)]
        );
    }
}
