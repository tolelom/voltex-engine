//! [`Sprite`] — a camera-facing billboard component for the [`BasicRenderer`]
//! path. An entity with a [`WorldTransform`](voltex_ecs::WorldTransform) and a
//! `Sprite` is drawn as a screen-aligned quad at its world position, after the
//! mesh passes (alpha-blended, depth-tested). Use for pickups, particles,
//! world-space markers, or simple 2D-in-3D enemies.

use crate::assets::TextureHandle;
use voltex_ecs::World;

/// A billboarded quad. `size` is the world-space width/height; `color` tints
/// the texture (or acts as a solid color when `texture` is `None`, since the
/// renderer samples a white default). `color.w` is alpha.
#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    /// Optional diffuse texture; `None` → solid `color` quad.
    pub texture: Option<TextureHandle>,
    /// World-space [width, height].
    pub size: [f32; 2],
    /// RGBA tint / solid color (linear, alpha-blended).
    pub color: [f32; 4],
    /// Texture sub-rect `[u_min, v_min, u_max, v_max]` for sprite sheets.
    /// `[0, 0, 1, 1]` (default) samples the whole texture.
    pub uv_rect: [f32; 4],
    /// Mirror horizontally at render time (e.g. a right-facing character
    /// walking left). Display-only: `uv_rect` stays forward-oriented, so
    /// sheet-frame updates ([`set_sheet_frame`](Self::set_sheet_frame),
    /// [`update_sprite_animations`]) never fight the flip.
    pub flip_x: bool,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            texture: None,
            size: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            flip_x: false,
        }
    }
}

impl Sprite {
    /// A solid-color billboard of the given world-space size.
    pub fn solid(size: [f32; 2], color: [f32; 4]) -> Self {
        Self { texture: None, size, color, ..Default::default() }
    }

    /// A textured billboard (white tint) of the given world-space size.
    pub fn textured(texture: TextureHandle, size: [f32; 2]) -> Self {
        Self {
            texture: Some(texture),
            size,
            ..Default::default()
        }
    }

    /// Set the texture sub-rect (builder style) — e.g. for an atlas region.
    pub fn with_uv_rect(mut self, uv_rect: [f32; 4]) -> Self {
        self.uv_rect = uv_rect;
        self
    }

    /// The uv rect the renderer should sample — [`Self::flip_x`] swaps the
    /// u endpoints (the sprite shader lerps between them, so reversed
    /// endpoints mirror cleanly).
    pub fn effective_uv_rect(&self) -> [f32; 4] {
        if self.flip_x {
            [self.uv_rect[2], self.uv_rect[1], self.uv_rect[0], self.uv_rect[3]]
        } else {
            self.uv_rect
        }
    }

    /// Point this sprite at frame `index` of a `cols × rows` sprite sheet
    /// (row-major, left-to-right then top-to-bottom). Sets `uv_rect`.
    pub fn set_sheet_frame(&mut self, cols: u32, rows: u32, index: u32) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let index = index % (cols * rows);
        let (cx, cy) = (index % cols, index / cols);
        let (fw, fh) = (1.0 / cols as f32, 1.0 / rows as f32);
        let u0 = cx as f32 * fw;
        let v0 = cy as f32 * fh;
        self.uv_rect = [u0, v0, u0 + fw, v0 + fh];
    }
}

/// Plays a sprite-sheet animation by advancing a [`Sprite`]'s `uv_rect` over
/// time. Pair with a [`Sprite`] (its texture should be the sheet);
/// [`update_sprite_animations`] drives the frame. Frames run row-major across
/// a `cols × rows` grid, `0..frame_count`.
#[derive(Debug, Clone, Copy)]
pub struct SpriteAnimation {
    /// Sheet grid columns.
    pub cols: u32,
    /// Sheet grid rows.
    pub rows: u32,
    /// First sheet cell of this clip (row-major index) — lets one sheet hold
    /// several clips (idle/run/jump) selected by offset. `0` = sheet start.
    pub first_frame: u32,
    /// Number of frames to play (≤ `cols * rows`).
    pub frame_count: u32,
    /// Playback speed in frames per second.
    pub fps: f32,
    /// Seconds elapsed in the current playback.
    pub time: f32,
    /// Whether playback is advancing.
    pub playing: bool,
    /// Loop back to the clip's first frame at the end (else clamp to the
    /// clip's last frame + stop).
    pub looping: bool,
}

impl SpriteAnimation {
    /// A looping animation over the first `frame_count` cells of a
    /// `cols × rows` sheet at `fps` frames/sec.
    pub fn new(cols: u32, rows: u32, frame_count: u32, fps: f32) -> Self {
        Self::clip(cols, rows, 0, frame_count, fps)
    }

    /// A looping animation over `frame_count` cells starting at sheet cell
    /// `first_frame` (row-major) — one sheet, many clips. The clip is clamped
    /// to the sheet: frames past `cols * rows` would otherwise wrap to cell 0
    /// (`set_sheet_frame` wraps modulo the sheet) and silently splice another
    /// clip's frames into playback.
    pub fn clip(cols: u32, rows: u32, first_frame: u32, frame_count: u32, fps: f32) -> Self {
        let total = cols.max(1) * rows.max(1);
        let first_frame = first_frame.min(total - 1);
        Self {
            cols,
            rows,
            first_frame,
            frame_count: frame_count.clamp(1, total - first_frame),
            fps,
            time: 0.0,
            playing: true,
            looping: true,
        }
    }

    /// The current frame index for the elapsed `time` (before wrap/clamp).
    pub fn raw_frame(&self) -> u32 {
        (self.time * self.fps) as u32
    }
}

/// Advance every [`SpriteAnimation`] and write the resulting frame into its
/// entity's [`Sprite`] (`uv_rect`). Looping animations wrap; non-looping ones
/// clamp to the last frame and set `playing = false`. No-op when none exist.
pub fn update_sprite_animations(world: &mut World, dt: f32) {
    let ents: Vec<voltex_ecs::Entity> =
        world.query::<SpriteAnimation>().map(|(e, _)| e).collect();
    for e in ents {
        let Some(mut anim) = world.get::<SpriteAnimation>(e).copied() else {
            continue;
        };
        if anim.playing {
            anim.time += dt;
        }
        let raw = anim.raw_frame();
        let frame = if anim.looping {
            raw % anim.frame_count
        } else if raw >= anim.frame_count {
            anim.playing = false;
            anim.frame_count - 1
        } else {
            raw
        };
        if let Some(s) = world.get_mut::<Sprite>(e) {
            s.set_sheet_frame(anim.cols, anim.rows, anim.first_frame + frame);
        }
        if let Some(slot) = world.get_mut::<SpriteAnimation>(e) {
            *slot = anim;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_ecs::Transform;
    use voltex_math::Vec3;

    #[test]
    fn default_uv_rect_is_full_texture() {
        assert_eq!(Sprite::default().uv_rect, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(Sprite::solid([1.0, 1.0], [1.0; 4]).uv_rect, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn sheet_frame_picks_the_right_cell() {
        let mut s = Sprite::default();
        // 2×2 sheet: frame 0 = top-left quadrant.
        s.set_sheet_frame(2, 2, 0);
        assert_eq!(s.uv_rect, [0.0, 0.0, 0.5, 0.5]);
        // frame 3 = bottom-right quadrant (col 1, row 1).
        s.set_sheet_frame(2, 2, 3);
        assert_eq!(s.uv_rect, [0.5, 0.5, 1.0, 1.0]);
        // index wraps modulo the frame count.
        s.set_sheet_frame(2, 2, 4);
        assert_eq!(s.uv_rect, [0.0, 0.0, 0.5, 0.5]);
    }

    fn anim_world() -> (World, voltex_ecs::Entity) {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::ZERO));
        world.add(e, Sprite::default());
        world.add(e, SpriteAnimation::new(2, 2, 4, 10.0)); // 10 fps, 4 frames
        (world, e)
    }

    #[test]
    fn animation_advances_frames_over_time() {
        let (mut world, e) = anim_world();
        // 0.25s @ 10fps = frame 2 → BL quadrant [0, 0.5, 0.5, 1.0].
        update_sprite_animations(&mut world, 0.25);
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.0, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn looping_animation_wraps() {
        let (mut world, e) = anim_world();
        // 0.5s @ 10fps = raw frame 5 → 5 % 4 = 1 → TR quadrant.
        update_sprite_animations(&mut world, 0.5);
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.5, 0.0, 1.0, 0.5]);
        assert!(world.get::<SpriteAnimation>(e).unwrap().playing);
    }

    #[test]
    fn flip_x_swaps_u_endpoints_at_render_time_only() {
        let mut s = Sprite::default();
        s.set_sheet_frame(2, 2, 1); // TR quadrant [0.5, 0, 1, 0.5]
        assert_eq!(s.effective_uv_rect(), [0.5, 0.0, 1.0, 0.5]);
        s.flip_x = true;
        // 표시용 uv만 반전 — 저장된 uv_rect는 정방향 유지 (애니 시스템 소유).
        assert_eq!(s.effective_uv_rect(), [1.0, 0.0, 0.5, 0.5]);
        assert_eq!(s.uv_rect, [0.5, 0.0, 1.0, 0.5]);
        // set_sheet_frame이 flip을 파괴하지 않는다.
        s.set_sheet_frame(2, 2, 0);
        assert!(s.flip_x);
        assert_eq!(s.effective_uv_rect(), [0.5, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn clip_plays_from_first_frame_offset() {
        let (mut world, e) = anim_world();
        // 4x2 시트에서 클립 = 프레임 4..6 (2프레임, 10fps).
        if let Some(a) = world.get_mut::<SpriteAnimation>(e) {
            *a = SpriteAnimation::clip(4, 2, 4, 2, 10.0);
        }
        // 0.05s → raw 0 → 표시 프레임 4 (2행 좌단).
        update_sprite_animations(&mut world, 0.05);
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.0, 0.5, 0.25, 1.0]);
        // +0.1s → raw 1 → 표시 프레임 5.
        update_sprite_animations(&mut world, 0.1);
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.25, 0.5, 0.5, 1.0]);
        // 루프 wrap은 클립 내부: +0.1s → raw 2 → 2 % 2 = 0 → 프레임 4.
        update_sprite_animations(&mut world, 0.1);
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.0, 0.5, 0.25, 1.0]);
    }

    #[test]
    fn clip_is_clamped_to_the_sheet() {
        // 8칸 시트에서 6번부터 4프레임을 요구 → 시트 밖 2프레임은 잘려
        // 6..8만 재생 (안 자르면 set_sheet_frame wrap으로 0,1이 섞인다).
        let a = SpriteAnimation::clip(4, 2, 6, 4, 10.0);
        assert_eq!((a.first_frame, a.frame_count), (6, 2));
        // first_frame 자체가 밖이면 마지막 셀 1프레임으로 강등.
        let a = SpriteAnimation::clip(4, 2, 99, 4, 10.0);
        assert_eq!((a.first_frame, a.frame_count), (7, 1));
    }

    #[test]
    fn non_looping_clip_clamps_to_clip_end() {
        let (mut world, e) = anim_world();
        if let Some(a) = world.get_mut::<SpriteAnimation>(e) {
            *a = SpriteAnimation::clip(4, 2, 4, 2, 10.0);
            a.looping = false;
        }
        update_sprite_animations(&mut world, 1.0); // 클립 끝을 지나침
        // 클립 마지막 프레임(5)에 고정 + 정지.
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.25, 0.5, 0.5, 1.0]);
        assert!(!world.get::<SpriteAnimation>(e).unwrap().playing);
    }

    #[test]
    fn non_looping_animation_clamps_and_stops() {
        let (mut world, e) = anim_world();
        if let Some(a) = world.get_mut::<SpriteAnimation>(e) {
            a.looping = false;
        }
        update_sprite_animations(&mut world, 1.0); // well past the end
        // Clamped to last frame (3 → BR).
        assert_eq!(world.get::<Sprite>(e).unwrap().uv_rect, [0.5, 0.5, 1.0, 1.0]);
        assert!(!world.get::<SpriteAnimation>(e).unwrap().playing);
    }
}
