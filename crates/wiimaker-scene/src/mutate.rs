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

/// Suggest a unique entity name: `base`, or `base_1`, `base_2`, …
pub fn unique_entity_name(scene: &Scene, base: &str) -> String {
    if !name_exists(scene, base) {
        return base.to_string();
    }
    let mut i = 1u32;
    loop {
        let candidate = format!("{base}_{i}");
        if !name_exists(scene, &candidate) {
            return candidate;
        }
        i = i.saturating_add(1);
        if i == 0 {
            // absurd overflow guard
            return format!("{base}_{}", scene.entities.len());
        }
    }
}

fn name_exists(scene: &Scene, name: &str) -> bool {
    scene.entities.iter().any(|e| e.name == name)
}

pub fn add_entity(scene: &mut Scene, name: &str, opts: &MutateOpts) -> Result<()> {
    if name_exists(scene, name) {
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

/// Rename an entity in place. Fails if `new` is empty or already taken.
pub fn rename_entity(scene: &mut Scene, old: &str, new: &str) -> Result<()> {
    if new.is_empty() {
        bail!("entity name cannot be empty");
    }
    if old != new && name_exists(scene, new) {
        bail!("entity '{new}' already exists");
    }
    let ent = find_mut(scene, old)?;
    ent.name = new.to_string();
    Ok(())
}

/// Deep-clone an entity with a unique name and a slight position offset.
/// Returns the new entity's name.
pub fn duplicate_entity(scene: &mut Scene, name: &str) -> Result<String> {
    let src = scene
        .entities
        .iter()
        .find(|e| e.name == name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    Ok(insert_entity_clone(scene, &src))
}

/// Insert a clone of `entity` with a unique name and +16,+16 translation offset.
/// Returns the new entity's name. Used by paste and [`duplicate_entity`].
pub fn insert_entity_clone(scene: &mut Scene, entity: &EntityData) -> String {
    let new_name = unique_entity_name(scene, &entity.name);
    let mut clone = entity.clone();
    clone.name = new_name.clone();
    clone.transform.translation[0] += 16.0;
    clone.transform.translation[1] += 16.0;
    scene.entities.push(clone);
    new_name
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_scene() -> Scene {
        Scene::new("test")
    }

    #[test]
    fn unique_entity_name_base_and_suffixes() {
        let mut scene = empty_scene();
        assert_eq!(unique_entity_name(&scene, "foo"), "foo");
        add_entity(&mut scene, "foo", &MutateOpts::default()).unwrap();
        assert_eq!(unique_entity_name(&scene, "foo"), "foo_1");
        add_entity(&mut scene, "foo_1", &MutateOpts::default()).unwrap();
        assert_eq!(unique_entity_name(&scene, "foo"), "foo_2");
    }

    #[test]
    fn rename_entity_ok_and_errors() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "a", &MutateOpts::default()).unwrap();
        add_entity(&mut scene, "b", &MutateOpts::default()).unwrap();

        rename_entity(&mut scene, "a", "alpha").unwrap();
        assert_eq!(scene.entities[0].name, "alpha");

        assert!(rename_entity(&mut scene, "alpha", "").is_err());
        assert!(rename_entity(&mut scene, "alpha", "b").is_err());
        // same name is a no-op success
        rename_entity(&mut scene, "alpha", "alpha").unwrap();
    }

    #[test]
    fn duplicate_entity_clones_and_offsets() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "orb",
            &MutateOpts {
                x: Some(100.0),
                y: Some(200.0),
                radius: Some(10.0),
                ..Default::default()
            },
        )
        .unwrap();

        let new_name = duplicate_entity(&mut scene, "orb").unwrap();
        assert_eq!(new_name, "orb_1");
        assert_eq!(scene.entities.len(), 2);
        let dup = &scene.entities[1];
        assert_eq!(dup.name, "orb_1");
        assert_eq!(dup.transform.translation[0], 116.0);
        assert_eq!(dup.transform.translation[1], 216.0);
        assert!(dup.components.disc.is_some());

        let new2 = duplicate_entity(&mut scene, "orb").unwrap();
        assert_eq!(new2, "orb_2");
    }

    #[test]
    fn insert_entity_clone_for_paste() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "hero",
            &MutateOpts {
                x: Some(10.0),
                y: Some(20.0),
                ..Default::default()
            },
        )
        .unwrap();
        let clip = scene.entities[0].clone();
        let pasted = insert_entity_clone(&mut scene, &clip);
        assert_eq!(pasted, "hero_1");
        assert_eq!(scene.entities[1].transform.translation[0], 26.0);
        assert_eq!(scene.entities[1].transform.translation[1], 36.0);
    }
}
