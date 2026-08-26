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
pub const WSCN_MAGIC: &[u8; 8] = b"WSCN0002";

pub const KIND_NONE: u8 = 0;
pub const KIND_SPRITE: u8 = 1;
pub const KIND_DISC: u8 = 2;

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

        match (&ent.components.sprite, &ent.components.disc) {
            (Some(sp), _) if sp.enabled => {
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
            (_, Some(d)) if d.enabled => {
                buf.write_u8(KIND_DISC)?;
                buf.write_f32::<LittleEndian>(d.radius)?;
                buf.write_all(&d.color)?;
                buf.write_f32::<LittleEndian>(d.z)?;
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
