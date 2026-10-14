//! Remap scene + prefab JSON when project-level authoring changes.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::project::{list_scenes, load_project};
use crate::scene::{load_prefab, load_scene, save_prefab, save_scene, EntityData, Prefab, Scene};

/// Visit the root entity and every nested prefab child.
pub fn remap_prefab_entities(prefab: &mut Prefab, mut f: impl FnMut(&mut EntityData) -> usize) -> usize {
    let mut n = f(&mut prefab.entity);
    for child in &mut prefab.children {
        n += f(child);
    }
    n
}

/// Visit every entity in a scene (flat store).
pub fn remap_scene_entities(scene: &mut Scene, mut f: impl FnMut(&mut EntityData) -> usize) -> usize {
    let mut n = 0;
    for ent in &mut scene.entities {
        n += f(ent);
    }
    n
}

/// Load every scene and prefab, apply `on_entity`, then save only files that changed.
///
/// All authoring files are loaded and mutated before any write (all-or-nothing planning).
/// `skip_scene` (absolute path) is left untouched so a dirty editor can remap in memory.
pub fn remap_authoring_files(
    game_dir: &Path,
    skip_scene: Option<&Path>,
    mut on_entity: impl FnMut(&mut EntityData) -> usize,
) -> Result<()> {
    let project = load_project(game_dir)?;
    let prefab_dir = project.assets_path(game_dir).join("prefabs");
    let skip = skip_scene.map(dunce_norm);

    let mut scenes: Vec<(PathBuf, Scene)> = Vec::new();
    for rel in list_scenes(game_dir)? {
        let abs = game_dir.join(&rel);
        if skip.as_ref().is_some_and(|s| dunce_norm(&abs) == *s) {
            continue;
        }
        if !abs.is_file() {
            continue;
        }
        let scene = load_scene(&abs)?;
        scenes.push((abs, scene));
    }

    let mut prefabs: Vec<(PathBuf, Prefab)> = Vec::new();
    if prefab_dir.is_dir() {
        for entry in fs::read_dir(&prefab_dir)? {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".prefab.json"))
            {
                let loaded = load_prefab(&path)?;
                prefabs.push((path, loaded));
            }
        }
    }

    let mut dirty_scenes = Vec::new();
    for (path, scene) in &mut scenes {
        if remap_scene_entities(scene, &mut on_entity) > 0 {
            dirty_scenes.push(path.clone());
        }
    }

    let mut dirty_prefabs = Vec::new();
    for (path, prefab) in &mut prefabs {
        if remap_prefab_entities(prefab, &mut on_entity) > 0 {
            dirty_prefabs.push(path.clone());
        }
    }

    for path in dirty_scenes {
        let scene = scenes
            .iter()
            .find(|(p, _)| p == &path)
            .map(|(_, s)| s)
            .expect("dirty scene path");
        save_scene(&path, scene)?;
    }
    for path in dirty_prefabs {
        let prefab = prefabs
            .iter()
            .find(|(p, _)| p == &path)
            .map(|(_, p)| p)
            .expect("dirty prefab path");
        save_prefab(&path, prefab)?;
    }

    Ok(())
}

fn dunce_norm(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}
