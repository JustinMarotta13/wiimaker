//! Controller override-layer rename/remove + scene Animator weight overrides.

use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_assets::{
    apply_remove_controller_layer, apply_rename_controller_layer, AnimatorControllerMeta,
};

use crate::authoring_remap::remap_authoring_files;
use crate::project::load_project;
use crate::scene::{EntityData, Scene, SceneAnimatorLayer};

/// Rename a layer-weight override on one entity's Animator when it uses `controller`.
///
/// When `to` already has an override, the renamed layer's weight wins and `from` is removed.
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
    let from_weight = anim
        .layers
        .iter()
        .find(|l| l.name == from)
        .map(|l| l.weight);
    if from_weight.is_none() {
        return 0;
    }
    let from_weight = from_weight.unwrap();
    anim.layers.retain(|l| l.name != from);
    if let Some(layer) = anim.layers.iter_mut().find(|l| l.name == to) {
        layer.weight = from_weight;
    } else {
        anim.layers.push(SceneAnimatorLayer {
            name: to.to_string(),
            weight: from_weight,
        });
    }
    1
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
    let to = to.trim();
    if to.is_empty() {
        anyhow::bail!("animator layer name must not be empty");
    }
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
