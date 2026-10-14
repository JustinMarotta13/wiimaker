//! Controller parameter rename/remove + scene Animator overrides.

use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_assets::{
    AnimatorControllerMeta, remove_controller_param, rename_controller_param,
};

use crate::authoring_remap::remap_authoring_files;
use crate::scene::{EntityData, Scene};

/// Rename a parameter override on every Animator using `controller` in `scene`.
pub fn remap_scene_animator_param(
    scene: &mut Scene,
    controller: &str,
    from: &str,
    to: &str,
) -> usize {
    if from == to {
        return 0;
    }
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remap_entity_animator_param(ent, controller, from, to);
    }
    n
}

fn remap_entity_animator_param(
    ent: &mut EntityData,
    controller: &str,
    from: &str,
    to: &str,
) -> usize {
    let Some(anim) = ent.components.animator.as_mut() else {
        return 0;
    };
    if anim.controller != controller {
        return 0;
    }
    let mut n = 0;
    for p in &mut anim.parameters {
        if p.name == from {
            p.name = to.to_string();
            n += 1;
        }
    }
    n
}

/// Drop a parameter override on every Animator using `controller` in `scene`.
pub fn remove_scene_animator_param(scene: &mut Scene, controller: &str, name: &str) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remove_entity_animator_param(ent, controller, name);
    }
    n
}

fn remove_entity_animator_param(ent: &mut EntityData, controller: &str, name: &str) -> usize {
    let Some(anim) = ent.components.animator.as_mut() else {
        return 0;
    };
    if anim.controller != controller {
        return 0;
    }
    let before = anim.parameters.len();
    anim.parameters.retain(|p| p.name != name);
    before - anim.parameters.len()
}

/// Rename a controller parameter and retarget scene overrides project-wide.
pub fn rename_controller_param_for_game(
    game_dir: &Path,
    controller: &str,
    from: &str,
    to: &str,
    skip_scene: Option<&Path>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let assets = game_dir.join("assets");
    let (path, meta) = rename_controller_param(&assets, controller, from, to)?;
    remap_authoring_files(game_dir, skip_scene, |scene| {
        remap_scene_animator_param(scene, controller, from, to)
    })?;
    Ok((path, meta))
}

/// Remove a controller parameter and drop matching scene overrides project-wide.
pub fn remove_controller_param_for_game(
    game_dir: &Path,
    controller: &str,
    name: &str,
    skip_scene: Option<&Path>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let assets = game_dir.join("assets");
    let (path, meta) = remove_controller_param(&assets, controller, name)?;
    remap_authoring_files(game_dir, skip_scene, |scene| {
        remove_scene_animator_param(scene, controller, name)
    })?;
    Ok((path, meta))
}
