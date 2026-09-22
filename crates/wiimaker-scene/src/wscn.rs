//! Compact binary scene for Wii embedding (`scene.wscn`).
//!
//! Texture names / sprite cells are resolved to wpack indices + UV/pivot at bake
//! time so the C runtime never parses JSON or string-matches asset names.
//!
//! Host-first Sorting Layers are **not** packed: WSCN0003 still stores raw `z`
//! (order-in-layer). The C player sorts by `z` only. Do not bump the magic for
//! this feature.
//!
//! `KIND_TEXT = 4`, `KIND_TILEMAP = 3`, and `KIND_AUDIO = 5` share this magic
//! (sprite / disc / tilemap still win over text; those plus text win over
//! audio-only). Tilemap payloads stay length-prefixed: grid + solid bits, then
//! a resolved palette the GX player draws as textured or untextured quads.
//!
//! AudioSource is a component, not a exclusive draw kind:
//! - **KIND_AUDIO=5** — audio-only entities (no enabled Sprite/Disc/Tilemap/Text).
//!   Payload: `u16` wpack clip index ([`WSCN_AUDIO_NO_CLIP`] if missing), `f32`
//!   volume 0..1, `u8` play_on_awake.
//! - **Trailing table** (after the entity list, only when ≥1 enabled AudioSource):
//!   `u16` n, then n × (`u16` entity index, same clip/volume/awake payload).
//!   Covers Sprite/Disc/Tilemap/Text + AudioSource. C applies the table after
//!   kinds (idempotent if KIND_AUDIO is also listed). Old bakes omit the tail.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use wiimaker_assets::{AnimClipCatalog, SpriteCatalog, WPack};

use crate::scene::Scene;

/// Format version with UV rect + pivot on sprites.
pub const WSCN_MAGIC: &[u8; 8] = b"WSCN0003";

pub const KIND_NONE: u8 = 0;
pub const KIND_SPRITE: u8 = 1;
pub const KIND_DISC: u8 = 2;
pub const KIND_TILEMAP: u8 = 3;
/// HUD bitmap string (same 8×8 bits as `wiimaker-assets` font.rs). Magic stays WSCN0003.
pub const KIND_TEXT: u8 = 4;
/// Audio-only entity (no enabled Sprite/Disc/Tilemap/Text). Magic stays WSCN0003.
pub const KIND_AUDIO: u8 = 5;

/// Wpack audio TOC index meaning “missing / empty clip” — C must not play.
pub const WSCN_AUDIO_NO_CLIP: u16 = 0xFFFF;

/// Reasonable UTF-8 byte cap for Wii C `Entity.text` (loader still skips a longer bake).
pub const WSCN_TEXT_MAX_BYTES: usize = 255;

/// Wpack index meaning “no texture” — GX draws an untextured tinted quad
/// (host `TextureId(u32::MAX)` white sample × palette color).
pub const WSCN_TILE_NO_TEX: u16 = 0xFFFF;

/// Palette `auto_tile` byte in the tilemap payload.
pub const WSCN_TILEMAP_AUTO_OFF: u8 = 0;
pub const WSCN_TILEMAP_AUTO_ID: u8 = 1;
pub const WSCN_TILEMAP_AUTO_SOLID: u8 = 2;

/// Cap baked anim frames so the C player stays small (host clips may be longer).
pub const WSCN_TILEMAP_MAX_FRAMES: usize = 16;

/// Same default wall tint as [`crate::scene::SceneTilePalette`] / host `QUAD` cells.
pub const WSCN_TILE_DEFAULT_COLOR: [u8; 4] = [48, 88, 176, 255];

/// Bake a scene against a cooked pack into little-endian `scene.wscn` bytes.
pub fn bake_scene_wscn(scene: &Scene, pack: &WPack) -> Result<Vec<u8>> {
    bake_scene_wscn_with_catalog(scene, pack, None)
}

pub fn bake_scene_wscn_with_catalog(
    scene: &Scene,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
) -> Result<Vec<u8>> {
    bake_scene_wscn_with_catalogs(scene, pack, catalog, None)
}

pub fn bake_scene_wscn_with_catalogs(
    scene: &Scene,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.write_all(WSCN_MAGIC)?;
    buf.write_all(&scene.clear_color)?;
    buf.write_u32::<LittleEndian>(scene.entities.len() as u32)?;

    for ent in &scene.entities {
        let name = ent.name.as_bytes();
        if name.len() > u16::MAX as usize {
            bail!("entity name too long: {}", ent.name);
        }
        buf.write_u16::<LittleEndian>(name.len() as u16)?;
        buf.write_all(name)?;

        // Bake world-space pose so the Wii runtime needs no parent chain.
        let t = scene
            .world_transform(&ent.name)
            .unwrap_or_else(|| ent.transform.clone());
        for v in t.translation {
            buf.write_f32::<LittleEndian>(v)?;
        }
        for v in t.scale {
            buf.write_f32::<LittleEndian>(v)?;
        }

        match (
            &ent.components.sprite,
            &ent.components.disc,
            &ent.components.tilemap,
            &ent.components.text,
        ) {
            (Some(sp), _, _, _) if sp.enabled => {
                if ent.components.disc.as_ref().is_some_and(|d| d.enabled) {
                    bail!(
                        "entity '{}': Wii bake supports Sprite or Disc, not both",
                        ent.name
                    );
                }
                let (sheet, uv, catalog_pivot) = resolve_for_bake(&sp.texture, catalog);
                let pivot = sp.effective_pivot(catalog_pivot);
                let idx = pack.texture_index(&sheet).ok_or_else(|| {
                    anyhow::anyhow!(
                        "entity '{}': texture '{}' missing from wpack",
                        ent.name,
                        sheet
                    )
                })?;
                if idx > u16::MAX as usize {
                    bail!("texture index overflow");
                }
                buf.write_u8(KIND_SPRITE)?;
                buf.write_u16::<LittleEndian>(idx as u16)?;
                buf.write_f32::<LittleEndian>(sp.size[0])?;
                buf.write_f32::<LittleEndian>(sp.size[1])?;
                // UV: u0, v0, u1, v1
                buf.write_f32::<LittleEndian>(uv[0])?;
                buf.write_f32::<LittleEndian>(uv[1])?;
                buf.write_f32::<LittleEndian>(uv[0] + uv[2])?;
                buf.write_f32::<LittleEndian>(uv[1] + uv[3])?;
                buf.write_f32::<LittleEndian>(pivot[0])?;
                buf.write_f32::<LittleEndian>(pivot[1])?;
                buf.write_all(&sp.color)?;
                buf.write_f32::<LittleEndian>(sp.z)?;
            }
            (_, Some(d), _, _) if d.enabled => {
                buf.write_u8(KIND_DISC)?;
                buf.write_f32::<LittleEndian>(d.radius)?;
                buf.write_all(&d.color)?;
                buf.write_f32::<LittleEndian>(d.z)?;
            }
            (_, _, Some(tm), _) if tm.enabled => {
                buf.write_u8(KIND_TILEMAP)?;
                write_tilemap_payload(&mut buf, tm, pack, catalog, anims)?;
            }
            (_, _, _, Some(t)) if t.enabled => {
                buf.write_u8(KIND_TEXT)?;
                write_text_payload(&mut buf, t)?;
            }
            _ => {
                if let Some(a) = ent
                    .components
                    .audio_source
                    .as_ref()
                    .filter(|a| a.enabled)
                {
                    buf.write_u8(KIND_AUDIO)?;
                    write_audio_payload(&mut buf, a, pack)?;
                } else {
                    // Animation / GridMover / Camera-only / disabled audio.
                    buf.write_u8(KIND_NONE)?;
                }
            }
        }
    }
    write_audio_table(&mut buf, scene, pack)?;
    Ok(buf)
}

fn resolve_for_bake(
    sprite_id: &str,
    catalog: Option<&SpriteCatalog>,
) -> (String, [f32; 4], [f32; 2]) {
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(sprite_id) {
            return (r.sheet_texture.clone(), r.uv, r.pivot);
        }
    }
    // Legacy: full texture, content UV filled by C player when u1/v1 == 1.
    (sprite_id.to_string(), [0.0, 0.0, 1.0, 1.0], [0.5, 0.5])
}

pub fn write_scene_wscn(path: impl AsRef<Path>, scene: &Scene, pack: &WPack) -> Result<()> {
    write_scene_wscn_with_catalog(path, scene, pack, None)
}

pub fn write_scene_wscn_with_catalog(
    path: impl AsRef<Path>,
    scene: &Scene,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
) -> Result<()> {
    write_scene_wscn_with_catalogs(path, scene, pack, catalog, None)
}

pub fn write_scene_wscn_with_catalogs(
    path: impl AsRef<Path>,
    scene: &Scene,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = bake_scene_wscn_with_catalogs(scene, pack, catalog, anims)?;
    let mut f = File::create(path).with_context(|| format!("create {}", path.display()))?;
    f.write_all(&bytes)?;
    Ok(())
}

/// KIND_TEXT payload (little-endian, after the kind byte):
/// - `u16` UTF-8 byte length + that many bytes (ASCII is enough; truncated to
///   [`WSCN_TEXT_MAX_BYTES`])
/// - `f32` size (glyph cell height in world px; host `Text.size`)
/// - `u8` align (`0` Left, `1` Center, `2` Right — [`crate::scene::SceneTextAlign`])
/// - `u8[4]` color RGBA
/// - `f32` z (order-in-layer)
/// KIND_AUDIO payload and each trailing-table entry body (little-endian):
/// - `u16` wpack audio TOC index ([`WSCN_AUDIO_NO_CLIP`] if empty/missing)
/// - `f32` volume (clamped 0..1)
/// - `u8` play_on_awake (0/1)
fn write_audio_payload(
    buf: &mut Vec<u8>,
    a: &crate::scene::SceneAudioSource,
    pack: &WPack,
) -> Result<()> {
    let idx = resolve_audio_clip(&a.clip, pack);
    buf.write_u16::<LittleEndian>(idx)?;
    buf.write_f32::<LittleEndian>(a.volume.clamp(0.0, 1.0))?;
    buf.write_u8(if a.play_on_awake { 1 } else { 0 })?;
    Ok(())
}

fn resolve_audio_clip(clip: &str, pack: &WPack) -> u16 {
    if clip.trim().is_empty() {
        return WSCN_AUDIO_NO_CLIP;
    }
    pack.audio_index(clip)
        .filter(|&i| i <= u16::MAX as usize)
        .map(|i| i as u16)
        .unwrap_or(WSCN_AUDIO_NO_CLIP)
}

/// Additive scene tail: every enabled AudioSource (including KIND_AUDIO entities).
/// Omitted entirely when the scene has none so existing length-exact tests stay valid.
fn write_audio_table(buf: &mut Vec<u8>, scene: &Scene, pack: &WPack) -> Result<()> {
    let entries: Vec<(usize, &crate::scene::SceneAudioSource)> = scene
        .entities
        .iter()
        .enumerate()
        .filter_map(|(i, ent)| {
            ent.components
                .audio_source
                .as_ref()
                .filter(|a| a.enabled)
                .map(|a| (i, a))
        })
        .collect();
    if entries.is_empty() {
        return Ok(());
    }
    if entries.len() > u16::MAX as usize {
        bail!("too many AudioSource components for WSCN");
    }
    buf.write_u16::<LittleEndian>(entries.len() as u16)?;
    for (i, a) in entries {
        if i > u16::MAX as usize {
            bail!("entity index overflow in audio table");
        }
        buf.write_u16::<LittleEndian>(i as u16)?;
        write_audio_payload(buf, a, pack)?;
    }
    Ok(())
}

fn write_text_payload(buf: &mut Vec<u8>, t: &crate::scene::SceneText) -> Result<()> {
    let bytes = t.text.as_bytes();
    let n = bytes.len().min(WSCN_TEXT_MAX_BYTES);
    buf.write_u16::<LittleEndian>(n as u16)?;
    buf.write_all(&bytes[..n])?;
    buf.write_f32::<LittleEndian>(t.size)?;
    buf.write_u8(t.align.to_wscn())?;
    buf.write_all(&t.color)?;
    buf.write_f32::<LittleEndian>(t.z)?;
    Ok(())
}

/// KIND_TILEMAP payload (little-endian, after the kind byte):
/// - `u32` length of the rest of this blob (C can skip unknown tails)
/// - `f32` cell, origin x/y · `u16` width/height · `f32` z · `u32` n
/// - `u16` cells `[n]` row-major · packed solid bits `ceil(n/8)`
/// - palette (appended; older bakes omit this and GX uses the default wall tint):
///   - `u16` pal_n
///   - per entry: `u16` id, `u8[4]` RGBA, `u16` tex ([`WSCN_TILE_NO_TEX`] = color quad),
///     `f32` u0,v0,u1,v1, `u8` auto_mode (0 off / 1 id / 2 solid),
///     `u16` auto_bits (NESW mask slots 0..=15), then that many (tex+uv),
///     `u8` frame_n, `f32` fps, then `frame_n` (tex+uv) anim frames (frame 0 is
///     the static/base texture; GX ticks when frame_n > 1)
fn write_tilemap_payload(
    buf: &mut Vec<u8>,
    tm: &crate::scene::SceneTilemap,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    let n = (tm.width as usize).saturating_mul(tm.height as usize);
    let solid_bytes = (n + 7) / 8;
    let mut payload = Vec::new();
    payload.write_f32::<LittleEndian>(tm.cell)?;
    payload.write_f32::<LittleEndian>(tm.origin[0])?;
    payload.write_f32::<LittleEndian>(tm.origin[1])?;
    payload.write_u16::<LittleEndian>(tm.width.min(u16::MAX as u32) as u16)?;
    payload.write_u16::<LittleEndian>(tm.height.min(u16::MAX as u32) as u16)?;
    payload.write_f32::<LittleEndian>(tm.z)?;
    payload.write_u32::<LittleEndian>(n as u32)?;
    for i in 0..n {
        let id = tm.cells.get(i).copied().unwrap_or(0);
        payload.write_u16::<LittleEndian>(id)?;
    }
    let mut bits = vec![0u8; solid_bytes];
    for (i, flag) in tm.solid.iter().take(n).enumerate() {
        if *flag != 0 {
            bits[i / 8] |= 1 << (i % 8);
        }
    }
    payload.write_all(&bits)?;
    write_tilemap_palette(&mut payload, tm, pack, catalog, anims)?;
    buf.write_u32::<LittleEndian>(payload.len() as u32)?;
    buf.write_all(&payload)?;
    Ok(())
}

fn resolve_tile_tex(
    name: &str,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
) -> Option<(u16, [f32; 4])> {
    let (sheet, uv, _) = resolve_for_bake(name, catalog);
    let idx = pack.texture_index(&sheet)?;
    if idx > u16::MAX as usize {
        return None;
    }
    Some((idx as u16, [uv[0], uv[1], uv[0] + uv[2], uv[1] + uv[3]]))
}

fn write_tex_uv(payload: &mut Vec<u8>, tex: u16, uv: [f32; 4]) -> Result<()> {
    payload.write_u16::<LittleEndian>(tex)?;
    for v in uv {
        payload.write_f32::<LittleEndian>(v)?;
    }
    Ok(())
}

fn write_tilemap_palette(
    payload: &mut Vec<u8>,
    tm: &crate::scene::SceneTilemap,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    let entries: Vec<&crate::scene::SceneTilePalette> = tm.palette.iter().collect();
    if entries.is_empty() {
        payload.write_u16::<LittleEndian>(1)?;
        write_palette_entry(
            payload,
            1,
            WSCN_TILE_DEFAULT_COLOR,
            None,
            WSCN_TILEMAP_AUTO_OFF,
            &[],
        )?;
        payload.write_u8(0)?;
        payload.write_f32::<LittleEndian>(0.0)?;
        return Ok(());
    }
    payload.write_u16::<LittleEndian>(entries.len().min(u16::MAX as usize) as u16)?;
    for pal in entries {
        let mut base = pal
            .sprite
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|name| resolve_tile_tex(name, pack, catalog));

        let mut frames: Vec<(u16, [f32; 4])> = Vec::new();
        let mut fps = 0.0f32;
        if let Some(clip) = pal.anim_clip() {
            let meta = anims.and_then(|c| c.lookup(clip));
            fps = pal
                .anim_fps
                .filter(|f| *f > 0.0)
                .or_else(|| meta.map(|m| m.fps))
                .unwrap_or(10.0);
            if let Some(meta) = meta {
                for cell in &meta.cells {
                    if let Some(tex) = resolve_tile_tex(cell, pack, catalog) {
                        frames.push(tex);
                    }
                    if frames.len() >= WSCN_TILEMAP_MAX_FRAMES {
                        break;
                    }
                }
            }
            if frames.is_empty() {
                if let Some(tex) = base {
                    frames.push(tex);
                }
            } else {
                base = Some(frames[0]);
            }
        }
        if frames.len() <= 1 {
            frames.clear();
            fps = 0.0;
        }

        let auto_mode = match pal.auto_tile {
            Some(crate::scene::SceneAutoTile::Id) => WSCN_TILEMAP_AUTO_ID,
            Some(crate::scene::SceneAutoTile::Solid) => WSCN_TILEMAP_AUTO_SOLID,
            None => WSCN_TILEMAP_AUTO_OFF,
        };
        let mut auto_slots = [(WSCN_TILE_NO_TEX, [0.0, 0.0, 1.0, 1.0]); 16];
        if auto_mode != WSCN_TILEMAP_AUTO_OFF {
            let stem = pal
                .sprite
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            for mask in 0..16 {
                let named = pal
                    .auto_sprites
                    .get(mask)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| stem.map(|s| format!("{s}_{mask}")));
                if let Some(name) = named {
                    if let Some(tex) = resolve_tile_tex(&name, pack, catalog) {
                        auto_slots[mask] = tex;
                    }
                }
            }
        }

        write_palette_entry(payload, pal.id, pal.color, base, auto_mode, &auto_slots)?;
        payload.write_u8(frames.len() as u8)?;
        payload.write_f32::<LittleEndian>(fps)?;
        for (tex, uv) in &frames {
            write_tex_uv(payload, *tex, *uv)?;
        }
    }
    Ok(())
}

fn write_palette_entry(
    payload: &mut Vec<u8>,
    id: u16,
    color: [u8; 4],
    base: Option<(u16, [f32; 4])>,
    auto_mode: u8,
    auto_slots: &[(u16, [f32; 4])],
) -> Result<()> {
    payload.write_u16::<LittleEndian>(id)?;
    payload.write_all(&color)?;
    let (tex, uv) = base.unwrap_or((WSCN_TILE_NO_TEX, [0.0, 0.0, 1.0, 1.0]));
    write_tex_uv(payload, tex, uv)?;
    payload.write_u8(auto_mode)?;
    let mut auto_bits: u16 = 0;
    if auto_mode != WSCN_TILEMAP_AUTO_OFF {
        for (mask, (atex, _)) in auto_slots.iter().enumerate().take(16) {
            if *atex != WSCN_TILE_NO_TEX {
                auto_bits |= 1 << mask;
            }
        }
    }
    payload.write_u16::<LittleEndian>(auto_bits)?;
    if auto_mode != WSCN_TILEMAP_AUTO_OFF {
        for (mask, (atex, auv)) in auto_slots.iter().enumerate().take(16) {
            if auto_bits & (1 << mask) != 0 {
                write_tex_uv(payload, *atex, *auv)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_component_sprite, add_entity, MutateOpts};
    use crate::scene::{Scene, SceneTextAlign};
    use crate::tilemap::{add_component_tilemap, tilemap_stamp_ascii};

    #[test]
    fn bake_writes_clear_color() {
        use crate::mutate::set_scene_clear;

        let mut scene = Scene::new("t");
        set_scene_clear(&mut scene, [1, 2, 3]);
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        assert_eq!(&bytes[0..8], b"WSCN0003");
        assert_eq!(&bytes[8..12], &[1, 2, 3, 255]);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0);
    }

    fn read_f32(bytes: &[u8], i: &mut usize) -> f32 {
        let v = f32::from_le_bytes(bytes[*i..*i + 4].try_into().unwrap());
        *i += 4;
        v
    }

    fn read_u16(bytes: &[u8], i: &mut usize) -> u16 {
        let v = u16::from_le_bytes(bytes[*i..*i + 2].try_into().unwrap());
        *i += 2;
        v
    }

    fn skip_to_kind(bytes: &[u8]) -> usize {
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let i = 8 + 4 + 4;
        let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i + 2 + nlen + 6 * 4
    }

    #[test]
    fn bake_tilemap_kind_and_magic() {
        let mut scene = Scene::new("maze");
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_tilemap(&mut scene, "Maze", 3, 2, 16.0).unwrap();
        tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "###\n#.#").unwrap();
        let pack = WPack::new();
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_TILEMAP);
        i += 1;
        let plen = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        assert!(plen > 0);
        assert_eq!(i + plen, bytes.len());

        let cell = read_f32(&bytes, &mut i);
        let ox = read_f32(&bytes, &mut i);
        let oy = read_f32(&bytes, &mut i);
        let w = read_u16(&bytes, &mut i);
        let h = read_u16(&bytes, &mut i);
        let z = read_f32(&bytes, &mut i);
        let n = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        i += 4;
        assert!((cell - 16.0).abs() < 1e-4);
        assert!((ox).abs() < 1e-4 && oy.abs() < 1e-4);
        assert_eq!((w, h, n), (3, 2, 6));
        assert!((z + 1.0).abs() < 1e-4);
        let cells: Vec<u16> = (0..6).map(|_| read_u16(&bytes, &mut i)).collect();
        assert_eq!(cells, vec![1, 1, 1, 1, 0, 1]);
        let solid_bytes = (6 + 7) / 8;
        i += solid_bytes;
        let pal_n = read_u16(&bytes, &mut i);
        assert_eq!(pal_n, 1);
        let pal_id = read_u16(&bytes, &mut i);
        assert_eq!(pal_id, 1);
        assert_eq!(&bytes[i..i + 4], &WSCN_TILE_DEFAULT_COLOR);
        i += 4;
        let tex = read_u16(&bytes, &mut i);
        assert_eq!(tex, WSCN_TILE_NO_TEX);
        i += 16; // uv
        assert_eq!(bytes[i], WSCN_TILEMAP_AUTO_OFF);
        i += 1;
        let auto_bits = read_u16(&bytes, &mut i);
        assert_eq!(auto_bits, 0);
        assert_eq!(bytes[i], 0); // frame_n
    }

    #[test]
    fn bake_tilemap_palette_sprite_and_autotile() {
        use crate::tilemap::{tilemap_set_cell, tilemap_set_palette, TilePaletteOpts};
        use wiimaker_assets::{ResolvedSprite, SpriteCatalog};

        let mut scene = Scene::new("maze");
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(8.0),
                y: Some(16.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_tilemap(&mut scene, "Maze", 2, 1, 16.0).unwrap();
        tilemap_set_palette(
            &mut scene,
            "Maze",
            2,
            &TilePaletteOpts {
                sprite: Some("wall".into()),
                auto_tile: Some("id".into()),
                auto_sprites: Some(vec![String::new(), String::new(), "wall_e".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        tilemap_set_cell(&mut scene, "Maze", 0, 0, 2, true).unwrap();
        tilemap_set_cell(&mut scene, "Maze", 1, 0, 2, true).unwrap();

        let pack = dummy_pack("sheet");

        let mut cat = SpriteCatalog::empty();
        cat.insert(
            "wall",
            ResolvedSprite {
                sheet_texture: "sheet".into(),
                uv: [0.0, 0.0, 0.5, 1.0],
                pivot: [0.5, 0.5],
                pixel_size: [8.0, 16.0],
                is_cell: true,
            },
        );
        cat.insert(
            "wall_e",
            ResolvedSprite {
                sheet_texture: "sheet".into(),
                uv: [0.5, 0.0, 0.5, 1.0],
                pivot: [0.5, 0.5],
                pixel_size: [8.0, 16.0],
                is_cell: true,
            },
        );

        let bytes = bake_scene_wscn_with_catalog(&scene, &pack, Some(&cat)).unwrap();
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_TILEMAP);
        i += 1;
        let plen = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        let end = i + plen;
        let _cell = read_f32(&bytes, &mut i);
        let _ox = read_f32(&bytes, &mut i);
        let _oy = read_f32(&bytes, &mut i);
        let w = read_u16(&bytes, &mut i);
        let h = read_u16(&bytes, &mut i);
        let _z = read_f32(&bytes, &mut i);
        let n = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        i += 4;
        assert_eq!((w, h, n), (2, 1, 2));
        i += 2 * 2; // cells
        i += (2 + 7) / 8; // solid
        let pal_n = read_u16(&bytes, &mut i);
        assert!(pal_n >= 1);
        // Default id=1 plus painted id=2 (set-palette adds/updates).
        let mut found_wall = false;
        for _ in 0..pal_n {
            let id = read_u16(&bytes, &mut i);
            i += 4; // color
            let tex = read_u16(&bytes, &mut i);
            let u0 = read_f32(&bytes, &mut i);
            let v0 = read_f32(&bytes, &mut i);
            let u1 = read_f32(&bytes, &mut i);
            let v1 = read_f32(&bytes, &mut i);
            let auto_mode = bytes[i];
            i += 1;
            let auto_bits = read_u16(&bytes, &mut i);
            for mask in 0..16 {
                if auto_bits & (1 << mask) != 0 {
                    let atex = read_u16(&bytes, &mut i);
                    let au0 = read_f32(&bytes, &mut i);
                    i += 12;
                    if id == 2 && mask == 2 {
                        assert_eq!(atex, 0);
                        assert!((au0 - 0.5).abs() < 1e-4);
                    }
                }
            }
            let frame_n = bytes[i];
            i += 1;
            let _fps = read_f32(&bytes, &mut i);
            for _ in 0..frame_n {
                i += 2 + 16;
            }
            if id == 2 {
                found_wall = true;
                assert_eq!(tex, 0);
                assert!((u0).abs() < 1e-4 && v0.abs() < 1e-4);
                assert!((u1 - 0.5).abs() < 1e-4 && (v1 - 1.0).abs() < 1e-4);
                assert_eq!(auto_mode, WSCN_TILEMAP_AUTO_ID);
                assert_eq!(auto_bits & (1 << 2), 1 << 2);
            }
        }
        assert!(found_wall);
        assert_eq!(i, end);
    }

    #[test]
    fn bake_tilemap_anim_frames() {
        use crate::tilemap::{tilemap_set_cell, tilemap_set_palette, TilePaletteOpts};
        use wiimaker_assets::{write_anim_clip, AnimClipCatalog, ResolvedSprite, SpriteCatalog};

        let dir =
            std::env::temp_dir().join(format!("wiimaker-wscn-tile-anim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        write_anim_clip(
            &dir,
            "water",
            vec!["water_a".into(), "water_b".into()],
            8.0,
            true,
        )
        .unwrap();
        let anims = AnimClipCatalog::load_dir(&dir).unwrap();

        let mut scene = Scene::new("maze");
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_tilemap(&mut scene, "Maze", 1, 1, 16.0).unwrap();
        tilemap_set_palette(
            &mut scene,
            "Maze",
            3,
            &TilePaletteOpts {
                sprite: Some("water_a".into()),
                anim: Some("water".into()),
                anim_fps: Some(8.0),
                ..Default::default()
            },
        )
        .unwrap();
        tilemap_set_cell(&mut scene, "Maze", 0, 0, 3, false).unwrap();

        let mut pack = WPack::new();
        pack.textures.push(wiimaker_assets::PackedTexture {
            name: "atlas".into(),
            width: 16,
            height: 8,
            rgba16: vec![0; 256],
        });
        let mut cat = SpriteCatalog::empty();
        cat.insert(
            "water_a",
            ResolvedSprite {
                sheet_texture: "atlas".into(),
                uv: [0.0, 0.0, 0.5, 1.0],
                pivot: [0.5, 0.5],
                pixel_size: [8.0, 8.0],
                is_cell: true,
            },
        );
        cat.insert(
            "water_b",
            ResolvedSprite {
                sheet_texture: "atlas".into(),
                uv: [0.5, 0.0, 0.5, 1.0],
                pivot: [0.5, 0.5],
                pixel_size: [8.0, 8.0],
                is_cell: true,
            },
        );

        let bytes = bake_scene_wscn_with_catalogs(&scene, &pack, Some(&cat), Some(&anims)).unwrap();
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_TILEMAP);
        i += 1;
        let plen = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        let end = i + plen;
        i += 4 + 4 + 4 + 2 + 2 + 4 + 4; // cell origin w h z n
        i += 2; // one cell
        i += 1; // solid
        let pal_n = read_u16(&bytes, &mut i);
        let mut found = false;
        for _ in 0..pal_n {
            let id = read_u16(&bytes, &mut i);
            i += 4;
            let tex = read_u16(&bytes, &mut i);
            let u0 = read_f32(&bytes, &mut i);
            i += 12;
            let _auto_mode = bytes[i];
            i += 1;
            let auto_bits = read_u16(&bytes, &mut i);
            for mask in 0..16 {
                if auto_bits & (1 << mask) != 0 {
                    i += 2 + 16;
                }
            }
            let frame_n = bytes[i];
            i += 1;
            let fps = read_f32(&bytes, &mut i);
            let mut frame_u0 = Vec::new();
            for _ in 0..frame_n {
                let _ft = read_u16(&bytes, &mut i);
                frame_u0.push(read_f32(&bytes, &mut i));
                i += 12;
            }
            if id == 3 {
                found = true;
                assert_eq!(tex, 0);
                assert!(u0.abs() < 1e-4);
                assert_eq!(frame_n, 2);
                assert!((fps - 8.0).abs() < 1e-4);
                assert_eq!(frame_u0.len(), 2);
                assert!(frame_u0[0].abs() < 1e-4);
                assert!((frame_u0[1] - 0.5).abs() < 1e-4);
            }
        }
        assert!(found);
        assert_eq!(i, end);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn dummy_pack(name: &str) -> WPack {
        let mut pack = WPack::new();
        pack.textures.push(wiimaker_assets::PackedTexture {
            name: name.into(),
            width: 8,
            height: 8,
            rgba16: vec![0; 128],
        });
        pack
    }

    fn sprite_pivot_from_wscn(bytes: &[u8]) -> [f32; 2] {
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let mut i = 8 + 4 + 4;
        let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2 + nlen + 6 * 4;
        assert_eq!(bytes[i], KIND_SPRITE);
        i += 1; // kind
        i += 2; // tex idx
        i += 8; // size xy
        i += 16; // uv
        let px = f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let py = f32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap());
        [px, py]
    }

    #[test]
    fn bake_writes_effective_pivot() {
        use crate::mutate::add_component_sprite;
        use wiimaker_assets::{ResolvedSprite, SpriteCatalog};

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Hero",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_sprite(&mut scene, "Hero", "hero_2", [16.0, 16.0]).unwrap();
        let pack = dummy_pack("sheet");

        let mut cat = SpriteCatalog::empty();
        cat.insert(
            "hero_2",
            ResolvedSprite {
                sheet_texture: "sheet".into(),
                uv: [0.0, 0.0, 1.0, 1.0],
                pivot: [0.25, 0.75],
                pixel_size: [16.0, 16.0],
                is_cell: true,
            },
        );

        let bytes = bake_scene_wscn_with_catalog(&scene, &pack, Some(&cat)).unwrap();
        let p = sprite_pivot_from_wscn(&bytes);
        assert!((p[0] - 0.25).abs() < 1e-4 && (p[1] - 0.75).abs() < 1e-4);

        scene
            .entities
            .iter_mut()
            .find(|e| e.name == "Hero")
            .unwrap()
            .components
            .sprite
            .as_mut()
            .unwrap()
            .pivot = Some([0.0, 1.0]);
        let bytes = bake_scene_wscn_with_catalog(&scene, &pack, Some(&cat)).unwrap();
        let p = sprite_pivot_from_wscn(&bytes);
        assert!((p[0] - 0.0).abs() < 1e-4 && (p[1] - 1.0).abs() < 1e-4);
        assert_eq!(&bytes[0..8], b"WSCN0003");
    }

    #[test]
    fn bake_text_kind_and_payload() {
        use crate::mutate::add_component_text;

        let mut scene = Scene::new("hud");
        add_entity(
            &mut scene,
            "Score",
            &MutateOpts {
                x: Some(40.0),
                y: Some(12.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_text(
            &mut scene,
            "Score",
            "Hi",
            16.0,
            [255, 32, 64, 200],
            SceneTextAlign::Center,
        )
        .unwrap();
        scene.entities[0].components.text.as_mut().unwrap().z = 3.5;

        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_TEXT);
        i += 1;
        let slen = u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap()) as usize;
        i += 2;
        assert_eq!(slen, 2);
        assert_eq!(&bytes[i..i + 2], b"Hi");
        i += 2;
        let size = f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        i += 4;
        assert!((size - 16.0).abs() < 1e-4);
        assert_eq!(bytes[i], 1); // Center
        i += 1;
        assert_eq!(&bytes[i..i + 4], &[255, 32, 64, 200]);
        i += 4;
        let z = f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        assert!((z - 3.5).abs() < 1e-4);
        assert_eq!(i + 4, bytes.len());
    }

    #[test]
    fn bake_sprite_wins_over_text() {
        use crate::mutate::add_component_text;

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Hero",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_sprite(&mut scene, "Hero", "sheet", [16.0, 16.0]).unwrap();
        add_component_text(
            &mut scene,
            "Hero",
            "nope",
            8.0,
            [255, 255, 255, 255],
            SceneTextAlign::Left,
        )
        .unwrap();
        let bytes = bake_scene_wscn(&scene, &dummy_pack("sheet")).unwrap();
        assert_eq!(bytes[skip_to_kind(&bytes)], KIND_SPRITE);
    }

    #[test]
    fn bake_disc_wins_over_text() {
        use crate::mutate::{add_component_disc, add_component_text};

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Orb",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_disc(&mut scene, "Orb", 10.0, [255, 0, 0, 255]).unwrap();
        add_component_text(
            &mut scene,
            "Orb",
            "nope",
            8.0,
            [255, 255, 255, 255],
            SceneTextAlign::Left,
        )
        .unwrap();
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        assert_eq!(bytes[skip_to_kind(&bytes)], KIND_DISC);
    }

    #[test]
    fn bake_text_preferred_over_none_with_camera() {
        use crate::mutate::{add_component_camera, add_component_text};

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Hud",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_camera(&mut scene, "Hud", true).unwrap();
        add_component_text(
            &mut scene,
            "Hud",
            "GO",
            8.0,
            [255, 255, 255, 255],
            SceneTextAlign::Right,
        )
        .unwrap();
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        let i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_TEXT);
        let slen = u16::from_le_bytes(bytes[i + 1..i + 3].try_into().unwrap()) as usize;
        assert_eq!(&bytes[i + 3..i + 3 + slen], b"GO");
        assert_eq!(bytes[i + 3 + slen + 4], 2); // Right after size f32
    }

    fn pack_with_beep() -> WPack {
        let mut pack = WPack::new();
        pack.audio.push(wiimaker_assets::PackedAudio {
            name: "beep".into(),
            sample_rate: 22050,
            channels: 1,
            pcm: vec![0, 1, -1],
        });
        pack
    }

    fn read_audio_payload(bytes: &[u8], i: &mut usize) -> (u16, f32, u8) {
        let clip = read_u16(bytes, i);
        let vol = read_f32(bytes, i);
        let awake = bytes[*i];
        *i += 1;
        (clip, vol, awake)
    }

    #[test]
    fn bake_audio_kind_and_payload() {
        use crate::mutate::add_component_audio_source;

        let mut scene = Scene::new("sfx");
        add_entity(
            &mut scene,
            "Beep",
            &MutateOpts {
                x: Some(10.0),
                y: Some(20.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_audio_source(&mut scene, "Beep", "beep", 0.5, true).unwrap();

        let pack = pack_with_beep();
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_AUDIO);
        i += 1;
        let (clip, vol, awake) = read_audio_payload(&bytes, &mut i);
        assert_eq!(clip, 0);
        assert!((vol - 0.5).abs() < 1e-4);
        assert_eq!(awake, 1);

        // Trailing table also lists this entity (C applies after kinds).
        let n = read_u16(&bytes, &mut i);
        assert_eq!(n, 1);
        let ei = read_u16(&bytes, &mut i);
        assert_eq!(ei, 0);
        let (tclip, tvol, tawake) = read_audio_payload(&bytes, &mut i);
        assert_eq!((tclip, tawake), (clip, awake));
        assert!((tvol - vol).abs() < 1e-4);
        assert_eq!(i, bytes.len());
    }

    #[test]
    fn bake_missing_audio_clip_is_no_clip() {
        use crate::mutate::add_component_audio_source;

        let mut scene = Scene::new("sfx");
        add_entity(
            &mut scene,
            "Ghost",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_audio_source(&mut scene, "Ghost", "nope", 1.0, false).unwrap();
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_AUDIO);
        i += 1;
        let (clip, vol, awake) = read_audio_payload(&bytes, &mut i);
        assert_eq!(clip, WSCN_AUDIO_NO_CLIP);
        assert!((vol - 1.0).abs() < 1e-4);
        assert_eq!(awake, 0);
        assert_eq!(read_u16(&bytes, &mut i), 1); // table n
    }

    #[test]
    fn bake_sprite_wins_over_audio_table() {
        use crate::mutate::add_component_audio_source;

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Hero",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_sprite(&mut scene, "Hero", "sheet", [16.0, 16.0]).unwrap();
        add_component_audio_source(&mut scene, "Hero", "beep", 0.25, true).unwrap();
        let mut pack = dummy_pack("sheet");
        pack.audio.push(wiimaker_assets::PackedAudio {
            name: "beep".into(),
            sample_rate: 22050,
            channels: 1,
            pcm: vec![0],
        });
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        let mut i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_SPRITE);
        i += 1;
        i += 2 + 8 + 16 + 8 + 4 + 4; // tex size uv pivot color z
        let n = read_u16(&bytes, &mut i);
        assert_eq!(n, 1);
        assert_eq!(read_u16(&bytes, &mut i), 0);
        let (clip, vol, awake) = read_audio_payload(&bytes, &mut i);
        assert_eq!(clip, 0);
        assert!((vol - 0.25).abs() < 1e-4);
        assert_eq!(awake, 1);
        assert_eq!(i, bytes.len());
    }

    #[test]
    fn bake_disabled_audio_omits_kind_and_table() {
        use crate::mutate::add_component_audio_source;

        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Quiet",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_audio_source(&mut scene, "Quiet", "beep", 1.0, true).unwrap();
        scene.entities[0].components.audio_source.as_mut().unwrap().enabled = false;
        let bytes = bake_scene_wscn(&scene, &pack_with_beep()).unwrap();
        let i = skip_to_kind(&bytes);
        assert_eq!(bytes[i], KIND_NONE);
        assert_eq!(i + 1, bytes.len());
    }
}
