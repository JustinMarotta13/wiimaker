//! Controller parameter rename/remove + scene Animator overrides.

use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_assets::{
    apply_remove_controller_param, apply_rename_controller_param, AnimatorControllerMeta,
};

use crate::authoring_remap::remap_authoring_files;
use crate::project::load_project;
use crate::scene::{EntityData, Scene};

/// Rename a parameter override on one entity's Animator when it uses `controller`.
pub fn remap_entity_animator_param(
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
    let mut n = 0;
    for p in &mut anim.parameters {
        if p.name == from {
            p.name = to.to_string();
            n += 1;
        }
    }
    n
}

/// Rename a parameter override on every Animator using `controller` in `scene`.
pub fn remap_scene_animator_param(
    scene: &mut Scene,
    controller: &str,
    from: &str,
    to: &str,
) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remap_entity_animator_param(ent, controller, from, to);
    }
    n
}

/// Drop a parameter override on one entity's Animator when it uses `controller`.
pub fn remove_entity_animator_param(ent: &mut EntityData, controller: &str, name: &str) -> usize {
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

/// Drop a parameter override on every Animator using `controller` in `scene`.
pub fn remove_scene_animator_param(scene: &mut Scene, controller: &str, name: &str) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += remove_entity_animator_param(ent, controller, name);
    }
    n
}

/// Rename a controller parameter and retarget scene overrides project-wide.
pub fn rename_controller_param_for_game(
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
    apply_rename_controller_param(&mut meta, controller, from, to)?;
    remap_authoring_files(game_dir, skip_scene, |ent| {
        remap_entity_animator_param(ent, controller, from, to)
    })?;
    meta.save(&path)?;
    Ok((path, meta))
}

/// Remove a controller parameter and drop matching scene overrides project-wide.
pub fn remove_controller_param_for_game(
    game_dir: &Path,
    controller: &str,
    name: &str,
    skip_scene: Option<&Path>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let project = load_project(game_dir)?;
    let assets = project.assets_path(game_dir);
    let path = AnimatorControllerMeta::path(&assets, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    apply_remove_controller_param(&mut meta, controller, name)?;
    remap_authoring_files(game_dir, skip_scene, |ent| {
        remove_entity_animator_param(ent, controller, name)
    })?;
    meta.save(&path)?;
    Ok((path, meta))
}
