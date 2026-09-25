//! Shared scene mutations used by CLI and editor.

use anyhow::{bail, Result};

use crate::scene::{EntityData, Scene, SceneComponents, SceneDisc, SceneSprite, SceneTransform};

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
            size: [opts.sprite_w.unwrap_or(32.0), opts.sprite_h.unwrap_or(32.0)],
            color: opts.color.unwrap_or([255, 255, 255, 255]),
            z: 0.0,
            sorting_layer: String::new(),
            enabled: true,
            pivot: None,
        });
    }
    if let Some(radius) = opts.radius {
        components.disc = Some(SceneDisc {
            radius,
            color: opts.color.unwrap_or([72, 210, 160, 255]),
            z: 0.0,
            sorting_layer: String::new(),
            enabled: true,
        });
    }
    scene.entities.push(EntityData {
        name: name.to_string(),
        parent: None,
        transform: SceneTransform::from_xy(x, y),
        components,
        tag: 0,
        prefab: None,
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
    scene
        .entities
        .retain(|e| !kill.iter().any(|k| k == &e.name));
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
            if let Some(cam) = e.components.camera.as_mut() {
                if cam.follow.as_deref() == Some(old) {
                    cam.follow = Some(new.to_string());
                }
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

/// Snapshot an entity as a Prefab (root; parent + instance link cleared).
pub fn entity_to_prefab(scene: &Scene, name: &str) -> Result<crate::scene::Prefab> {
    let ent = scene
        .find_entity(name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let mut entity = ent;
    entity.parent = None;
    entity.prefab = None;
    Ok(crate::scene::Prefab { entity })
}

/// Record `source` (stem or relative `*.prefab.json`) on an existing entity.
pub fn attach_prefab_instance(scene: &mut Scene, name: &str, source: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let link = crate::prefab::normalize_prefab_source(source);
    if link.is_empty() {
        bail!("prefab source cannot be empty");
    }
    ent.prefab = Some(link);
    Ok(())
}

/// Instantiate a prefab into the scene at optional world XY. Returns new entity name.
/// `source` is stored on the instance (stem or game-relative `*.prefab.json`).
pub fn instantiate_prefab(
    scene: &mut Scene,
    prefab: &crate::scene::Prefab,
    source: &str,
    x: Option<f32>,
    y: Option<f32>,
) -> String {
    let mut entity = prefab.entity.clone();
    entity.parent = None;
    entity.prefab = Some(crate::prefab::normalize_prefab_source(source));
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

/// Unity Apply: push instance transform / components / tag onto the prefab asset blob.
/// Keeps the prefab's own name; clears parent/link on the asset. Links the instance to `source`.
pub fn apply_prefab(
    scene: &mut Scene,
    name: &str,
    prefab: &mut crate::scene::Prefab,
    source: &str,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    prefab.entity.transform = ent.transform.clone();
    prefab.entity.components = ent.components.clone();
    prefab.entity.tag = ent.tag;
    prefab.entity.parent = None;
    prefab.entity.prefab = None;
    let link = crate::prefab::normalize_prefab_source(source);
    if link.is_empty() {
        bail!("prefab source cannot be empty");
    }
    ent.prefab = Some(link);
    Ok(())
}

/// Unity Revert: reset instance transform / components / tag from the prefab asset.
/// Keeps name, parent, and prefab link.
pub fn revert_prefab_instance(
    scene: &mut Scene,
    name: &str,
    prefab: &crate::scene::Prefab,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.transform = prefab.entity.transform.clone();
    ent.components = prefab.entity.components.clone();
    ent.tag = prefab.entity.tag;
    Ok(())
}

/// Clear the prefab instance link; current values stay as a plain entity.
pub fn unpack_prefab_instance(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.prefab = None;
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

/// Set world-space XY while keeping local scale/rotation; converts through parent if any
/// (inverse-rotates the delta when the parent’s world rotation is non-zero).
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
        sorting_layer: String::new(),
        enabled: true,
        pivot: None,
    });
    Ok(())
}

/// Set or clear the Sprite component pivot override (catalog fallback when cleared).
///
/// Pass `clear` to drop the override. Otherwise `x` / `y` write the override;
/// a missing axis keeps the current override or `0.5`.
pub fn set_entity_sprite_pivot(
    scene: &mut Scene,
    name: &str,
    x: Option<f32>,
    y: Option<f32>,
    clear: bool,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let sp = ent
        .components
        .sprite
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Sprite"))?;
    if clear {
        sp.pivot = None;
        return Ok(());
    }
    if x.is_none() && y.is_none() {
        bail!("set pivot: pass --pivot-x and/or --pivot-y, or --clear-pivot");
    }
    let mut p = sp.pivot.unwrap_or([0.5, 0.5]);
    if let Some(vx) = x {
        p[0] = vx;
    }
    if let Some(vy) = y {
        p[1] = vy;
    }
    sp.pivot = Some(p);
    Ok(())
}

pub fn add_component_disc(
    scene: &mut Scene,
    name: &str,
    radius: f32,
    color: [u8; 4],
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.disc = Some(SceneDisc {
        radius,
        color,
        z: 0.0,
        sorting_layer: String::new(),
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
        "tilemap" => {
            let tm = ent
                .components
                .tilemap
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Tilemap"))?;
            tm.enabled = enabled;
        }
        "collider" => {
            let c = ent
                .components
                .collider
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Collider"))?;
            c.enabled = enabled;
        }
        "animation" => {
            let a = ent
                .components
                .animation
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Animation"))?;
            a.enabled = enabled;
        }
        "camera" => {
            let c = ent
                .components
                .camera
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Camera"))?;
            c.active = enabled;
        }
        "gridmover" | "grid_mover" | "grid-mover" => {
            let g = ent
                .components
                .grid_mover
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no GridMover"))?;
            g.enabled = enabled;
        }
        "audiosource" | "audio_source" | "audio-source" | "audio" => {
            let a = ent
                .components
                .audio_source
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no AudioSource"))?;
            a.enabled = enabled;
        }
        "text" | "label" | "hud" => {
            let t = ent
                .components
                .text
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Text"))?;
            t.enabled = enabled;
        }
        other => {
            bail!(
                "unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider|Animation|Camera|GridMover|AudioSource|Text)"
            )
        }
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

pub fn add_component_animation(
    scene: &mut Scene,
    name: &str,
    clip: &str,
    fps: Option<f32>,
    loop_: bool,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.animation = Some(crate::scene::SceneAnimation {
        clip: clip.to_string(),
        fps,
        loop_,
        enabled: true,
    });
    Ok(())
}

pub fn remove_component_animation(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.animation.is_none() {
        bail!("entity '{name}' has no Animation");
    }
    ent.components.animation = None;
    Ok(())
}

pub fn set_entity_anim(
    scene: &mut Scene,
    name: &str,
    clip: &str,
    fps: Option<f32>,
    loop_: Option<bool>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    match ent.components.animation.as_mut() {
        Some(a) => {
            a.clip = clip.to_string();
            if fps.is_some() {
                a.fps = fps;
            }
            if let Some(l) = loop_ {
                a.loop_ = l;
            }
            a.enabled = true;
        }
        None => {
            ent.components.animation = Some(crate::scene::SceneAnimation {
                clip: clip.to_string(),
                fps,
                loop_: loop_.unwrap_or(true),
                enabled: true,
            });
        }
    }
    Ok(())
}

pub fn add_component_camera(scene: &mut Scene, name: &str, active: bool) -> Result<()> {
    let ent = find_mut(scene, name)?;
    match ent.components.camera.as_mut() {
        Some(c) => c.active = active,
        None => {
            ent.components.camera = Some(crate::scene::SceneCamera {
                active,
                follow: None,
                lerp: 0.15,
            });
        }
    }
    Ok(())
}

pub fn remove_component_camera(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.camera.is_none() {
        bail!("entity '{name}' has no Camera");
    }
    ent.components.camera = None;
    Ok(())
}
/// Clear Camera follow state without removing the Camera component.
/// Follow is stored on SceneCamera rather than as a separate component.
pub fn remove_component_follow(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let cam = ent
        .components
        .camera
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Camera"))?;
    cam.follow = None;
    cam.lerp = 0.15;
    Ok(())
}

/// Ensure Camera exists and set follow target + optional lerp.
/// Empty `target` clears follow.
pub fn add_component_follow(
    scene: &mut Scene,
    name: &str,
    target: &str,
    lerp: Option<f32>,
) -> Result<()> {
    set_entity_follow(scene, name, Some(target), lerp)
}

/// Set Camera follow. `target` `None` leaves the name unchanged; `Some("")` clears it.
/// Creates an active Camera if missing.
pub fn set_entity_follow(
    scene: &mut Scene,
    name: &str,
    target: Option<&str>,
    lerp: Option<f32>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let cam = ent
        .components
        .camera
        .get_or_insert_with(|| crate::scene::SceneCamera {
            active: true,
            follow: None,
            lerp: 0.15,
        });
    if let Some(t) = target {
        cam.follow = if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        };
    }
    if let Some(l) = lerp {
        cam.lerp = l.clamp(0.0, 1.0);
    }
    Ok(())
}

pub fn add_component_grid_mover(
    scene: &mut Scene,
    name: &str,
    cell: f32,
    speed: f32,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let mut gm = crate::scene::SceneGridMover::new(cell, speed);
    if let Some(prev) = ent.components.grid_mover.as_ref() {
        gm.queued_dir = prev.queued_dir;
        gm.enabled = prev.enabled;
    }
    ent.components.grid_mover = Some(gm);
    Ok(())
}

pub fn remove_component_grid_mover(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.grid_mover.is_none() {
        bail!("entity '{name}' has no GridMover");
    }
    ent.components.grid_mover = None;
    Ok(())
}

/// Create or update GridMover. `None` fields leave the existing value (or defaults).
pub fn set_entity_grid_mover(
    scene: &mut Scene,
    name: &str,
    cell: Option<f32>,
    speed: Option<f32>,
    queued_dir: Option<Option<crate::scene::SceneDir>>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let gm = ent
        .components
        .grid_mover
        .get_or_insert_with(crate::scene::SceneGridMover::default);
    if let Some(c) = cell {
        gm.cell = if c <= 0.0 { 16.0 } else { c };
    }
    if let Some(s) = speed {
        gm.speed = s.max(0.0);
    }
    if let Some(q) = queued_dir {
        gm.queued_dir = q;
    }
    gm.enabled = true;
    Ok(())
}

pub fn add_component_audio_source(
    scene: &mut Scene,
    name: &str,
    clip: &str,
    volume: f32,
    play_on_awake: bool,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let mut src = crate::scene::SceneAudioSource::new(clip, volume, play_on_awake);
    if let Some(prev) = ent.components.audio_source.as_ref() {
        src.enabled = prev.enabled;
    }
    ent.components.audio_source = Some(src);
    Ok(())
}

pub fn remove_component_audio_source(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.audio_source.is_none() {
        bail!("entity '{name}' has no AudioSource");
    }
    ent.components.audio_source = None;
    Ok(())
}

/// Create or update AudioSource. `None` fields leave the existing value (or defaults).
pub fn set_entity_audio_source(
    scene: &mut Scene,
    name: &str,
    clip: Option<&str>,
    volume: Option<f32>,
    play_on_awake: Option<bool>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let a = ent
        .components
        .audio_source
        .get_or_insert_with(crate::scene::SceneAudioSource::default);
    if let Some(c) = clip {
        a.clip = c.to_string();
    }
    if let Some(v) = volume {
        a.volume = v.clamp(0.0, 1.0);
    }
    if let Some(p) = play_on_awake {
        a.play_on_awake = p;
    }
    a.enabled = true;
    Ok(())
}

pub fn add_component_text(
    scene: &mut Scene,
    name: &str,
    text: &str,
    size: f32,
    color: [u8; 4],
    align: crate::scene::SceneTextAlign,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let mut t = crate::scene::SceneText::new(text, size, color);
    t.align = align;
    if let Some(prev) = ent.components.text.as_ref() {
        t.enabled = prev.enabled;
        t.z = prev.z;
        t.sorting_layer = prev.sorting_layer.clone();
    }
    ent.components.text = Some(t);
    Ok(())
}

pub fn remove_component_text(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.text.is_none() {
        bail!("entity '{name}' has no Text");
    }
    ent.components.text = None;
    Ok(())
}

/// Create or update Text. `None` fields leave the existing value (or defaults).
pub fn set_entity_text(
    scene: &mut Scene,
    name: &str,
    text: Option<&str>,
    size: Option<f32>,
    color: Option<[u8; 4]>,
    align: Option<crate::scene::SceneTextAlign>,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let t = ent
        .components
        .text
        .get_or_insert_with(crate::scene::SceneText::default);
    if let Some(s) = text {
        t.text = s.to_string();
    }
    if let Some(sz) = size {
        t.size = if sz <= 0.0 { 16.0 } else { sz };
    }
    if let Some(c) = color {
        t.color = c;
    }
    if let Some(a) = align {
        t.align = a;
    }
    t.enabled = true;
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
        assert_eq!(
            scene.find_entity("b").unwrap().parent.as_deref(),
            Some("root")
        );
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
        assert_eq!(
            scene.find_entity("c").unwrap().transform.translation[0],
            50.0
        );
        assert_eq!(
            scene.find_entity("c").unwrap().transform.translation[1],
            20.0
        );
        let w = scene.world_transform("c").unwrap();
        assert_eq!(w.translation[0], 150.0);
        assert_eq!(w.translation[1], 120.0);
    }

    fn near(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn world_transform_rotates_child_offset_then_translates() {
        // Parent (100, 200) +90° Z; child local (10, 0) → world (100, 210).
        // World rotation = parent 90° + local 15° = 105°.
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "p",
            &MutateOpts {
                x: Some(100.0),
                y: Some(200.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "c",
            &MutateOpts {
                x: Some(10.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "p", 90.0_f32.to_radians()).unwrap();
        set_entity_rotation_z(&mut scene, "c", 15.0_f32.to_radians()).unwrap();
        scene
            .entities
            .iter_mut()
            .find(|e| e.name == "c")
            .unwrap()
            .parent = Some("p".into());

        let world = scene.world_transform("c").unwrap();
        near(world.translation[0], 100.0);
        near(world.translation[1], 210.0);
        near(world.rotation_z(), 105.0_f32.to_radians());
    }

    #[test]
    fn set_parent_under_rotated_parent_preserves_world_pose() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "p",
            &MutateOpts {
                x: Some(100.0),
                y: Some(50.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "c",
            &MutateOpts {
                x: Some(130.0),
                y: Some(70.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "p", 90.0_f32.to_radians()).unwrap();
        set_entity_rotation_z(&mut scene, "c", 20.0_f32.to_radians()).unwrap();

        set_entity_parent(&mut scene, "c", Some("p")).unwrap();
        let world = scene.world_transform("c").unwrap();
        near(world.translation[0], 130.0);
        near(world.translation[1], 70.0);
        near(world.rotation_z(), 20.0_f32.to_radians());

        // Inverse-rotate (30, 20) by −90°: (x,y) → (y, −x) → (20, −30).
        let local = &scene.find_entity("c").unwrap().transform;
        near(local.translation[0], 20.0);
        near(local.translation[1], -30.0);
        near(local.rotation_z(), (20.0 - 90.0_f32).to_radians());

        set_entity_parent(&mut scene, "c", None).unwrap();
        let root = scene.find_entity("c").unwrap();
        assert!(root.parent.is_none());
        near(root.transform.translation[0], 130.0);
        near(root.transform.translation[1], 70.0);
        near(root.transform.rotation_z(), 20.0_f32.to_radians());
    }

    #[test]
    fn set_entity_world_xy_under_rotated_parent_keeps_local_rotation() {
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
                x: Some(140.0),
                y: Some(100.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "p", 90.0_f32.to_radians()).unwrap();
        set_entity_rotation_z(&mut scene, "c", -10.0_f32.to_radians()).unwrap();
        set_entity_parent(&mut scene, "c", Some("p")).unwrap();

        // Drag child to (100, 150): +90° of local +X 50. Local rotation stays.
        let local_rot_before = scene.find_entity("c").unwrap().transform.rotation_z();
        set_entity_world_xy(&mut scene, "c", 100.0, 150.0).unwrap();
        let local = &scene.find_entity("c").unwrap().transform;
        near(local.translation[0], 50.0);
        near(local.translation[1], 0.0);
        near(local.rotation_z(), local_rot_before);
        let w = scene.world_transform("c").unwrap();
        near(w.translation[0], 100.0);
        near(w.translation[1], 150.0);
        near(w.rotation_z(), -10.0_f32.to_radians());
    }

    #[test]
    fn hydrate_writes_composed_world_rotation() {
        use crate::hydrate::{hydrate_lenient, TextureMap};

        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "p",
            &MutateOpts {
                x: Some(80.0),
                y: Some(40.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "c",
            &MutateOpts {
                x: Some(10.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "p", 90.0_f32.to_radians()).unwrap();
        set_entity_rotation_z(&mut scene, "c", 15.0_f32.to_radians()).unwrap();
        set_entity_parent(&mut scene, "c", Some("p")).unwrap();

        let world = hydrate_lenient(&scene, &TextureMap::new());
        let id = world.find_by_name("c").unwrap();
        let xf = world.transform(id).unwrap();
        near(xf.translation.x, 80.0);
        near(xf.translation.y, 50.0);
        near(xf.rotation_z(), 105.0_f32.to_radians());
    }

    #[test]
    fn camera_follow_mutate_and_rename_target() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
        add_entity(&mut scene, "Main Camera", &MutateOpts::default()).unwrap();
        add_component_camera(&mut scene, "Main Camera", true).unwrap();
        assert!(scene
            .find_entity("Main Camera")
            .unwrap()
            .components
            .camera
            .as_ref()
            .unwrap()
            .follow
            .is_none());
        set_entity_follow(&mut scene, "Main Camera", Some("Player"), Some(0.2)).unwrap();
        let cam = scene
            .find_entity("Main Camera")
            .unwrap()
            .components
            .camera
            .as_ref()
            .unwrap();
        assert!(cam.active);
        assert_eq!(cam.follow.as_deref(), Some("Player"));
        assert!((cam.lerp - 0.2).abs() < 1e-6);
        rename_entity(&mut scene, "Player", "Hero").unwrap();
        assert_eq!(
            scene
                .find_entity("Main Camera")
                .unwrap()
                .components
                .camera
                .as_ref()
                .unwrap()
                .follow
                .as_deref(),
            Some("Hero")
        );
        remove_component_follow(&mut scene, "Main Camera").unwrap();
        let cam = scene
            .find_entity("Main Camera")
            .unwrap()
            .components
            .camera
            .as_ref()
            .unwrap();
        assert!(cam.follow.is_none());
        assert!((cam.lerp - 0.15).abs() < 1e-6);
    }

    #[test]
    fn grid_mover_add_set_remove() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
        add_component_grid_mover(&mut scene, "Player", 20.0, 6.0).unwrap();
        let g = scene
            .find_entity("Player")
            .unwrap()
            .components
            .grid_mover
            .as_ref()
            .unwrap();
        assert!((g.cell - 20.0).abs() < 1e-6);
        assert!((g.speed - 6.0).abs() < 1e-6);
        set_entity_grid_mover(&mut scene, "Player", Some(16.0), Some(120.0), None).unwrap();
        let g = scene
            .find_entity("Player")
            .unwrap()
            .components
            .grid_mover
            .as_ref()
            .unwrap();
        assert!((g.cell - 16.0).abs() < 1e-6);
        assert!((g.speed - 120.0).abs() < 1e-6);
        remove_component_grid_mover(&mut scene, "Player").unwrap();
        assert!(scene
            .find_entity("Player")
            .unwrap()
            .components
            .grid_mover
            .is_none());
    }

    #[test]
    fn audio_source_add_set_remove() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
        add_component_audio_source(&mut scene, "Player", "beep", 0.5, true).unwrap();
        let a = scene
            .find_entity("Player")
            .unwrap()
            .components
            .audio_source
            .as_ref()
            .unwrap();
        assert_eq!(a.clip, "beep");
        assert!((a.volume - 0.5).abs() < 1e-6);
        assert!(a.play_on_awake);
        set_entity_audio_source(&mut scene, "Player", Some("hit"), Some(1.0), Some(false)).unwrap();
        let a = scene
            .find_entity("Player")
            .unwrap()
            .components
            .audio_source
            .as_ref()
            .unwrap();
        assert_eq!(a.clip, "hit");
        assert!((a.volume - 1.0).abs() < 1e-6);
        assert!(!a.play_on_awake);
        remove_component_audio_source(&mut scene, "Player").unwrap();
        assert!(scene
            .find_entity("Player")
            .unwrap()
            .components
            .audio_source
            .is_none());
    }

    #[test]
    fn text_add_set_remove() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "Hud", &MutateOpts::default()).unwrap();
        add_component_text(
            &mut scene,
            "Hud",
            "Score: 0",
            16.0,
            [255, 255, 0, 255],
            crate::scene::SceneTextAlign::Left,
        )
        .unwrap();
        let t = scene
            .find_entity("Hud")
            .unwrap()
            .components
            .text
            .as_ref()
            .unwrap();
        assert_eq!(t.text, "Score: 0");
        assert!((t.size - 16.0).abs() < 1e-6);
        set_entity_text(
            &mut scene,
            "Hud",
            Some("Score: 1"),
            Some(24.0),
            Some([255, 255, 255, 255]),
            Some(crate::scene::SceneTextAlign::Center),
        )
        .unwrap();
        let t = scene
            .find_entity("Hud")
            .unwrap()
            .components
            .text
            .as_ref()
            .unwrap();
        assert_eq!(t.text, "Score: 1");
        assert!((t.size - 24.0).abs() < 1e-6);
        assert_eq!(t.align, crate::scene::SceneTextAlign::Center);
        remove_component_text(&mut scene, "Hud").unwrap();
        assert!(scene.find_entity("Hud").unwrap().components.text.is_none());
    }

    #[test]
    fn instantiate_records_prefab_link_and_unpack_clears() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "Ghost",
            &MutateOpts {
                x: Some(80.0),
                y: Some(90.0),
                radius: Some(10.0),
                ..Default::default()
            },
        )
        .unwrap();
        let prefab = entity_to_prefab(&scene, "Ghost").unwrap();
        assert!(prefab.entity.prefab.is_none());

        let name = instantiate_prefab(&mut scene, &prefab, "ghost", Some(200.0), Some(210.0));
        assert_eq!(name, "Ghost_1");
        let inst = scene.find_entity(&name).unwrap();
        assert_eq!(
            inst.prefab.as_deref(),
            Some("assets/prefabs/ghost.prefab.json")
        );
        assert_eq!(inst.transform.translation[0], 200.0);
        assert!(inst.components.disc.is_some());

        unpack_prefab_instance(&mut scene, &name).unwrap();
        let plain = scene.find_entity(&name).unwrap();
        assert!(plain.prefab.is_none());
        assert_eq!(plain.transform.translation[0], 200.0);
        assert!(plain.components.disc.is_some());
    }

    #[test]
    fn apply_writes_prefab_blob_and_revert_restores() {
        let mut scene = empty_scene();
        add_entity(
            &mut scene,
            "Dot",
            &MutateOpts {
                x: Some(16.0),
                y: Some(32.0),
                radius: Some(4.0),
                ..Default::default()
            },
        )
        .unwrap();
        let mut prefab = entity_to_prefab(&scene, "Dot").unwrap();
        let name = instantiate_prefab(&mut scene, &prefab, "dot", None, None);
        set_entity_transform(&mut scene, &name, Some(99.0), Some(88.0)).unwrap();
        scene
            .entities
            .iter_mut()
            .find(|e| e.name == name)
            .unwrap()
            .tag = 7;

        apply_prefab(&mut scene, &name, &mut prefab, "dot").unwrap();
        assert_eq!(prefab.entity.transform.translation[0], 99.0);
        assert_eq!(prefab.entity.tag, 7);
        assert!(prefab.entity.prefab.is_none());
        assert_eq!(
            scene.find_entity(&name).unwrap().prefab.as_deref(),
            Some("assets/prefabs/dot.prefab.json")
        );

        set_entity_transform(&mut scene, &name, Some(1.0), Some(2.0)).unwrap();
        revert_prefab_instance(&mut scene, &name, &prefab).unwrap();
        let inst = scene.find_entity(&name).unwrap();
        assert_eq!(inst.transform.translation[0], 99.0);
        assert_eq!(inst.transform.translation[1], 88.0);
        assert_eq!(inst.tag, 7);
        assert_eq!(
            inst.prefab.as_deref(),
            Some("assets/prefabs/dot.prefab.json")
        );
    }

    #[test]
    fn set_entity_sprite_pivot_sets_and_clears() {
        let mut scene = empty_scene();
        add_entity(&mut scene, "orb", &MutateOpts::default()).unwrap();
        assert!(set_entity_sprite_pivot(&mut scene, "orb", Some(0.0), Some(1.0), false).is_err());
        add_component_sprite(&mut scene, "orb", "tex", [16.0, 16.0]).unwrap();
        set_entity_sprite_pivot(&mut scene, "orb", Some(0.0), Some(1.0), false).unwrap();
        let sp = scene
            .find_entity("orb")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap();
        assert_eq!(sp.pivot, Some([0.0, 1.0]));
        let json = serde_json::to_string(sp).unwrap();
        assert!(json.contains("\"pivot\""), "{json}");
        set_entity_sprite_pivot(&mut scene, "orb", None, None, true).unwrap();
        let sp = scene
            .find_entity("orb")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap();
        assert!(sp.pivot.is_none());
        let json = serde_json::to_string(sp).unwrap();
        assert!(!json.contains("pivot"), "{json}");
        set_entity_sprite_pivot(&mut scene, "orb", Some(0.25), None, false).unwrap();
        let sp = scene
            .find_entity("orb")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap();
        assert_eq!(sp.pivot, Some([0.25, 0.5]));
    }

    #[test]
    fn set_scene_clear_sets_rgb_and_opaque_alpha() {
        let mut scene = empty_scene();
        assert_eq!(scene.clear_color, crate::scene::DEFAULT_CLEAR_COLOR);
        scene.clear_color = [1, 2, 3, 4];
        set_scene_clear(&mut scene, [200, 10, 30]);
        assert_eq!(scene.clear_color, [200, 10, 30, 255]);
        let json = serde_json::to_string(&scene).unwrap();
        let loaded: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.clear_color, [200, 10, 30, 255]);
        let rgba = loaded.clear_rgba();
        assert_eq!([rgba.r, rgba.g, rgba.b, rgba.a], [200, 10, 30, 255]);
    }
}
