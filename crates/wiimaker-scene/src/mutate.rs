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
            enabled: true,
        });
    }
    if let Some(radius) = opts.radius {
        components.disc = Some(SceneDisc {
            radius,
            color: opts.color.unwrap_or([72, 210, 160, 255]),
            z: 0.0,
            enabled: true,
        });
    }
    scene.entities.push(EntityData {
        name: name.to_string(),
        parent: None,
        transform: SceneTransform::from_xy(x, y),
        components,
        tag: 0,
    });
    Ok(())
}

/// Remove an entity and all of its descendants (Unity-style cascade).
pub fn remove_entity(scene: &mut Scene, name: &str) -> Result<()> {
    if !name_exists(scene, name) {
        bail!("entity '{name}' not found");
    }
    let mut kill = vec![name.to_string()];
    let mut i = 0;
    while i < kill.len() {
        let n = kill[i].clone();
        for child in scene.child_names(&n) {
            if !kill.iter().any(|k| k == &child) {
                kill.push(child);
            }
        }
        i += 1;
    }
    scene.entities.retain(|e| !kill.iter().any(|k| k == &e.name));
    Ok(())
}

/// Rename an entity in place. Fails if `new` is empty or already taken.
/// Updates child `parent` refs that pointed at `old`.
pub fn rename_entity(scene: &mut Scene, old: &str, new: &str) -> Result<()> {
    if new.is_empty() {
        bail!("entity name cannot be empty");
    }
    if old != new && name_exists(scene, new) {
        bail!("entity '{new}' already exists");
    }
    let ent = find_mut(scene, old)?;
    ent.name = new.to_string();
    if old != new {
        for e in &mut scene.entities {
            if e.parent.as_deref() == Some(old) {
                e.parent = Some(new.to_string());
            }
        }
    }
    Ok(())
}

/// Reparent `child` under `parent` (or scene root if `None`), preserving world pose.
pub fn set_entity_parent(scene: &mut Scene, child: &str, parent: Option<&str>) -> Result<()> {
    if !name_exists(scene, child) {
        bail!("entity '{child}' not found");
    }
    if let Some(p) = parent {
        if p == child {
            bail!("cannot parent '{child}' to itself");
        }
        if !name_exists(scene, p) {
            bail!("parent entity '{p}' not found");
        }
        if scene.is_descendant_of(p, child) {
            bail!("cannot parent '{child}' under descendant '{p}' (cycle)");
        }
    }
    let world = scene
        .world_transform(child)
        .ok_or_else(|| anyhow::anyhow!("entity '{child}' has a cyclic parent chain"))?;
    let local = match parent {
        Some(p) => {
            let pw = scene
                .world_transform(p)
                .ok_or_else(|| anyhow::anyhow!("parent '{p}' has a cyclic parent chain"))?;
            SceneTransform::to_local(&pw, &world)
        }
        None => world,
    };
    let ent = find_mut(scene, child)?;
    ent.parent = parent.map(|s| s.to_string());
    ent.transform = local;
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

/// Snapshot an entity as a Prefab (root; parent cleared).
pub fn entity_to_prefab(scene: &Scene, name: &str) -> Result<crate::scene::Prefab> {
    let ent = scene
        .find_entity(name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let mut entity = ent;
    entity.parent = None;
    Ok(crate::scene::Prefab { entity })
}

/// Instantiate a prefab into the scene at optional world XY. Returns new entity name.
pub fn instantiate_prefab(
    scene: &mut Scene,
    prefab: &crate::scene::Prefab,
    x: Option<f32>,
    y: Option<f32>,
) -> String {
    let mut entity = prefab.entity.clone();
    entity.parent = None;
    if let Some(x) = x {
        entity.transform.translation[0] = x;
    }
    if let Some(y) = y {
        entity.transform.translation[1] = y;
    }
    // Avoid double +16 when x/y provided: insert without offset path.
    let new_name = unique_entity_name(scene, &entity.name);
    entity.name = new_name.clone();
    scene.entities.push(entity);
    new_name
}

/// Apply prefab components + local transform onto an existing entity (keeps name/parent).
pub fn apply_prefab(scene: &mut Scene, name: &str, prefab: &crate::scene::Prefab) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.transform = prefab.entity.transform.clone();
    ent.components = prefab.entity.components.clone();
    ent.tag = prefab.entity.tag;
    Ok(())
}

/// "Unpack" v0: clear any future prefab link — today just verifies the entity exists.
/// Kept so CLI/editor can grow instance metadata later without renaming the verb.
pub fn unpack_prefab_instance(scene: &mut Scene, name: &str) -> Result<()> {
    let _ = find_mut(scene, name)?;
    Ok(())
}

/// Set **local** translation (relative to parent). Prefer [`set_entity_world_xy`] for viewport drags.
pub fn set_entity_transform(
    scene: &mut Scene,
    name: &str,
    x: Option<f32>,
    y: Option<f32>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if let Some(x) = x {
        ent.transform.translation[0] = x;
    }
    if let Some(y) = y {
        ent.transform.translation[1] = y;
    }
    Ok(())
}

/// Set world-space XY while keeping local scale/rotation; converts through parent if any.
pub fn set_entity_world_xy(scene: &mut Scene, name: &str, x: f32, y: f32) -> Result<()> {
    let parent = scene
        .find_entity(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?
        .parent
        .clone();
    let mut world = scene
        .world_transform(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has a cyclic parent chain"))?;
    world.translation[0] = x;
    world.translation[1] = y;
    let local = match parent.as_deref() {
        Some(p) => {
            let pw = scene
                .world_transform(p)
                .ok_or_else(|| anyhow::anyhow!("parent '{p}' has a cyclic parent chain"))?;
            SceneTransform::to_local(&pw, &world)
        }
        None => world,
    };
    let ent = find_mut(scene, name)?;
    ent.transform.translation = local.translation;
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
        enabled: true,
    });
    Ok(())
}

pub fn add_component_disc(scene: &mut Scene, name: &str, radius: f32, color: [u8; 4]) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.disc = Some(SceneDisc {
        radius,
        color,
        z: 0.0,
        enabled: true,
    });
    Ok(())
}

pub fn remove_component_sprite(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.sprite.is_none() {
        bail!("entity '{name}' has no Sprite");
    }
    ent.components.sprite = None;
    Ok(())
}

pub fn remove_component_disc(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.disc.is_none() {
        bail!("entity '{name}' has no Disc");
    }
    ent.components.disc = None;
    Ok(())
}

pub fn set_component_enabled(
    scene: &mut Scene,
    name: &str,
    kind: &str,
    enabled: bool,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    match kind.to_ascii_lowercase().as_str() {
        "sprite" => {
            let sp = ent
                .components
                .sprite
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Sprite"))?;
            sp.enabled = enabled;
        }
        "disc" => {
            let d = ent
                .components
                .disc
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Disc"))?;
            d.enabled = enabled;
        }
        other => bail!("unknown component kind '{other}' (Sprite|Disc)"),
    }
    Ok(())
}

/// Set local XY scale (preserves Z scale).
pub fn set_entity_scale(scene: &mut Scene, name: &str, sx: f32, sy: f32) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.transform.scale[0] = sx;
    ent.transform.scale[1] = sy;
    Ok(())
}

/// Set 2D Z rotation (radians) as a local quaternion. X/Y rotation cleared.
pub fn set_entity_rotation_z(scene: &mut Scene, name: &str, radians: f32) -> Result<()> {
    let half = radians * 0.5;
    let ent = find_mut(scene, name)?;
    ent.transform.rotation = [0.0, 0.0, half.sin(), half.cos()];
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

    #[test]
    fn set_parent_preserves_world_and_rejects_cycles() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "parent",
            &MutateOpts {
                x: Some(100.0),
                y: Some(50.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "child",
            &MutateOpts {
                x: Some(130.0),
                y: Some(70.0),
                ..Default::default()
            },
        )
        .unwrap();

        set_entity_parent(&mut scene, "child", Some("parent")).unwrap();
        let child = scene.find_entity("child").unwrap();
        assert_eq!(child.parent.as_deref(), Some("parent"));
        assert_eq!(child.transform.translation[0], 30.0);
        assert_eq!(child.transform.translation[1], 20.0);
        let world = scene.world_transform("child").unwrap();
        assert_eq!(world.translation[0], 130.0);
        assert_eq!(world.translation[1], 70.0);

        assert!(set_entity_parent(&mut scene, "parent", Some("child")).is_err());
        assert!(set_entity_parent(&mut scene, "child", Some("child")).is_err());

        set_entity_parent(&mut scene, "child", None).unwrap();
        assert!(scene.find_entity("child").unwrap().parent.is_none());
        assert_eq!(
            scene.find_entity("child").unwrap().transform.translation[0],
            130.0
        );
    }

    #[test]
    fn remove_cascades_to_children() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "a", &MutateOpts::default()).unwrap();
        add_entity(&mut scene, "b", &MutateOpts::default()).unwrap();
        add_entity(&mut scene, "c", &MutateOpts::default()).unwrap();
        set_entity_parent(&mut scene, "b", Some("a")).unwrap();
        set_entity_parent(&mut scene, "c", Some("b")).unwrap();
        remove_entity(&mut scene, "a").unwrap();
        assert!(scene.entities.is_empty());
    }

    #[test]
    fn rename_updates_child_parent_refs() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "a", &MutateOpts::default()).unwrap();
        add_entity(&mut scene, "b", &MutateOpts::default()).unwrap();
        set_entity_parent(&mut scene, "b", Some("a")).unwrap();
        rename_entity(&mut scene, "a", "root").unwrap();
        assert_eq!(scene.find_entity("b").unwrap().parent.as_deref(), Some("root"));
    }

    #[test]
    fn set_entity_world_xy_under_parent() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "p",
            &MutateOpts {
                x: Some(100.0),
                y: Some(100.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "c",
            &MutateOpts {
                x: Some(100.0),
                y: Some(100.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_parent(&mut scene, "c", Some("p")).unwrap();
        set_entity_world_xy(&mut scene, "c", 150.0, 120.0).unwrap();
        assert_eq!(scene.find_entity("c").unwrap().transform.translation[0], 50.0);
        assert_eq!(scene.find_entity("c").unwrap().transform.translation[1], 20.0);
        let w = scene.world_transform("c").unwrap();
        assert_eq!(w.translation[0], 150.0);
        assert_eq!(w.translation[1], 120.0);
    }
}
