//! WSCN0002 / WSCN0003 → core [`World`] (no JSON / no `wiimaker-scene` on PowerPC).
//!
//! Texture ids are wpack indices (same as host atlas insertion order). WSCN has
//! no named sorting layers — drawables keep Default + baked `z` as order-in-layer.

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{Rect, TextureId};
use wiimaker_core::math::{Quat, Vec2, Vec3};
use wiimaker_core::sorting::default_sorting_layer_index;
use wiimaker_core::text::{Text, TextAlign};
use wiimaker_core::tilemap::{AutoTileMatch, TileVisual, Tilemap};
use wiimaker_core::world::{Disc, EntityId, Sprite, Transform, World};
use wiimaker_core::AudioSource;

#[cfg(feature = "std")]
use std::string::{String, ToString};
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub const KIND_NONE: u8 = 0;
pub const KIND_SPRITE: u8 = 1;
pub const KIND_DISC: u8 = 2;
pub const KIND_TILEMAP: u8 = 3;
pub const KIND_TEXT: u8 = 4;
pub const KIND_AUDIO: u8 = 5;

pub const TILE_NO_TEX: u16 = 0xFFFF;
pub const TILE_AUTO_OFF: u8 = 0;
pub const TILE_AUTO_ID: u8 = 1;
pub const TILE_AUTO_SOLID: u8 = 2;
pub const MAX_TILE_FRAMES: usize = 16;
pub const MAX_TILE_CELLS: u32 = 16384;
pub const AUDIO_NO_CLIP: u16 = 0xFFFF;

/// Parsed WSCN ready for `render_world` (clear + populated [`World`]).
#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub clear: [u8; 4],
    pub world: World,
}

impl LoadedScene {
    pub fn clear_rgba(&self) -> Rgba8 {
        Rgba8::new(self.clear[0], self.clear[1], self.clear[2], self.clear[3])
    }
}

#[derive(Debug)]
pub enum ParseError {
    TooShort,
    BadMagic,
    Truncated,
    Alloc,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::TooShort => write!(f, "wscn too short"),
            ParseError::BadMagic => write!(f, "bad wscn magic"),
            ParseError::Truncated => write!(f, "wscn truncated"),
            ParseError::Alloc => write!(f, "alloc failed"),
        }
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, i: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.i)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.i + n > self.data.len() {
            return Err(ParseError::Truncated);
        }
        let s = &self.data[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ParseError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, ParseError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self) -> Result<f32, ParseError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn rgba(&mut self) -> Result<[u8; 4], ParseError> {
        let b = self.take(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }
}

fn string_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn uv_rect(u0: f32, v0: f32, u1: f32, v1: f32) -> Rect {
    Rect::new(u0, v0, u1 - u0, v1 - v0)
}

fn maybe_tex(tex: u16, u0: f32, v0: f32, u1: f32, v1: f32) -> Option<(TextureId, Rect)> {
    if tex == TILE_NO_TEX {
        None
    } else {
        Some((TextureId(tex as u32), uv_rect(u0, v0, u1, v1)))
    }
}

fn rgba8(c: [u8; 4]) -> Rgba8 {
    Rgba8::new(c[0], c[1], c[2], c[3])
}

/// Wii ASND clip index stored on [`AudioSource::clip`] (decimal). Empty = skip.
pub fn audio_clip_string(clip: u16) -> String {
    if clip == AUDIO_NO_CLIP {
        String::new()
    } else {
        clip.to_string()
    }
}

/// Parse a decimal wpack audio index written by [`audio_clip_string`].
pub fn parse_audio_clip_id(clip: &str) -> Option<u32> {
    let id: u32 = clip.parse().ok()?;
    if id == AUDIO_NO_CLIP as u32 {
        None
    } else {
        Some(id)
    }
}

fn attach_audio(world: &mut World, id: EntityId, clip: u16, volume: f32, awake: bool) {
    world.set_audio_source(
        id,
        Some(AudioSource::new(audio_clip_string(clip), volume, awake)),
    );
}

fn load_tilemap_payload(c: &mut Cursor<'_>) -> Result<Tilemap, ParseError> {
    let cell = c.f32()?;
    let ox = c.f32()?;
    let oy = c.f32()?;
    let w = c.u16()?;
    let h = c.u16()?;
    let z = c.f32()?;
    let n = c.u32()?;
    let mut tm = Tilemap::new(w as u32, h as u32, cell);
    tm.origin = Vec2::new(ox, oy);
    tm.z = z;
    tm.sorting_layer = default_sorting_layer_index();
    let expect = tm.width.saturating_mul(tm.height);
    if n != expect || n > MAX_TILE_CELLS {
        return Ok(tm);
    }
    tm.cells.clear();
    tm.cells.reserve(n as usize);
    for _ in 0..n {
        tm.cells.push(c.u16()?);
    }
    let packed_n = ((n + 7) / 8) as usize;
    let solid_bytes = packed_n.max(1);
    tm.solid = if c.remaining() >= packed_n {
        let bytes = c.take(packed_n)?;
        let mut v = bytes.to_vec();
        if v.len() < solid_bytes {
            v.resize(solid_bytes, 0);
        }
        v
    } else {
        vec![0u8; solid_bytes]
    };

    if c.remaining() >= 2 {
        let pal_n = c.u16()?;
        for _ in 0..pal_n {
            if c.remaining() == 0 {
                break;
            }
            let id = c.u16()?;
            let color = rgba8(c.rgba()?);
            let tex = c.u16()?;
            let u0 = c.f32()?;
            let v0 = c.f32()?;
            let u1 = c.f32()?;
            let v1 = c.f32()?;
            let auto_mode = if c.remaining() > 0 { c.u8()? } else { 0 };
            let auto_bits = c.u16()?;
            let mut vis = TileVisual::color_only(id, color);
            vis.texture = maybe_tex(tex, u0, v0, u1, v1);
            vis.auto_tile = match auto_mode {
                TILE_AUTO_ID => Some(AutoTileMatch::Id),
                TILE_AUTO_SOLID => Some(AutoTileMatch::Solid),
                TILE_AUTO_OFF | _ => None,
            };
            for m in 0..16 {
                if auto_bits & (1u16 << m) != 0 {
                    let at = c.u16()?;
                    let au0 = c.f32()?;
                    let av0 = c.f32()?;
                    let au1 = c.f32()?;
                    let av1 = c.f32()?;
                    vis.auto_frames[m] = maybe_tex(at, au0, av0, au1, av1);
                }
            }
            let mut frame_n = if c.remaining() > 0 { c.u8()? } else { 0 };
            vis.fps = c.f32()?;
            vis.loop_ = true;
            if frame_n as usize > MAX_TILE_FRAMES {
                frame_n = MAX_TILE_FRAMES as u8;
            }
            for _ in 0..frame_n as usize {
                let ft = c.u16()?;
                let fu0 = c.f32()?;
                let fv0 = c.f32()?;
                let fu1 = c.f32()?;
                let fv1 = c.f32()?;
                if let Some(fr) = maybe_tex(ft, fu0, fv0, fu1, fv1) {
                    vis.frames.push(fr);
                }
            }
            if vis.texture.is_none() {
                vis.texture = vis.frames.first().copied();
            }
            tm.palette.push(vis);
        }
    }
    Ok(tm)
}

/// Parse a baked `scene.wscn` (WSCN0002 or WSCN0003) into a core [`World`].
pub fn parse_wscn(data: &[u8]) -> Result<LoadedScene, ParseError> {
    if data.len() < 16 {
        return Err(ParseError::TooShort);
    }
    let mut c = Cursor::new(data);
    let magic = c.take(8)?;
    if magic != b"WSCN0003" && magic != b"WSCN0002" {
        return Err(ParseError::BadMagic);
    }
    let clear = c.rgba()?;
    let n = c.u32()?;
    let mut world = World::new();
    let mut ids: Vec<EntityId> = Vec::with_capacity(n.min(64) as usize);
    let default_layer = default_sorting_layer_index();

    for _ in 0..n {
        let name_len = c.u16()? as usize;
        let name_bytes = c.take(name_len)?;
        let name = string_from_bytes(name_bytes);
        let x = c.f32()?;
        let y = c.f32()?;
        let tz = c.f32()?;
        let sx = c.f32()?;
        let sy = c.f32()?;
        let sz = c.f32()?;
        let kind = if c.remaining() > 0 { c.u8()? } else { KIND_NONE };

        let xf = Transform {
            translation: Vec3::new(x, y, tz),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(sx, sy, sz),
        };
        let id = world.spawn_named(name, xf);

        match kind {
            KIND_SPRITE => {
                let tex = c.u16()?;
                let size_w = c.f32()?;
                let size_h = c.f32()?;
                let u0 = c.f32()?;
                let v0 = c.f32()?;
                let u1 = c.f32()?;
                let v1 = c.f32()?;
                let pivot_x = c.f32()?;
                let pivot_y = c.f32()?;
                let color = rgba8(c.rgba()?);
                let z = c.f32()?;
                let mut sp = Sprite::new(TextureId(tex as u32), Vec2::new(size_w, size_h));
                sp.uv = uv_rect(u0, v0, u1, v1);
                sp.pivot = Vec2::new(pivot_x, pivot_y);
                sp.color = color;
                sp.z = z;
                sp.sorting_layer = default_layer;
                world.set_sprite(id, Some(sp));
            }
            KIND_DISC => {
                let radius = c.f32()?;
                let color = rgba8(c.rgba()?);
                let z = c.f32()?;
                let mut d = Disc::new(radius, color);
                d.z = z;
                d.sorting_layer = default_layer;
                world.set_disc(id, Some(d));
            }
            KIND_TILEMAP => {
                let plen = c.u32()? as usize;
                let payload = c.take(plen)?;
                let mut tc = Cursor::new(payload);
                let tm = load_tilemap_payload(&mut tc)?;
                world.set_tilemap(id, Some(tm));
            }
            KIND_TEXT => {
                let slen = c.u16()? as usize;
                let bytes = c.take(slen)?;
                let string = string_from_bytes(bytes);
                let size = c.f32()?;
                let align = TextAlign::from_u8(if c.remaining() > 0 { c.u8()? } else { 0 });
                let color = rgba8(c.rgba()?);
                let z = c.f32()?;
                let mut t = Text::new(string, size, color);
                t.align = align;
                t.z = z;
                t.sorting_layer = default_layer;
                world.set_text(id, Some(t));
            }
            KIND_AUDIO => {
                let clip = c.u16()?;
                let volume = c.f32()?;
                let awake = if c.remaining() > 0 { c.u8()? != 0 } else { false };
                attach_audio(&mut world, id, clip, volume, awake);
            }
            _ => {}
        }
        ids.push(id);
    }

    if c.remaining() >= 2 {
        let an = c.u16()?;
        for _ in 0..an {
            if c.remaining() == 0 {
                break;
            }
            let ei = c.u16()? as usize;
            let clip = c.u16()?;
            let vol = c.f32()?;
            let awake = if c.remaining() > 0 { c.u8()? != 0 } else { false };
            if ei < ids.len() {
                attach_audio(&mut world, ids[ei], clip, vol, awake);
            }
        }
    }

    Ok(LoadedScene { clear, world })
}
