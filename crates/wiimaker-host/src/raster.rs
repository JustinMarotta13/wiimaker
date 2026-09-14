//! Tiny CPU rasterizer for DrawList commands (sprites/discs/text + clear).

use std::sync::OnceLock;

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{DrawCmd, DrawList, Rect};
use wiimaker_core::text::{line_origin_x, TextAlign};

use crate::atlas::TextureAtlas;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    pub fn fill(&mut self, color: Rgba8) {
        let c = pack(color);
        self.pixels.fill(c);
    }

    pub fn blend(&mut self, x: i32, y: i32, color: Rgba8) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        let dst = unpack(self.pixels[idx]);
        let a = color.a as f32 / 255.0;
        let out = Rgba8::new(
            lerp(dst.r, color.r, a),
            lerp(dst.g, color.g, a),
            lerp(dst.b, color.b, a),
            255,
        );
        self.pixels[idx] = pack(out);
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t) as u8
}

fn pack(c: Rgba8) -> u32 {
    // minifb expects 0RGB
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

fn unpack(p: u32) -> Rgba8 {
    Rgba8::new(
        ((p >> 16) & 0xff) as u8,
        ((p >> 8) & 0xff) as u8,
        (p & 0xff) as u8,
        255,
    )
}

pub fn flush(list: &DrawList, fb: &mut Framebuffer) {
    flush_with_atlas(list, fb, None);
}

pub fn flush_with_atlas(list: &DrawList, fb: &mut Framebuffer, atlas: Option<&TextureAtlas>) {
    for cmd in list.cmds() {
        match cmd {
            DrawCmd::Clear { color } => fb.fill(*color),
            DrawCmd::DrawDisc {
                center,
                radius,
                color,
                ..
            } => fill_disc(fb, center.x, center.y, *radius, *color),
            DrawCmd::DrawSprite {
                texture,
                dest,
                uv,
                color,
                ..
            } => {
                if let Some(atlas) = atlas {
                    blit_sprite(fb, atlas, *texture, dest, uv, *color);
                } else {
                    fill_rect(
                        fb,
                        dest.x as i32,
                        dest.y as i32,
                        dest.w as i32,
                        dest.h as i32,
                        *color,
                    );
                }
            }
            DrawCmd::DrawText {
                pos,
                text,
                color,
                size,
                align,
                ..
            } => blit_text(fb, pos.x, pos.y, text, *color, *size, *align),
            DrawCmd::SetCamera { .. } | DrawCmd::SetTexture { .. } | DrawCmd::DrawMesh { .. } => {}
        }
    }
}

fn font_atlas() -> &'static (u32, u32, Vec<u8>) {
    static ATLAS: OnceLock<(u32, u32, Vec<u8>)> = OnceLock::new();
    ATLAS.get_or_init(wiimaker_assets::atlas_rgba8)
}

fn sample_font(u: f32, v: f32) -> Rgba8 {
    let (w, h, rgba) = font_atlas();
    if *w == 0 || *h == 0 || rgba.is_empty() {
        return Rgba8::WHITE;
    }
    let u = u.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let x = ((u * *w as f32) as u32).min(w.saturating_sub(1));
    let y = ((v * *h as f32) as u32).min(h.saturating_sub(1));
    let i = ((y * *w + x) * 4) as usize;
    if i + 3 >= rgba.len() {
        return Rgba8::WHITE;
    }
    Rgba8::new(rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3])
}

fn blit_text(
    fb: &mut Framebuffer,
    anchor_x: f32,
    anchor_y: f32,
    text: &str,
    tint: Rgba8,
    size: f32,
    align: TextAlign,
) {
    if size <= 0.5 {
        return;
    }
    let mut y = anchor_y;
    for line in text.split('\n') {
        let cols = line.chars().count();
        let mut x = line_origin_x(anchor_x, cols, size, align);
        for ch in line.chars() {
            let mapped = wiimaker_assets::map_char(ch);
            let (u, v, du, dv) = wiimaker_assets::glyph_uv(mapped);
            blit_font_quad(
                fb,
                &Rect::new(x, y, size, size),
                &Rect::new(u, v, du, dv),
                tint,
            );
            x += size;
        }
        y += size;
    }
}

fn blit_font_quad(fb: &mut Framebuffer, dest: &Rect, uv: &Rect, tint: Rgba8) {
    let x0 = dest.x.floor() as i32;
    let y0 = dest.y.floor() as i32;
    let w = dest.w.ceil() as i32;
    let h = dest.h.ceil() as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    for py in 0..h {
        for px in 0..w {
            let u = uv.x + uv.w * ((px as f32 + 0.5) / w as f32);
            let v = uv.y + uv.h * ((py as f32 + 0.5) / h as f32);
            let mut sample = sample_font(u, v);
            sample.r = ((sample.r as u16 * tint.r as u16) / 255) as u8;
            sample.g = ((sample.g as u16 * tint.g as u16) / 255) as u8;
            sample.b = ((sample.b as u16 * tint.b as u16) / 255) as u8;
            sample.a = ((sample.a as u16 * tint.a as u16) / 255) as u8;
            if sample.a == 0 {
                continue;
            }
            fb.blend(x0 + px, y0 + py, sample);
        }
    }
}

fn blit_sprite(
    fb: &mut Framebuffer,
    atlas: &TextureAtlas,
    texture: wiimaker_core::TextureId,
    dest: &Rect,
    uv: &Rect,
    tint: Rgba8,
) {
    let x0 = dest.x.floor() as i32;
    let y0 = dest.y.floor() as i32;
    let w = dest.w.ceil() as i32;
    let h = dest.h.ceil() as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    for py in 0..h {
        for px in 0..w {
            let u = uv.x + uv.w * ((px as f32 + 0.5) / w as f32);
            let v = uv.y + uv.h * ((py as f32 + 0.5) / h as f32);
            let mut sample = atlas.sample(texture, u, v);
            sample.r = ((sample.r as u16 * tint.r as u16) / 255) as u8;
            sample.g = ((sample.g as u16 * tint.g as u16) / 255) as u8;
            sample.b = ((sample.b as u16 * tint.b as u16) / 255) as u8;
            sample.a = ((sample.a as u16 * tint.a as u16) / 255) as u8;
            if sample.a == 0 {
                continue;
            }
            fb.blend(x0 + px, y0 + py, sample);
        }
    }
}

fn fill_rect(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, color: Rgba8) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            fb.blend(px, py, color);
        }
    }
}

fn fill_disc(fb: &mut Framebuffer, cx: f32, cy: f32, radius: f32, color: Rgba8) {
    let r = radius.ceil() as i32;
    let icx = cx as i32;
    let icy = cy as i32;
    let r2 = radius * radius;
    for y in -r..=r {
        for x in -r..=r {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            if fx * fx + fy * fy <= r2 {
                fb.blend(icx + x, icy + y, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::draw::DrawList;
    use wiimaker_core::math::Vec2;

    fn unpack_rgb(p: u32) -> (u8, u8, u8) {
        (
            ((p >> 16) & 0xff) as u8,
            ((p >> 8) & 0xff) as u8,
            (p & 0xff) as u8,
        )
    }

    #[test]
    fn draw_text_inks_pixels() {
        let mut dl = DrawList::new();
        dl.clear(Rgba8::BLACK);
        dl.text(Vec2::new(2.0, 2.0), "A", Rgba8::WHITE, 16.0);
        let mut fb = Framebuffer::new(48, 32);
        flush(&dl, &mut fb);
        let ink = fb
            .pixels
            .iter()
            .filter(|p| unpack_rgb(**p) != (0, 0, 0))
            .count();
        assert!(ink > 20, "expected glyph ink, got {ink}");
    }

    #[test]
    fn missing_glyph_draws_question_mark() {
        let mut known = DrawList::new();
        known.clear(Rgba8::BLACK);
        known.text(Vec2::new(0.0, 0.0), "?", Rgba8::WHITE, 16.0);
        let mut fb_q = Framebuffer::new(24, 24);
        flush(&known, &mut fb_q);

        let mut missing = DrawList::new();
        missing.clear(Rgba8::BLACK);
        missing.text(Vec2::new(0.0, 0.0), "\u{2603}", Rgba8::WHITE, 16.0);
        let mut fb_m = Framebuffer::new(24, 24);
        flush(&missing, &mut fb_m);
        assert_eq!(fb_q.pixels, fb_m.pixels);
    }
}
