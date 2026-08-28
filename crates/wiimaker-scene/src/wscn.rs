//! Compact binary scene for Wii embedding (`scene.wscn`).
//!
//! Texture names / sprite cells are resolved to wpack indices + UV/pivot at bake
//! time so the C runtime never parses JSON or string-matches asset names.

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
        ) {
            (Some(sp), _, _) if sp.enabled => {
                if ent.components.disc.as_ref().is_some_and(|d| d.enabled) {
                    bail!(
                        "entity '{}': Wii bake supports Sprite or Disc, not both",
                        ent.name
                    );
                }
                let (sheet, uv, pivot) = resolve_for_bake(&sp.texture, catalog);
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
            (_, Some(d), _) if d.enabled => {
                buf.write_u8(KIND_DISC)?;
                buf.write_f32::<LittleEndian>(d.radius)?;
                buf.write_all(&d.color)?;
                buf.write_f32::<LittleEndian>(d.z)?;
            }
            (_, _, Some(tm)) if tm.enabled => {
                buf.write_u8(KIND_TILEMAP)?;
                write_tilemap_payload(&mut buf, tm)?;
            }
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
    use crate::mutate::{add_entity, MutateOpts};
    use crate::scene::Scene;
    use crate::tilemap::{add_component_tilemap, tilemap_stamp_ascii};

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
}
