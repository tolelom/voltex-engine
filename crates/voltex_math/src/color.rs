//! `Color` — an RGBA `f32` color with the ergonomics games reach for: named
//! constants, hex literals, HSV, alpha tweaks, and interpolation. Stored in the
//! same `[r, g, b, a]` linear layout the renderer's material/sprite paths use,
//! so it converts to/from `[f32; 4]` for free.
//!
//! ```ignore
//! let c = Color::hex(0xFF8800);              // orange
//! let faded = c.with_alpha(0.5);
//! let mixed = Color::RED.lerp(Color::BLUE, 0.5);
//! material_desc_color(mixed.to_array());     // hand off to the renderer
//! ```

/// An RGBA color, components nominally in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha (opacity).
    pub a: f32,
}

impl Color {
    /// Opaque white.
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Opaque black.
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Fully transparent (zero everywhere).
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    /// Opaque red.
    pub const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Opaque green.
    pub const GREEN: Color = Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Opaque blue.
    pub const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Opaque yellow.
    pub const YELLOW: Color = Color { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Opaque cyan.
    pub const CYAN: Color = Color { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Opaque magenta.
    pub const MAGENTA: Color = Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Opaque mid gray.
    pub const GRAY: Color = Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 };

    /// Opaque color from RGB components.
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Color from RGBA components.
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Opaque color from a `0xRRGGBB` hex literal.
    pub fn hex(rgb: u32) -> Self {
        let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
        let b = (rgb & 0xFF) as f32 / 255.0;
        Self { r, g, b, a: 1.0 }
    }

    /// Color from a `0xRRGGBBAA` hex literal (alpha in the low byte).
    pub fn hexa(rgba: u32) -> Self {
        let r = ((rgba >> 24) & 0xFF) as f32 / 255.0;
        let g = ((rgba >> 16) & 0xFF) as f32 / 255.0;
        let b = ((rgba >> 8) & 0xFF) as f32 / 255.0;
        let a = (rgba & 0xFF) as f32 / 255.0;
        Self { r, g, b, a }
    }

    /// Opaque color from HSV (hue in degrees `[0, 360)`, saturation/value in
    /// `[0, 1]`). Useful for procedural / evenly-spaced palettes.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h.rem_euclid(360.0);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r1, g1, b1) = match h as u32 / 60 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Self {
            r: r1 + m,
            g: g1 + m,
            b: b1 + m,
            a: 1.0,
        }
    }

    /// From the renderer's `[r, g, b, a]` layout.
    pub fn from_array(c: [f32; 4]) -> Self {
        Self {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        }
    }

    /// To the renderer's `[r, g, b, a]` layout.
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// A copy with a different alpha.
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Component-wise linear interpolation toward `other` by `t` (for color
    /// transitions / tweens). `t` is not clamped — pass eased progress in `[0, 1]`.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl From<[f32; 4]> for Color {
    fn from(c: [f32; 4]) -> Self {
        Self::from_array(c)
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        c.to_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Color, b: Color) -> bool {
        (a.r - b.r).abs() < 1e-3
            && (a.g - b.g).abs() < 1e-3
            && (a.b - b.b).abs() < 1e-3
            && (a.a - b.a).abs() < 1e-3
    }

    #[test]
    fn rgb_is_opaque() {
        assert_eq!(Color::rgb(0.2, 0.4, 0.6), Color::rgba(0.2, 0.4, 0.6, 1.0));
    }

    #[test]
    fn hex_decodes_channels() {
        assert!(approx(Color::hex(0xFF0000), Color::RED));
        assert!(approx(Color::hex(0x00FF00), Color::GREEN));
        assert!(approx(Color::hex(0x0000FF), Color::BLUE));
        // A mixed value.
        let c = Color::hex(0xFF8000);
        assert!((c.r - 1.0).abs() < 1e-3 && (c.g - 0.502).abs() < 1e-3 && c.b == 0.0);
    }

    #[test]
    fn hexa_decodes_alpha() {
        let c = Color::hexa(0x00000080);
        assert!((c.a - 0.502).abs() < 1e-3, "alpha {}", c.a);
        assert_eq!(c.r, 0.0);
    }

    #[test]
    fn array_round_trips() {
        let c = Color::rgba(0.1, 0.2, 0.3, 0.4);
        assert_eq!(c.to_array(), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(Color::from_array([0.1, 0.2, 0.3, 0.4]), c);
        // From/Into conversions.
        let arr: [f32; 4] = c.into();
        assert_eq!(arr, [0.1, 0.2, 0.3, 0.4]);
        let back: Color = arr.into();
        assert_eq!(back, c);
    }

    #[test]
    fn with_alpha_only_changes_alpha() {
        let c = Color::RED.with_alpha(0.25);
        assert_eq!(c, Color::rgba(1.0, 0.0, 0.0, 0.25));
    }

    #[test]
    fn lerp_blends_channels() {
        let mid = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!(approx(mid, Color::rgba(0.5, 0.5, 0.5, 1.0)));
    }

    #[test]
    fn from_hsv_matches_known_hues() {
        assert!(approx(Color::from_hsv(0.0, 1.0, 1.0), Color::RED));
        assert!(approx(Color::from_hsv(120.0, 1.0, 1.0), Color::GREEN));
        assert!(approx(Color::from_hsv(240.0, 1.0, 1.0), Color::BLUE));
        // Zero saturation is grayscale = value on all channels.
        assert!(approx(Color::from_hsv(123.0, 0.0, 0.5), Color::rgba(0.5, 0.5, 0.5, 1.0)));
        // Hue wraps.
        assert!(approx(Color::from_hsv(360.0, 1.0, 1.0), Color::RED));
    }
}
