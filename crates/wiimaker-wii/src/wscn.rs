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
use wiimaker_core::{
    Animator, AnimatorCondition, AnimatorLayer, AnimatorParam, AnimatorParamKind, AnimatorState,
    AnimatorTransition, AudioSource, BlendDimension, BlendMotion, BlendTree, Curve, CurveInterp,
    CurveKey, PlayableDirector, TimelineClipRuntime, TimelineTrackKind, TimelineTrackRuntime,
};

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
/// Missing controller / timeline index.
pub const NO_REF: u16 = 0xFFFF;

/// Sprite cell resolved at bake (name → wpack tex + UV/pivot/size).
#[derive(Clone, Debug)]
pub struct BakedSpriteCell {
    pub name: String,
    pub tex: u16,
    pub uv: Rect,
    pub pivot: Vec2,
    pub size: Vec2,
}

impl BakedSpriteCell {
    pub fn texture_id(&self) -> Option<TextureId> {
        if self.tex == TILE_NO_TEX {
            None
        } else {
            Some(TextureId(self.tex as u32))
        }
    }
}

/// Parsed WSCN ready for `render_world` (clear + populated [`World`]).
#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub clear: [u8; 4],
    pub world: World,
    /// Cells used by baked Animator / timeline Animation tracks.
    pub cells: Vec<BakedSpriteCell>,
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
    BadValue,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::TooShort => write!(f, "wscn too short"),
            ParseError::BadMagic => write!(f, "bad wscn magic"),
            ParseError::Truncated => write!(f, "wscn truncated"),
            ParseError::Alloc => write!(f, "alloc failed"),
            ParseError::BadValue => write!(f, "wscn bad value"),
        }
    }
}

/// Cap `Vec::with_capacity` for u16 tail counts (corrupt `0xFFFF` must not reserve megabytes).
fn tail_reserve(c: &Cursor<'_>, n: usize) -> usize {
    n.min(64).min(c.remaining())
}

fn read_finite(c: &mut Cursor<'_>) -> Result<f32, ParseError> {
    let v = c.f32()?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(ParseError::BadValue)
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
        let kind = if c.remaining() > 0 {
            c.u8()?
        } else {
            KIND_NONE
        };

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
                let awake = if c.remaining() > 0 {
                    c.u8()? != 0
                } else {
                    false
                };
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
            let awake = if c.remaining() > 0 {
                c.u8()? != 0
            } else {
                false
            };
            if ei < ids.len() {
                attach_audio(&mut world, ids[ei], clip, vol, awake);
            }
        }
    }

    let mut cells = Vec::new();
    if c.remaining() >= 2 {
        load_animator_director_tail(&mut c, &mut world, &ids, &mut cells)?;
    }

    Ok(LoadedScene {
        clear,
        world,
        cells,
    })
}

fn read_str(c: &mut Cursor<'_>) -> Result<String, ParseError> {
    let n = c.u16()? as usize;
    Ok(string_from_bytes(c.take(n)?))
}

fn load_cell_list(c: &mut Cursor<'_>) -> Result<Vec<String>, ParseError> {
    let n = c.u16()? as usize;
    let mut cells = Vec::with_capacity(tail_reserve(c, n));
    for _ in 0..n {
        cells.push(read_str(c)?);
    }
    Ok(cells)
}

fn load_curve(c: &mut Cursor<'_>) -> Result<Option<Curve>, ParseError> {
    let n = c.u16()? as usize;
    if n == 0 {
        return Ok(None);
    }
    let mut keys = Vec::with_capacity(tail_reserve(c, n));
    for _ in 0..n {
        let t = read_finite(c)?;
        let v = read_finite(c)?;
        let interp = match c.u8()? {
            1 => CurveInterp::Constant,
            2 => CurveInterp::Ease,
            _ => CurveInterp::Linear,
        };
        keys.push(CurveKey { t, v, interp });
    }
    Ok(Some(Curve::from_keys(keys)))
}

fn load_blend_tree(c: &mut Cursor<'_>) -> Result<BlendTree, ParseError> {
    let dimension = match c.u8()? {
        1 => BlendDimension::TwoD,
        _ => BlendDimension::OneD,
    };
    let pn = c.u16()? as usize;
    let mut params = Vec::with_capacity(tail_reserve(c, pn));
    for _ in 0..pn {
        params.push(read_str(c)?);
    }
    let mn = c.u16()? as usize;
    let mut motions = Vec::with_capacity(tail_reserve(c, mn));
    for _ in 0..mn {
        let clip = read_str(c)?;
        let cells = load_cell_list(c)?;
        let fps = read_finite(c)?;
        let loop_ = c.u8()? != 0;
        let threshold = read_finite(c)?;
        let px = read_finite(c)?;
        let py = read_finite(c)?;
        motions.push(BlendMotion {
            clip,
            cells,
            fps,
            loop_,
            threshold,
            position: [px, py],
        });
    }
    Ok(BlendTree::new(dimension, params, motions))
}

fn load_state(c: &mut Cursor<'_>) -> Result<AnimatorState, ParseError> {
    let name = read_str(c)?;
    let clip = read_str(c)?;
    let speed = read_finite(c)?;
    let cells = load_cell_list(c)?;
    let fps = read_finite(c)?;
    let loop_ = c.u8()? != 0;
    let blend = if c.u8()? != 0 {
        Some(load_blend_tree(c)?)
    } else {
        None
    };
    Ok(AnimatorState {
        name,
        clip,
        speed,
        cells,
        fps,
        loop_,
        blend,
    })
}

fn load_condition(c: &mut Cursor<'_>) -> Result<AnimatorCondition, ParseError> {
    let kind = c.u8()?;
    let param = read_str(c)?;
    Ok(match kind {
        1 => AnimatorCondition::FloatEq {
            param,
            value: read_finite(c)?,
        },
        2 => AnimatorCondition::FloatGreater {
            param,
            value: read_finite(c)?,
        },
        3 => AnimatorCondition::FloatLess {
            param,
            value: read_finite(c)?,
        },
        4 => AnimatorCondition::Trigger { param },
        _ => AnimatorCondition::BoolEq {
            param,
            value: c.u8()? != 0,
        },
    })
}

fn load_transition(c: &mut Cursor<'_>) -> Result<AnimatorTransition, ParseError> {
    let from = read_str(c)?;
    let from_any = c.u8()? != 0;
    let to = read_str(c)?;
    let has_exit_time = c.u8()? != 0;
    let n = c.u16()? as usize;
    let mut conditions = Vec::with_capacity(tail_reserve(c, n));
    for _ in 0..n {
        conditions.push(load_condition(c)?);
    }
    Ok(AnimatorTransition {
        from,
        from_any,
        to,
        conditions,
        has_exit_time,
    })
}

fn load_machine(
    c: &mut Cursor<'_>,
) -> Result<(Vec<AnimatorState>, Vec<AnimatorTransition>), ParseError> {
    let sn = c.u16()? as usize;
    let mut states = Vec::with_capacity(tail_reserve(c, sn));
    for _ in 0..sn {
        states.push(load_state(c)?);
    }
    let tn = c.u16()? as usize;
    let mut transitions = Vec::with_capacity(tail_reserve(c, tn));
    for _ in 0..tn {
        transitions.push(load_transition(c)?);
    }
    Ok((states, transitions))
}

struct BakedController {
    name: String,
    default_state: String,
    base_weight: f32,
    parameters: Vec<(String, AnimatorParamKind, bool, f32)>,
    states: Vec<AnimatorState>,
    transitions: Vec<AnimatorTransition>,
    layers: Vec<(
        String,
        f32,
        String,
        Vec<AnimatorState>,
        Vec<AnimatorTransition>,
    )>,
}

fn load_controller(c: &mut Cursor<'_>) -> Result<BakedController, ParseError> {
    let name = read_str(c)?;
    let default_state = read_str(c)?;
    let base_weight = read_finite(c)?;
    let pn = c.u16()? as usize;
    let mut parameters = Vec::with_capacity(tail_reserve(c, pn));
    for _ in 0..pn {
        let pname = read_str(c)?;
        let kind = match c.u8()? {
            1 => AnimatorParamKind::Float,
            2 => AnimatorParamKind::Trigger,
            _ => AnimatorParamKind::Bool,
        };
        let default_bool = c.u8()? != 0;
        let default_float = read_finite(c)?;
        parameters.push((pname, kind, default_bool, default_float));
    }
    let (states, transitions) = load_machine(c)?;
    let ln = c.u16()? as usize;
    let mut layers = Vec::with_capacity(tail_reserve(c, ln));
    for _ in 0..ln {
        let lname = read_str(c)?;
        let weight = read_finite(c)?;
        let def = read_str(c)?;
        let (lst, ltr) = load_machine(c)?;
        layers.push((lname, weight, def, lst, ltr));
    }
    Ok(BakedController {
        name,
        default_state,
        base_weight,
        parameters,
        states,
        transitions,
        layers,
    })
}

fn load_timeline(c: &mut Cursor<'_>) -> Result<PlayableDirector, ParseError> {
    let name = read_str(c)?;
    let duration = read_finite(c)?;
    let mut d = PlayableDirector::new(name, duration, false, false);
    let tn = c.u16()? as usize;
    for _ in 0..tn {
        let tname = read_str(c)?;
        let kind = match c.u8()? {
            1 => TimelineTrackKind::Animation,
            2 => TimelineTrackKind::Audio,
            3 => TimelineTrackKind::Transform,
            4 => TimelineTrackKind::Signal,
            5 => TimelineTrackKind::Control,
            6 => TimelineTrackKind::Float,
            _ => TimelineTrackKind::Activation,
        };
        let binding = read_str(c)?;
        let property = read_str(c)?;
        let cn = c.u16()? as usize;
        let mut clips = Vec::with_capacity(tail_reserve(c, cn));
        for _ in 0..cn {
            let start = read_finite(c)?;
            let end = read_finite(c)?;
            let active = c.u8()? != 0;
            let clip = read_str(c)?;
            let cells = load_cell_list(c)?;
            let fps = read_finite(c)?;
            let loop_clip = c.u8()? != 0;
            let audio = read_str(c)?;
            let volume = read_finite(c)?;
            let from = [read_finite(c)?, read_finite(c)?];
            let to = [read_finite(c)?, read_finite(c)?];
            let signal = read_str(c)?;
            let payload = read_str(c)?;
            let curve_x = load_curve(c)?;
            let curve_y = load_curve(c)?;
            let curve_value = load_curve(c)?;
            clips.push(TimelineClipRuntime {
                start,
                end,
                active,
                clip,
                cells,
                fps,
                loop_clip,
                audio,
                volume,
                from,
                to,
                signal,
                payload,
                curve_x,
                curve_y,
                curve_value,
            });
        }
        d.tracks.push(TimelineTrackRuntime {
            name: tname,
            kind,
            binding,
            property,
            clips,
        });
    }
    Ok(d)
}

fn load_animator_director_tail(
    c: &mut Cursor<'_>,
    world: &mut World,
    ids: &[EntityId],
    cells_out: &mut Vec<BakedSpriteCell>,
) -> Result<(), ParseError> {
    let cell_n = c.u16()? as usize;
    for _ in 0..cell_n {
        let name = read_str(c)?;
        let tex = c.u16()?;
        let u0 = c.f32()?;
        let v0 = c.f32()?;
        let u1 = c.f32()?;
        let v1 = c.f32()?;
        let px = c.f32()?;
        let py = c.f32()?;
        let sw = c.f32()?;
        let sh = c.f32()?;
        cells_out.push(BakedSpriteCell {
            name,
            tex,
            uv: uv_rect(u0, v0, u1, v1),
            pivot: Vec2::new(px, py),
            size: Vec2::new(sw, sh),
        });
    }

    let cn = c.u16()? as usize;
    let mut controllers = Vec::with_capacity(tail_reserve(c, cn));
    for _ in 0..cn {
        controllers.push(load_controller(c)?);
    }

    let tn = c.u16()? as usize;
    let mut timelines = Vec::with_capacity(tail_reserve(c, tn));
    for _ in 0..tn {
        timelines.push(load_timeline(c)?);
    }

    let an = c.u16()? as usize;
    for _ in 0..an {
        let ei = c.u16()? as usize;
        let ci = c.u16()?;
        let mut rt = if ci != NO_REF {
            controllers
                .get(ci as usize)
                .map(|ctrl| {
                    let mut a = Animator::new(ctrl.name.clone());
                    a.state = ctrl.default_state.clone();
                    a.base_weight = ctrl.base_weight;
                    for (name, kind, db, df) in &ctrl.parameters {
                        let p = match kind {
                            AnimatorParamKind::Bool => AnimatorParam::bool_param(name.clone(), *db),
                            AnimatorParamKind::Float => {
                                AnimatorParam::float_param(name.clone(), *df)
                            }
                            AnimatorParamKind::Trigger => {
                                AnimatorParam::trigger_param(name.clone())
                            }
                        };
                        a.parameters.push(p);
                    }
                    a.states = ctrl.states.clone();
                    a.transitions = ctrl.transitions.clone();
                    for (lname, weight, def, lst, ltr) in &ctrl.layers {
                        a.layers.push(AnimatorLayer {
                            name: lname.clone(),
                            weight: *weight,
                            state: def.clone(),
                            state_time: 0.0,
                            states: lst.clone(),
                            transitions: ltr.clone(),
                        });
                    }
                    a
                })
                .unwrap_or_else(|| Animator::new(String::new()))
        } else {
            Animator::new(String::new())
        };
        let pon = c.u16()? as usize;
        for _ in 0..pon {
            let pname = read_str(c)?;
            let has_bool = c.u8()? != 0;
            let bval = c.u8()? != 0;
            let has_float = c.u8()? != 0;
            let fval = read_finite(c)?;
            if let Some(p) = rt.parameters.iter_mut().find(|p| p.name == pname) {
                if has_bool {
                    p.bool_value = bval;
                }
                if has_float {
                    p.float_value = fval;
                    if p.kind == AnimatorParamKind::Bool {
                        p.bool_value = fval != 0.0;
                    }
                }
            }
        }
        let lon = c.u16()? as usize;
        for _ in 0..lon {
            let lname = read_str(c)?;
            let weight = read_finite(c)?;
            let _ = rt.set_layer_weight(&lname, weight);
        }
        if ei < ids.len() {
            let id = ids[ei];
            let state = rt.state.clone();
            world.set_animator(id, Some(rt));
            if !state.is_empty() {
                world.apply_animator_state(id, &state);
            }
        }
    }

    let dn = c.u16()? as usize;
    for _ in 0..dn {
        let ei = c.u16()? as usize;
        let ti = c.u16()?;
        let play_on_awake = c.u8()? != 0;
        let loop_ = c.u8()? != 0;
        if ei >= ids.len() {
            continue;
        }
        let mut d = if ti != NO_REF {
            timelines
                .get(ti as usize)
                .cloned()
                .unwrap_or_else(|| PlayableDirector::new(String::new(), 0.0, play_on_awake, loop_))
        } else {
            PlayableDirector::new(String::new(), 0.0, play_on_awake, loop_)
        };
        d.play_on_awake = play_on_awake;
        d.loop_ = loop_;
        d.playing = play_on_awake;
        world.set_director(ids[ei], Some(d));
    }

    world.capture_timeline_baselines();
    world.apply_playing_timelines();
    apply_baked_clip_frames(world, cells_out, 0.0);
    Ok(())
}

fn lookup_cell<'a>(cells: &'a [BakedSpriteCell], name: &str) -> Option<&'a BakedSpriteCell> {
    cells.iter().find(|c| c.name == name)
}

/// Advance clip times (when `dt > 0`) and push the current cell onto sibling sprites.
pub fn apply_baked_clip_frames(world: &mut World, cells: &[BakedSpriteCell], dt: f32) {
    let ids: Vec<_> = world.iter_entities().collect();
    for id in ids {
        let Some(anim) = world.animation_mut(id) else {
            continue;
        };
        if anim.cells.is_empty() || anim.fps <= 0.0 {
            continue;
        }
        let n = anim.cells.len();
        let hold = anim.hold_time;
        if !hold && dt > 0.0 {
            anim.time += dt;
        }
        let frame_dur = 1.0 / anim.fps;
        let mut idx = (anim.time / frame_dur) as usize;
        if hold {
            if anim.loop_ {
                idx %= n;
            } else if idx >= n {
                idx = n - 1;
            }
        } else if anim.loop_ {
            idx %= n;
            let cycle = frame_dur * n as f32;
            if cycle > 0.0 && anim.time >= cycle {
                anim.time %= cycle;
            }
        } else if idx >= n {
            idx = n - 1;
            anim.time = frame_dur * n as f32;
        }
        anim.frame = idx;
        let cell_name = anim.cells[idx].clone();
        let Some(resolved) = lookup_cell(cells, &cell_name) else {
            continue;
        };
        let Some(tex) = resolved.texture_id() else {
            continue;
        };
        if let Some(sp) = world.sprite_mut(id) {
            sp.texture = tex;
            sp.uv = resolved.uv;
            if !sp.lock_pivot {
                sp.pivot = resolved.pivot;
            }
            sp.size = resolved.size;
        }
    }
}
