//! Bitmap HUD text (Unity Text / UI analogue, monospace v1).
//!
//! Glyph pixels live in the host atlas (`wiimaker-assets` / `wiimaker-host`).
//! This crate only stores authored strings and layout.

use crate::color::Rgba8;
use crate::draw::Rect;
use crate::math::Vec2;
use crate::sorting::default_sorting_layer_index;

#[cfg(feature = "std")]
mod alloc_types {
    pub use std::string::String;
}

#[cfg(not(feature = "std"))]
mod alloc_types {
    extern crate alloc;
    pub use alloc::string::String;
}

use alloc_types::String;

/// Native glyph cell in the built-in 8×8 atlas (host).
pub const FONT_CELL_PX: f32 = 8.0;

/// Horizontal alignment of each line relative to the transform anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" | "l" => Some(TextAlign::Left),
            "center" | "centre" | "c" | "middle" => Some(TextAlign::Center),
            "right" | "r" => Some(TextAlign::Right),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TextAlign::Left => "Left",
            TextAlign::Center => "Center",
            TextAlign::Right => "Right",
        }
    }
}

/// Screen-space HUD string (Unity Text analogue for 2D).
#[derive(Clone, Debug)]
pub struct Text {
    pub string: String,
    /// Glyph height in world pixels (native atlas cell is [`FONT_CELL_PX`]).
    pub size: f32,
    pub color: Rgba8,
    pub align: TextAlign,
    /// Order in layer (Unity Order in Layer). Combined with [`Text::sorting_layer`].
    pub z: f32,
    /// Index into [`crate::world::World::sorting_layers`] (Unity Sorting Layer).
    pub sorting_layer: u16,
}

impl Text {
    pub fn new(string: impl Into<String>, size: f32, color: Rgba8) -> Self {
        Self {
            string: string.into(),
            size: size.max(0.0),
            color,
            align: TextAlign::Left,
            z: 0.0,
            sorting_layer: default_sorting_layer_index(),
        }
    }
}

/// Default authored HUD size (2× native 8px cell).
pub fn default_text_size() -> f32 {
    16.0
}

/// World-space AABB of `text` (top-left + size). Anchor is the transform XY;
/// each line is aligned independently.
pub fn text_aabb(text: &str, anchor: Vec2, size: f32, scale: Vec2, align: TextAlign) -> Rect {
    let gw = (size * scale.x).abs();
    let gh = (size * scale.y).abs();
    if gw < 1e-6 || gh < 1e-6 {
        return Rect::new(anchor.x, anchor.y, 0.0, 0.0);
    }
    let mut max_cols = 0usize;
    let mut lines = 0usize;
    for line in text.split('\n') {
        lines += 1;
        max_cols = max_cols.max(line.chars().count());
    }
    if lines == 0 {
        lines = 1;
    }
    let w = max_cols as f32 * gw;
    let h = lines as f32 * gh;
    let x = match align {
        TextAlign::Left => anchor.x,
        TextAlign::Center => anchor.x - w * 0.5,
        TextAlign::Right => anchor.x - w,
    };
    Rect::new(x, anchor.y, w, h)
}

/// X origin of one line of `cols` glyphs.
pub fn line_origin_x(anchor_x: f32, cols: usize, glyph_w: f32, align: TextAlign) -> f32 {
    let w = cols as f32 * glyph_w;
    match align {
        TextAlign::Left => anchor_x,
        TextAlign::Center => anchor_x - w * 0.5,
        TextAlign::Right => anchor_x - w,
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    #[test]
    fn left_aabb_is_anchor_origin() {
        let r = text_aabb(
            "Hi",
            Vec2::new(10.0, 20.0),
            8.0,
            Vec2::new(1.0, 1.0),
            TextAlign::Left,
        );
        assert!((r.x - 10.0).abs() < 1e-4);
        assert!((r.y - 20.0).abs() < 1e-4);
        assert!((r.w - 16.0).abs() < 1e-4);
        assert!((r.h - 8.0).abs() < 1e-4);
    }

    #[test]
    fn center_shifts_left_by_half_width() {
        let r = text_aabb(
            "AB",
            Vec2::new(100.0, 0.0),
            10.0,
            Vec2::ONE,
            TextAlign::Center,
        );
        assert!((r.x - 90.0).abs() < 1e-4);
        assert!((r.w - 20.0).abs() < 1e-4);
    }

    #[test]
    fn newline_grows_height() {
        let r = text_aabb("A\nB", Vec2::ZERO, 8.0, Vec2::ONE, TextAlign::Left);
        assert!((r.h - 16.0).abs() < 1e-4);
        assert!((r.w - 8.0).abs() < 1e-4);
    }
}
