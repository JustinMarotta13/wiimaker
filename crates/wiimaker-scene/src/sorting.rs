//! Project-level sorting layers (Unity Tags & Layers analogue) + entity assignment.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use wiimaker_core::{default_sorting_layers, is_default_sorting_layer_name, DEFAULT_SORTING_LAYER};

use crate::project::{list_scenes, load_project, save_project, GameProject};
use crate::scene::{
    display_sorting_layer, load_prefab, load_scene, save_prefab, save_scene, EntityData, Scene,
};

/// Validate a new layer name (non-empty, no path separators).
pub fn validate_sorting_layer_name(name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("sorting layer name required");
    }
    if name.contains('/') || name.contains('\\') || name.contains('.') {
        bail!("sorting layer name must be a simple identifier (no path or extension)");
    }
    Ok(())
}

fn ensure_layers(project: &mut GameProject) {
    if project.sorting_layers.is_empty() {
        project.sorting_layers = default_sorting_layers();
    }
}

fn find_layer_index(layers: &[String], name: &str) -> Option<usize> {
    let key = display_sorting_layer(name);
    layers.iter().position(|n| n == key || n == name.trim())
}

/// List effective sorting layers (defaults if unset).
pub fn list_sorting_layers(game_dir: &Path) -> Result<Vec<String>> {
    let project = load_project(game_dir)?;
    Ok(project.effective_sorting_layers())
}

/// Append or insert a sorting layer and save `game.toml`.
pub fn add_sorting_layer(game_dir: &Path, name: &str, index: Option<usize>) -> Result<Vec<String>> {
    validate_sorting_layer_name(name)?;
    let name = name.trim().to_string();
    let mut project = load_project(game_dir)?;
    ensure_layers(&mut project);
    if find_layer_index(&project.sorting_layers, &name).is_some() {
        bail!("sorting layer '{name}' already exists");
    }
    match index {
        Some(i) => {
            let i = i.min(project.sorting_layers.len());
            project.sorting_layers.insert(i, name);
        }
        None => project.sorting_layers.push(name),
    }
    save_project(game_dir, &project)?;
    Ok(project.sorting_layers)
}

/// Rename a layer in `game.toml` and remap scene/prefab component names.
///
/// `skip_scene` (absolute path) is left untouched so a dirty editor can remap in memory.
pub fn rename_sorting_layer(
    game_dir: &Path,
    from: &str,
    to: &str,
    skip_scene: Option<&Path>,
) -> Result<Vec<String>> {
    validate_sorting_layer_name(to)?;
    let from = display_sorting_layer(from).to_string();
    let to = to.trim().to_string();
    if from == to {
        return list_sorting_layers(game_dir);
    }
    let mut project = load_project(game_dir)?;
    ensure_layers(&mut project);
    let Some(idx) = find_layer_index(&project.sorting_layers, &from) else {
        bail!("sorting layer '{from}' not found");
    };
    if find_layer_index(&project.sorting_layers, &to).is_some() {
        bail!("sorting layer '{to}' already exists");
    }
    project.sorting_layers[idx] = to.clone();
    save_project(game_dir, &project)?;
    remap_authoring_files(game_dir, skip_scene, |scene| {
        remap_scene_sorting_layer(scene, &from, &to)
    })?;
    Ok(project.sorting_layers)
}

/// Move a layer to `index` (0 = back / drawn first).
pub fn move_sorting_layer(game_dir: &Path, name: &str, index: usize) -> Result<Vec<String>> {
    let key = display_sorting_layer(name).to_string();
    let mut project = load_project(game_dir)?;
    ensure_layers(&mut project);
    let Some(from) = find_layer_index(&project.sorting_layers, &key) else {
        bail!("sorting layer '{key}' not found");
    };
    let layer = project.sorting_layers.remove(from);
    let to = index.min(project.sorting_layers.len());
    project.sorting_layers.insert(to, layer);
    save_project(game_dir, &project)?;
    Ok(project.sorting_layers)
}

/// Remove a layer (not Default). Components using it are remapped to Default.
pub fn remove_sorting_layer(
    game_dir: &Path,
    name: &str,
    skip_scene: Option<&Path>,
) -> Result<Vec<String>> {
    let key = display_sorting_layer(name).to_string();
    if key.eq_ignore_ascii_case(DEFAULT_SORTING_LAYER) {
        bail!("cannot remove the Default sorting layer");
    }
    let mut project = load_project(game_dir)?;
    ensure_layers(&mut project);
    let Some(idx) = find_layer_index(&project.sorting_layers, &key) else {
        bail!("sorting layer '{name}' not found");
    };
    if project.sorting_layers.len() <= 1 {
        bail!("cannot remove the last sorting layer");
    }
    project.sorting_layers.remove(idx);
    save_project(game_dir, &project)?;
    remap_authoring_files(game_dir, skip_scene, |scene| {
        remap_scene_sorting_layer(scene, &key, DEFAULT_SORTING_LAYER)
    })?;
    Ok(project.sorting_layers)
}

/// Remap `from` → `to` on every drawable component. Returns how many fields changed.
pub fn remap_scene_sorting_layer(scene: &mut Scene, from: &str, to: &str) -> usize {
    let from = display_sorting_layer(from);
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remap_entity_sorting_layer(ent, from, to);
    }
    n
}

fn remap_entity_sorting_layer(ent: &mut EntityData, from: &str, to: &str) -> usize {
    let mut n = 0;
    if let Some(sp) = ent.components.sprite.as_mut() {
        if display_sorting_layer(&sp.sorting_layer) == from {
            sp.sorting_layer = normalize_stored_layer(to);
            n += 1;
        }
    }
    if let Some(d) = ent.components.disc.as_mut() {
        if display_sorting_layer(&d.sorting_layer) == from {
            d.sorting_layer = normalize_stored_layer(to);
            n += 1;
        }
    }
    if let Some(tm) = ent.components.tilemap.as_mut() {
        if display_sorting_layer(&tm.sorting_layer) == from {
            tm.sorting_layer = normalize_stored_layer(to);
            n += 1;
        }
    }
    if let Some(t) = ent.components.text.as_mut() {
        if display_sorting_layer(&t.sorting_layer) == from {
            t.sorting_layer = normalize_stored_layer(to);
            n += 1;
        }
    }
    n
}

fn normalize_stored_layer(name: &str) -> String {
    if is_default_sorting_layer_name(name) {
        String::new()
    } else {
        name.trim().to_string()
    }
}

fn remap_authoring_files(
    game_dir: &Path,
    skip_scene: Option<&Path>,
    mut f: impl FnMut(&mut Scene) -> usize,
) -> Result<()> {
    let skip = skip_scene.map(|p| dunce_norm(p));
    for rel in list_scenes(game_dir)? {
        let abs = game_dir.join(&rel);
        if skip.as_ref().is_some_and(|s| dunce_norm(&abs) == *s) {
            continue;
        }
        if !abs.is_file() {
            continue;
        }
        let mut scene = load_scene(&abs)?;
        if f(&mut scene) > 0 {
            save_scene(&abs, &scene)?;
        }
    }
    let prefab_dir = game_dir.join("assets").join("prefabs");
    if prefab_dir.is_dir() {
        for entry in fs::read_dir(&prefab_dir)? {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".prefab.json"))
            {
                let mut prefab = load_prefab(&path)?;
                let mut dummy = Scene::new("prefab");
                dummy.entities.push(prefab.entity.clone());
                if f(&mut dummy) > 0 {
                    prefab.entity = dummy.entities.remove(0);
                    save_prefab(&path, &prefab)?;
                }
            }
        }
    }
    Ok(())
}

fn dunce_norm(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Set Sorting Layer and/or Order in Layer on every drawable component on `name`.
pub fn set_entity_sorting(
    scene: &mut Scene,
    name: &str,
    sorting_layer: Option<&str>,
    order_in_layer: Option<f32>,
) -> Result<()> {
    if sorting_layer.is_none() && order_in_layer.is_none() {
        bail!("set sorting: pass sorting_layer and/or order_in_layer");
    }
    let ent = scene
        .entities
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let mut any = false;
    if let Some(sp) = ent.components.sprite.as_mut() {
        any = true;
        if let Some(layer) = sorting_layer {
            sp.sorting_layer = normalize_stored_layer(layer);
        }
        if let Some(z) = order_in_layer {
            sp.z = z;
        }
    }
    if let Some(d) = ent.components.disc.as_mut() {
        any = true;
        if let Some(layer) = sorting_layer {
            d.sorting_layer = normalize_stored_layer(layer);
        }
        if let Some(z) = order_in_layer {
            d.z = z;
        }
    }
    if let Some(tm) = ent.components.tilemap.as_mut() {
        any = true;
        if let Some(layer) = sorting_layer {
            tm.sorting_layer = normalize_stored_layer(layer);
        }
        if let Some(z) = order_in_layer {
            tm.z = z;
        }
    }
    if let Some(t) = ent.components.text.as_mut() {
        any = true;
        if let Some(layer) = sorting_layer {
            t.sorting_layer = normalize_stored_layer(layer);
        }
        if let Some(z) = order_in_layer {
            t.z = z;
        }
    }
    if !any {
        bail!("entity '{name}' has no Sprite, Disc, Tilemap, or Text");
    }
    Ok(())
}

/// Validate `name` exists on the project layer list (defaults if unset).
pub fn require_sorting_layer(project: &GameProject, name: &str) -> Result<()> {
    let layers = project.effective_sorting_layers();
    let key = display_sorting_layer(name);
    if find_layer_index(&layers, key).is_some() {
        Ok(())
    } else {
        bail!("unknown sorting layer '{key}'")
    }
}

/// Drawable sort key for picking: (layer index, z).
fn hit_sort_key(layers: &[String], layer_name: &str, z: f32) -> (u16, f32) {
    (wiimaker_core::sorting_layer_index(layers, layer_name), z)
}

/// Highest (layer, z) among enabled drawables on this entity.
pub fn entity_max_sorting_key(ent: &EntityData, layers: &[String]) -> Option<(u16, f32)> {
    let mut best: Option<(u16, f32)> = None;
    let consider = |best: &mut Option<(u16, f32)>, name: &str, z: f32| {
        let key = hit_sort_key(layers, name, z);
        let better = match *best {
            None => true,
            Some(b) => wiimaker_core::cmp_sorting(key.0, key.1, b.0, b.1).is_gt(),
        };
        if better {
            *best = Some(key);
        }
    };
    if let Some(sp) = &ent.components.sprite {
        if sp.enabled {
            consider(&mut best, sp.sorting_layer_name(), sp.z);
        }
    }
    if let Some(d) = &ent.components.disc {
        if d.enabled {
            consider(&mut best, d.sorting_layer_name(), d.z);
        }
    }
    if let Some(tm) = &ent.components.tilemap {
        if tm.enabled {
            consider(&mut best, tm.sorting_layer_name(), tm.z);
        }
    }
    if let Some(t) = &ent.components.text {
        if t.enabled {
            consider(&mut best, t.sorting_layer_name(), t.z);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_component_sprite, add_entity, MutateOpts};
    use crate::scene::Scene;

    #[test]
    fn set_entity_sorting_writes_layer_and_z() {
        let mut scene = Scene::new("t");
        add_entity(&mut scene, "orb", &MutateOpts::default()).unwrap();
        add_component_sprite(&mut scene, "orb", "tex", [16.0, 16.0]).unwrap();
        set_entity_sorting(&mut scene, "orb", Some("Foreground"), Some(7.0)).unwrap();
        let sp = scene
            .find_entity("orb")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap();
        assert_eq!(sp.sorting_layer, "Foreground");
        assert!((sp.z - 7.0).abs() < 1e-4);
        set_entity_sorting(&mut scene, "orb", Some("Default"), None).unwrap();
        let sp = scene
            .find_entity("orb")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap();
        assert!(sp.sorting_layer.is_empty());
    }
}
