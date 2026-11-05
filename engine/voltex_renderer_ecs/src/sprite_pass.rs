//! `BasicRenderer`'s billboard sprite pass: every visible [`Sprite`] is drawn as a
//! camera-facing quad, sorted back-to-front. Runs after the tilemap pass and
//! before debug lines.

use std::cell::Cell;

use voltex_ecs::{World, WorldTransform};
use voltex_math::{Mat4, Vec3, Vec4};
use voltex_renderer::{
    create_sprite_pipeline, sprite_uniform_bind_group_layout, GpuTexture, SpriteUniform,
    SPRITE_INDICES, SPRITE_QUAD,
};

use crate::assets::RenderAssets;
use crate::basic_renderer::warn_overflow_once;
use crate::dynamic_uniform::DynamicUniformBuffer;
use crate::frame::{FrameTarget, FrameView};
use crate::scene_pass::begin_overlay_pass;
use crate::sprite::Sprite;

/// Max billboard sprites drawn per frame (each needs one dynamic-UBO slot).
const MAX_SPRITES: usize = 256;

pub(crate) struct SpritePass {
    pipeline: wgpu::RenderPipeline,
    quad_vbuf: wgpu::Buffer,
    quad_ibuf: wgpu::Buffer,
    uniforms: DynamicUniformBuffer<SpriteUniform>,
    uniform_bind_group: wgpu::BindGroup,
    overflow_warned: Cell<bool>,
    /// 텍스처 없는 항목에 바인딩하는 `BasicRenderer` 의 흰색 1×1 (같은 GPU 리소스를 공유).
    default_texture_bind_group: wgpu::BindGroup,
}

impl SpritePass {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        texture_layout: &wgpu::BindGroupLayout,
        default_texture: &GpuTexture,
        alignment: u32,
    ) -> Self {
        let sprite_uniform_layout = sprite_uniform_bind_group_layout(device);
        let pipeline =
            create_sprite_pipeline(device, target_format, &sprite_uniform_layout, texture_layout);
        let quad_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Sprite Quad VB"),
            size: std::mem::size_of_val(&SPRITE_QUAD) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&quad_vbuf, 0, bytemuck::cast_slice(&SPRITE_QUAD));
        let quad_ibuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Sprite Quad IB"),
            size: std::mem::size_of_val(&SPRITE_INDICES) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&quad_ibuf, 0, bytemuck::cast_slice(&SPRITE_INDICES));
        let uniforms = DynamicUniformBuffer::<SpriteUniform>::new(
            device,
            "BasicRenderer Sprite Uniform Dynamic UBO",
            alignment,
            MAX_SPRITES,
        );
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Sprite Uniform BG"),
            layout: &sprite_uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.binding(),
            }],
        });
        Self {
            pipeline,
            quad_vbuf,
            quad_ibuf,
            uniforms,
            uniform_bind_group,
            overflow_warned: Cell::new(false),
            default_texture_bind_group: default_texture.bind_group.clone(),
        }
    }

    /// Billboard pass: draws every entity carrying a [`WorldTransform`] + a
    /// [`Sprite`] as a camera-facing quad at its world position (alpha-blended,
    /// depth-tested, no depth write). Camera right/up are recovered from the
    /// inverse view-projection, so no extra camera state is threaded in. Sprites
    /// are sorted back-to-front by distance from the camera so overlapping
    /// transparency blends correctly regardless of spawn order. Capped at
    /// [`MAX_SPRITES`] per frame.
    pub(crate) fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget<'_>,
        view: &FrameView,
        world: &mut World,
        assets: &RenderAssets,
    ) {
        let FrameView { view_proj, camera_pos, .. } = *view;
        // (world position, (sprite, world matrix)) for each visible sprite.
        let mut items: Vec<(Vec3, (Sprite, Mat4))> = world
            .query2::<WorldTransform, Sprite>()
            .filter(|(e, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, s)| {
                let c = wt.0.cols[3];
                (Vec3::new(c[0], c[1], c[2]), (*s, wt.0))
            })
            .collect();
        if items.is_empty() {
            return;
        }
        // World-space camera right/up/forward from the inverse VP (clip axes
        // map back to camera axes for a shear-free perspective/ortho proj).
        let inv = view_proj.inverse().unwrap_or(Mat4::IDENTITY);
        let dir = |v: Vec4| {
            let r = inv * v;
            Vec3::new(r.x, r.y, r.z).normalize_or_zero()
        };
        let right = dir(Vec4::new(1.0, 0.0, 0.0, 0.0));
        let up = dir(Vec4::new(0.0, 1.0, 0.0, 0.0));
        let forward = dir(Vec4::new(0.0, 0.0, 1.0, 0.0));

        warn_overflow_once(&self.overflow_warned, "sprites", items.len(), MAX_SPRITES);
        let draw_count = cull_and_sort_sprites(&mut items, camera_pos, forward, MAX_SPRITES);
        items.truncate(draw_count);

        let uniforms: Vec<SpriteUniform> = items
            .iter()
            .map(|(pos, (sprite, world_m))| {
                let roll = if sprite.roll_from_transform {
                    crate::sprite::sprite_roll(world_m, right, up)
                } else {
                    0.0
                };
                SpriteUniform {
                    view_proj: view_proj.cols,
                    camera_right: [right.x, right.y, right.z, 0.0],
                    camera_up: [up.x, up.y, up.z, 0.0],
                    center: [pos.x, pos.y, pos.z, 0.0],
                    color: sprite.color,
                    size: [sprite.size[0], sprite.size[1], roll, 0.0],
                    uv_rect: sprite.effective_uv_rect(),
                }
            })
            .collect();
        self.uniforms.upload(queue, &uniforms);

        let mut pass =
            begin_overlay_pass(
            encoder,
            "BasicRenderer Sprite Pass",
            target.color_view,
            target.depth_view,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.quad_vbuf.slice(..));
        pass.set_index_buffer(self.quad_ibuf.slice(..), wgpu::IndexFormat::Uint16);
        for (i, (_, (sprite, _))) in items.iter().enumerate() {
            let texture_bind_group = sprite
                .texture
                .and_then(|h| assets.texture(h))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture_bind_group);
            pass.set_bind_group(1, texture_bind_group, &[]);
            pass.set_bind_group(0, &self.uniform_bind_group, &[self.uniforms.offset(i)]);
            pass.draw_indexed(0..SPRITE_INDICES.len() as u32, 0, 0..1);
        }
    }
}

/// Cull to the `max` sprites **nearest** the camera, then sort those
/// back-to-front, both by view depth `(pos - cam_pos)·forward` (not euclidean
/// distance — radial distance flips layering for coplanar sprites under an
/// orthographic camera). Returns the draw count; `items[..n]` is the ordered
/// draw list.
fn cull_and_sort_sprites<T>(
    items: &mut [(Vec3, T)],
    cam_pos: Vec3,
    forward: Vec3,
    max: usize,
) -> usize {
    let depth = |p: Vec3| (p - cam_pos).dot(forward);
    let n = items.len().min(max);
    if items.len() > max {
        // 가까운(depth 작은) n개를 앞으로 — 초과분은 먼 것부터 버린다.
        items.select_nth_unstable_by(n - 1, |a, b| {
            depth(a.0)
                .partial_cmp(&depth(b.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    items[..n].sort_by(|a, b| {
        depth(b.0)
            .partial_cmp(&depth(a.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_cull_keeps_nearest_and_sorts_back_to_front() {
        // 카메라 원점, forward -Z: z가 더 음수일수록 멀다? 아니 — depth =
        // (pos-cam)·forward 이므로 forward=(0,0,-1)일 때 z=-5가 depth 5(멀고),
        // z=-1이 depth 1(가깝다).
        let cam = Vec3::ZERO;
        let fwd = Vec3::new(0.0, 0.0, -1.0);
        let at = |z: f32| (Vec3::new(0.0, 0.0, z), Sprite::default());
        // 5개 중 3개만 그릴 수 있음 — 가까운 1,2,3이 살아남고 4,5는 컬링.
        let mut items = vec![at(-4.0), at(-1.0), at(-5.0), at(-3.0), at(-2.0)];
        let n = cull_and_sort_sprites(&mut items, cam, fwd, 3);
        assert_eq!(n, 3);
        let depths: Vec<f32> = items[..n].iter().map(|(p, _)| -p.z).collect();
        assert_eq!(depths, vec![3.0, 2.0, 1.0], "back-to-front, 가까운 3개만");
        // cap 미만이면 전량 back-to-front.
        let mut items = vec![at(-1.0), at(-3.0), at(-2.0)];
        let n = cull_and_sort_sprites(&mut items, cam, fwd, 256);
        assert_eq!(n, 3);
        let depths: Vec<f32> = items.iter().map(|(p, _)| -p.z).collect();
        assert_eq!(depths, vec![3.0, 2.0, 1.0]);
        // 직교 동일평면 시나리오: z=0 평면, 카메라 (0,4,10) forward -Z.
        // 유클리드 정렬이면 y에 따라 순서가 갈라졌겠지만 depth는 전부 동일 —
        // 순서가 안정적으로 유지되고 패닉 없음.
        let cam = Vec3::new(0.0, 4.0, 10.0);
        let mut items = vec![
            (Vec3::new(0.0, 6.0, 0.0), Sprite::default()),
            (Vec3::new(0.0, 1.0, 0.0), Sprite::default()),
        ];
        let n = cull_and_sort_sprites(&mut items, cam, fwd, 256);
        assert_eq!(n, 2);
    }
}
