//! Controller override-layer rename/remove + scene Animator weight overrides.

use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_assets::{
    apply_remove_controller_layer, apply_rename_controller_layer, AnimatorControllerMeta,
};

use crate::authoring_remap::remap_authoring_files;
use crate::project::load_project;
use crate::scene::{EntityData, Scene};

/// Rename a layer-weight override on one entity's Animator when it uses `controller`.
///
/// If `to` is already present, the `from` entry is dropped (destination keeps its weight).
pub fn remap_entity_animator_layer(
    ent: &mut EntityData,
    controller: &str,
    from: &str,
    to: &str,
) -> usize {
    if from == to {
        return 0;
    }
    let Some(anim) = ent.components.animator.as_mut() else {
        return 0;
    };
    if anim.controller != controller {
        return 0;
    }
    let has_to = anim.layers.iter().any(|l| l.name == to);
    if has_to {
        let before = anim.layers.len();
        anim.layers.retain(|l| l.name != from);
        return before - anim.layers.len();
    }
    let mut n = 0;
    for layer in &mut anim.layers {
        if layer.name == from {
            layer.name = to.to_string();
            n += 1;
        }
    }
    n
}

/// Rename a layer-weight override on every Animator using `controller` in `scene`.
pub fn remap_scene_animator_layer(
    scene: &mut Scene,
    controller: &str,
    from: &str,
    to: &str,
) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remap_entity_animator_layer(ent, controller, from, to);
    }
    n
}

/// Drop a layer-weight override on one entity's Animator when it uses `controller`.
pub fn remove_entity_animator_layer(ent: &mut EntityData, controller: &str, name: &str) -> usize {
    let Some(anim) = ent.components.animator.as_mut() else {
        return 0;
    };
    if anim.controller != controller {
        return 0;
    }
    let before = anim.layers.len();
    anim.layers.retain(|l| l.name != name);
    before - anim.layers.len()
}

/// Drop a layer-weight override on every Animator using `controller` in `scene`.
pub fn remove_scene_animator_layer(scene: &mut Scene, controller: &str, name: &str) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remove_entity_animator_layer(ent, controller, name);
    }
    n
}

/// Rename a controller override layer and retarget scene weight overrides project-wide.
pub fn rename_controller_layer_for_game(
    game_dir: &Path,
    controller: &str,
    from: &str,
    to: &str,
    skip_scene: Option<&Path>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let project = load_project(game_dir)?;
    let assets = project.assets_path(game_dir);
    let path = AnimatorControllerMeta::path(&assets, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    apply_rename_controller_layer(&mut meta, controller, from, to)?;
    remap_authoring_files(game_dir, skip_scene, |ent| {
        remap_entity_animator_layer(ent, controller, from, to)
    })?;
    meta.save(&path)?;
    Ok((path, meta))
}

/// Remove a controller override layer and drop matching scene weight overrides project-wide.
pub fn remove_controller_layer_for_game(
    game_dir: &Path,
    controller: &str,
    name: &str,
    skip_scene: Option<&Path>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let project = load_project(game_dir)?;
    let assets = project.assets_path(game_dir);
    let path = AnimatorControllerMeta::path(&assets, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    apply_remove_controller_layer(&mut meta, controller, name)?;
    remap_authoring_files(game_dir, skip_scene, |ent| {
        remove_entity_animator_layer(ent, controller, name)
    })?;
    meta.save(&path)?;
    Ok((path, meta))
}
