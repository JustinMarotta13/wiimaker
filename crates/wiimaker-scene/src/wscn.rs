//! Compact binary scene for Wii embedding (`scene.wscn`).
//!
//! Texture names / sprite cells are resolved to wpack indices + UV/pivot at bake
//! time so the C runtime never parses JSON or string-matches asset names.
//!
//! Host-first Sorting Layers are **not** packed: WSCN0003 still stores raw `z`
//! (order-in-layer). The C player sorts by `z` only. Do not bump the magic for
//! this feature.
//!
//! `KIND_TEXT = 4` is packed under the same magic (sprite / disc / tilemap still
//! win when those components are enabled). Tilemaps remain length-prefixed and
//! skipped by the C player.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use wiimaker_assets::{SpriteCatalog, WPack};

use crate::scene::Scene;

/// Format version with UV rect + pivot on sprites.
pub const WSCN_MAGIC: &[u8; 8] = b"WSCN0003";

pub const KIND_NONE: u8 = 0;
pub const KIND_SPRITE: u8 = 1;
pub const KIND_DISC: u8 = 2;
pub const KIND_TILEMAP: u8 = 3;
/// HUD bitmap string (same 8×8 bits as `wiimaker-assets` font.rs). Magic stays WSCN0003.
pub const KIND_TEXT: u8 = 4;

/// Reasonable UTF-8 byte cap for Wii C `Entity.text` (loader still skips a longer bake).
pub const WSCN_TEXT_MAX_BYTES: usize = 255;

/// Bake a scene against a cooked pack into little-endian `scene.wscn` bytes.
pub fn bake_scene_wscn(scene: &Scene, pack: &WPack) -> Result<Vec<u8>> {
    bake_scene_wscn_with_catalog(scene, pack, None)
}

pub fn bake_scene_wscn_with_catalog(
    scene: &Scene,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
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
                write_tilemap_payload(&mut buf, tm)?;
            }
            (_, _, _, Some(t)) if t.enabled => {
                buf.write_u8(KIND_TEXT)?;
                write_text_payload(&mut buf, t)?;
            }
            // Animation / AudioSource / GridMover / Camera-only: host-first.
            _ => {
                buf.write_u8(KIND_NONE)?;
            }
        }
    }
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
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = bake_scene_wscn_with_catalog(scene, pack, catalog)?;
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

fn write_tilemap_payload(buf: &mut Vec<u8>, tm: &crate::scene::SceneTilemap) -> Result<()> {
    let n = (tm.width as usize).saturating_mul(tm.height as usize);
    let solid_bytes = (n + 7) / 8;
    // payload after the length prefix: cell, origin xy, w/h, z, n, cells, solid bits
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
    buf.write_u32::<LittleEndian>(payload.len() as u32)?;
    buf.write_all(&payload)?;
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
        // skip magic(8) + clear(4) + count(4) + name_len(2) + "Maze"(4) + xf 6xf32
        let mut i = 8 + 4 + 4;
        let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2 + nlen + 6 * 4;
        assert_eq!(bytes[i], KIND_TILEMAP);
        i += 1;
        let plen = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        assert!(plen > 0);
        assert_eq!(i + 4 + plen, bytes.len());
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

    fn skip_to_kind(bytes: &[u8]) -> usize {
        assert_eq!(&bytes[0..8], b"WSCN0003");
        let i = 8 + 4 + 4;
        let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i + 2 + nlen + 6 * 4
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
}
