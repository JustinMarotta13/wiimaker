//! Compact binary scene for Wii embedding (`scene.wscn`).
//!
//! Texture names are resolved to wpack indices at bake time so the C runtime
//! never parses JSON or string-matches asset names per frame.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use wiimaker_assets::WPack;

use crate::scene::Scene;

pub const WSCN_MAGIC: &[u8; 8] = b"WSCN0001";

pub const KIND_NONE: u8 = 0;
pub const KIND_SPRITE: u8 = 1;
pub const KIND_DISC: u8 = 2;

/// Bake a scene against a cooked pack into little-endian `scene.wscn` bytes.
pub fn bake_scene_wscn(scene: &Scene, pack: &WPack) -> Result<Vec<u8>> {
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

        let t = &ent.transform;
        for v in t.translation {
            buf.write_f32::<LittleEndian>(v)?;
        }
        for v in t.scale {
            buf.write_f32::<LittleEndian>(v)?;
        }

        match (&ent.components.sprite, &ent.components.disc) {
            (Some(sp), None) => {
                let idx = pack.texture_index(&sp.texture).ok_or_else(|| {
                    anyhow::anyhow!(
                        "entity '{}': texture '{}' missing from wpack",
                        ent.name,
                        sp.texture
                    )
                })?;
                if idx > u16::MAX as usize {
                    bail!("texture index overflow");
                }
                buf.write_u8(KIND_SPRITE)?;
                buf.write_u16::<LittleEndian>(idx as u16)?;
                buf.write_f32::<LittleEndian>(sp.size[0])?;
                buf.write_f32::<LittleEndian>(sp.size[1])?;
                buf.write_all(&sp.color)?;
                buf.write_f32::<LittleEndian>(sp.z)?;
            }
            (None, Some(d)) => {
                buf.write_u8(KIND_DISC)?;
                buf.write_f32::<LittleEndian>(d.radius)?;
                buf.write_all(&d.color)?;
                buf.write_f32::<LittleEndian>(d.z)?;
            }
            (None, None) => {
                buf.write_u8(KIND_NONE)?;
            }
            (Some(_), Some(_)) => {
                bail!(
                    "entity '{}': Wii bake supports Sprite or Disc, not both",
                    ent.name
                );
            }
        }
    }
    Ok(buf)
}

pub fn write_scene_wscn(path: impl AsRef<Path>, scene: &Scene, pack: &WPack) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = bake_scene_wscn(scene, pack)?;
    let mut f = File::create(path).with_context(|| format!("create {}", path.display()))?;
    f.write_all(&bytes)?;
    Ok(())
}
