//! [`Sprite`] — a camera-facing billboard component for the [`BasicRenderer`]
//! path. An entity with a [`WorldTransform`](voltex_ecs::WorldTransform) and a
//! `Sprite` is drawn as a screen-aligned quad at its world position, after the
//! mesh passes (alpha-blended, depth-tested). Use for pickups, particles,
//! world-space markers, or simple 2D-in-3D enemies.

use crate::assets::TextureHandle;

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
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            texture: None,
            size: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl Sprite {
    /// A solid-color billboard of the given world-space size.
    pub fn solid(size: [f32; 2], color: [f32; 4]) -> Self {
        Self { texture: None, size, color }
    }

    /// A textured billboard (white tint) of the given world-space size.
    pub fn textured(texture: TextureHandle, size: [f32; 2]) -> Self {
        Self {
            texture: Some(texture),
            size,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}
