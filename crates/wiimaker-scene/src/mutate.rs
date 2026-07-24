//! Shared scene mutations used by CLI and editor.

use anyhow::{bail, Result};

use crate::scene::{
    EntityData, Scene, SceneComponents, SceneDisc, SceneSprite, SceneTransform,
};

#[derive(Clone, Debug, Default)]
pub struct MutateOpts {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub sprite: Option<String>,
    pub sprite_w: Option<f32>,
    pub sprite_h: Option<f32>,
    pub radius: Option<f32>,
    pub color: Option<[u8; 4]>,
}

pub fn set_scene_clear(scene: &mut Scene, rgb: [u8; 3]) {
    scene.clear_color = [rgb[0], rgb[1], rgb[2], 255];
}

pub fn add_entity(scene: &mut Scene, name: &str, opts: &MutateOpts) -> Result<()> {
    if scene.entities.iter().any(|e| e.name == name) {
        bail!("entity '{name}' already exists");
    }
    let x = opts.x.unwrap_or(320.0);
    let y = opts.y.unwrap_or(240.0);
    let mut components = SceneComponents::default();
    if let Some(tex) = &opts.sprite {
        components.sprite = Some(SceneSprite {
            texture: tex.clone(),
            size: [
                opts.sprite_w.unwrap_or(32.0),
                opts.sprite_h.unwrap_or(32.0),
            ],
            color: opts.color.unwrap_or([255, 255, 255, 255]),
            z: 0.0,
        });
    }
    if let Some(radius) = opts.radius {
        components.disc = Some(SceneDisc {
            radius,
            color: opts.color.unwrap_or([72, 210, 160, 255]),
            z: 0.0,
        });
    }
    scene.entities.push(EntityData {
        name: name.to_string(),
        transform: SceneTransform::from_xy(x, y),
        components,
        tag: 0,
    });
    Ok(())
}

pub fn remove_entity(scene: &mut Scene, name: &str) -> Result<()> {
    let before = scene.entities.len();
    scene.entities.retain(|e| e.name != name);
    if scene.entities.len() == before {
        bail!("entity '{name}' not found");
    }
    Ok(())
}

pub fn set_entity_transform(
    scene: &mut Scene,
    name: &str,
    x: Option<f32>,
    y: Option<f32>,
) -> Result<()> {
    let ent = scene
        .entities
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    if let Some(x) = x {
        ent.transform.translation[0] = x;
    }
    if let Some(y) = y {
        ent.transform.translation[1] = y;
    }
    Ok(())
}

pub fn add_component_sprite(
    scene: &mut Scene,
    name: &str,
    texture: &str,
    size: [f32; 2],
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.sprite = Some(SceneSprite {
        texture: texture.to_string(),
        size,
        color: [255, 255, 255, 255],
        z: 0.0,
    });
    Ok(())
}

pub fn add_component_disc(scene: &mut Scene, name: &str, radius: f32, color: [u8; 4]) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.disc = Some(SceneDisc {
        radius,
        color,
        z: 0.0,
    });
    Ok(())
}

fn find_mut<'a>(scene: &'a mut Scene, name: &str) -> Result<&'a mut EntityData> {
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))
}
