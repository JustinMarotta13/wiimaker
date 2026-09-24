//! WSCN0002 / WSCN0003 loader (same layout as `stub_game.c` / `wiimaker-scene` bake).

#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;
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

#[derive(Clone, Debug)]
pub struct TilePal {
    pub id: u16,
    pub color: [u8; 4],
    pub tex: u16,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub auto_mode: u8,
    pub auto_tex: [u16; 16],
    pub auto_u0: [f32; 16],
    pub auto_v0: [f32; 16],
    pub auto_u1: [f32; 16],
    pub auto_v1: [f32; 16],
    pub frame_n: u8,
    pub fps: f32,
    pub time: f32,
    pub frame_tex: [u16; MAX_TILE_FRAMES],
    pub frame_u0: [f32; MAX_TILE_FRAMES],
    pub frame_v0: [f32; MAX_TILE_FRAMES],
    pub frame_u1: [f32; MAX_TILE_FRAMES],
    pub frame_v1: [f32; MAX_TILE_FRAMES],
}

impl Default for TilePal {
    fn default() -> Self {
        Self {
            id: 0,
            color: [48, 88, 176, 255],
            tex: TILE_NO_TEX,
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            auto_mode: TILE_AUTO_OFF,
            auto_tex: [TILE_NO_TEX; 16],
            auto_u0: [0.0; 16],
            auto_v0: [0.0; 16],
            auto_u1: [1.0; 16],
            auto_v1: [1.0; 16],
            frame_n: 0,
            fps: 0.0,
            time: 0.0,
            frame_tex: [TILE_NO_TEX; MAX_TILE_FRAMES],
            frame_u0: [0.0; MAX_TILE_FRAMES],
            frame_v0: [0.0; MAX_TILE_FRAMES],
            frame_u1: [1.0; MAX_TILE_FRAMES],
            frame_v1: [1.0; MAX_TILE_FRAMES],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WiiTilemap {
    pub cell: f32,
    pub ox: f32,
    pub oy: f32,
    pub w: u16,
    pub h: u16,
    pub cells: Vec<u16>,
    pub solid: Vec<u8>,
    pub pal: Vec<TilePal>,
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub sx: f32,
    pub sy: f32,
    pub kind: u8,
    pub tex: u16,
    pub size_w: f32,
    pub size_h: f32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub radius: f32,
    pub color: [u8; 4],
    pub z: f32,
    pub text: String,
    pub text_size: f32,
    pub text_align: u8,
    pub tm: Option<WiiTilemap>,
    pub has_audio: bool,
    pub audio_clip: u16,
    pub audio_volume: f32,
    pub audio_awake: bool,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            name: String::new(),
            x: 0.0,
            y: 0.0,
            sx: 1.0,
            sy: 1.0,
            kind: KIND_NONE,
            tex: 0,
            size_w: 0.0,
            size_h: 0.0,
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
            radius: 0.0,
            color: [255, 255, 255, 255],
            z: 0.0,
            text: String::new(),
            text_size: 8.0,
            text_align: 0,
            tm: None,
            has_audio: false,
            audio_clip: AUDIO_NO_CLIP,
            audio_volume: 1.0,
            audio_awake: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub clear: [u8; 4],
    pub entities: Vec<Entity>,
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

fn load_tilemap_payload(c: &mut Cursor<'_>) -> Result<(WiiTilemap, f32), ParseError> {
    let mut tm = WiiTilemap {
        cell: c.f32()?,
        ox: c.f32()?,
        oy: c.f32()?,
        w: c.u16()?,
        h: c.u16()?,
        ..Default::default()
    };
    let z = c.f32()?;
    let n = c.u32()?;
    let expect = tm.w as u32 * tm.h as u32;
    if n != expect || n > MAX_TILE_CELLS {
        return Ok((tm, z));
    }
    tm.cells = Vec::with_capacity(n as usize);
    for _ in 0..n {
        tm.cells.push(c.u16()?);
    }
    let sbytes = ((n + 7) / 8) as usize;
    let solid_bytes = if sbytes == 0 { 1 } else { sbytes };
    let packed = if c.remaining() >= ((n + 7) / 8) as usize {
        let nb = ((n + 7) / 8) as usize;
        let bytes = c.take(nb)?;
        let mut v = bytes.to_vec();
        if v.len() < solid_bytes {
            v.resize(solid_bytes, 0);
        }
        v
    } else {
        vec![0u8; solid_bytes]
    };
    tm.solid = packed;

    if c.remaining() >= 2 {
        let pal_n = c.u16()?;
        for _ in 0..pal_n {
            if c.remaining() == 0 {
                break;
            }
            let mut scratch = TilePal::default();
            scratch.id = c.u16()?;
            scratch.color = c.rgba()?;
            scratch.tex = c.u16()?;
            scratch.u0 = c.f32()?;
            scratch.v0 = c.f32()?;
            scratch.u1 = c.f32()?;
            scratch.v1 = c.f32()?;
            scratch.auto_mode = if c.remaining() > 0 { c.u8()? } else { 0 };
            let auto_bits = c.u16()?;
            for m in 0..16 {
                if auto_bits & (1u16 << m) != 0 {
                    scratch.auto_tex[m] = c.u16()?;
                    scratch.auto_u0[m] = c.f32()?;
                    scratch.auto_v0[m] = c.f32()?;
                    scratch.auto_u1[m] = c.f32()?;
                    scratch.auto_v1[m] = c.f32()?;
                }
            }
            scratch.frame_n = if c.remaining() > 0 { c.u8()? } else { 0 };
            scratch.fps = c.f32()?;
            if scratch.frame_n as usize > MAX_TILE_FRAMES {
                scratch.frame_n = MAX_TILE_FRAMES as u8;
            }
            for f in 0..scratch.frame_n as usize {
                scratch.frame_tex[f] = c.u16()?;
                scratch.frame_u0[f] = c.f32()?;
                scratch.frame_v0[f] = c.f32()?;
                scratch.frame_u1[f] = c.f32()?;
                scratch.frame_v1[f] = c.f32()?;
            }
            tm.pal.push(scratch);
        }
    }
    Ok((tm, z))
}

/// Parse a baked `scene.wscn` (WSCN0002 or WSCN0003).
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
    let mut entities = Vec::with_capacity(n.min(64) as usize);

    for _ in 0..n {
        let mut e = Entity::default();
        let name_len = c.u16()? as usize;
        let name_bytes = c.take(name_len)?;
        e.name = String::from_utf8_lossy(name_bytes).into_owned();
        e.x = c.f32()?;
        e.y = c.f32()?;
        let _tz = c.f32()?;
        e.sx = c.f32()?;
        e.sy = c.f32()?;
        let _sz = c.f32()?;
        e.kind = if c.remaining() > 0 { c.u8()? } else { KIND_NONE };

        match e.kind {
            KIND_SPRITE => {
                e.tex = c.u16()?;
                e.size_w = c.f32()?;
                e.size_h = c.f32()?;
                e.u0 = c.f32()?;
                e.v0 = c.f32()?;
                e.u1 = c.f32()?;
                e.v1 = c.f32()?;
                e.pivot_x = c.f32()?;
                e.pivot_y = c.f32()?;
                e.color = c.rgba()?;
                e.z = c.f32()?;
            }
            KIND_DISC => {
                e.radius = c.f32()?;
                e.color = c.rgba()?;
                e.z = c.f32()?;
            }
            KIND_TILEMAP => {
                let plen = c.u32()? as usize;
                let payload = c.take(plen)?;
                let mut tc = Cursor::new(payload);
                let (tm, z) = load_tilemap_payload(&mut tc)?;
                e.z = z;
                e.tm = Some(tm);
            }
            KIND_TEXT => {
                let slen = c.u16()? as usize;
                let bytes = c.take(slen)?;
                e.text = String::from_utf8_lossy(bytes).into_owned();
                e.text_size = c.f32()?;
                e.text_align = if c.remaining() > 0 { c.u8()? } else { 0 };
                e.color = c.rgba()?;
                e.z = c.f32()?;
            }
            KIND_AUDIO => {
                e.has_audio = true;
                e.audio_clip = c.u16()?;
                e.audio_volume = c.f32()?;
                e.audio_awake = if c.remaining() > 0 { c.u8()? != 0 } else { false };
            }
            _ => {}
        }
        entities.push(e);
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
            if ei < entities.len() {
                entities[ei].has_audio = true;
                entities[ei].audio_clip = clip;
                entities[ei].audio_volume = vol;
                entities[ei].audio_awake = awake;
            }
        }
    }

    Ok(LoadedScene { clear, entities })
}

pub fn rgba_pack(c: [u8; 4]) -> u32 {
    ((c[0] as u32) << 24) | ((c[1] as u32) << 16) | ((c[2] as u32) << 8) | (c[3] as u32)
}
