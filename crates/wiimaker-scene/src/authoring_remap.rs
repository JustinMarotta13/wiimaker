//! Remap scene + prefab JSON when project-level authoring changes.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::project::list_scenes;
use crate::scene::{load_prefab, load_scene, save_prefab, save_scene, Prefab, Scene};

/// Apply `f` to every scene and prefab entity tree under `game_dir`.
///
/// `skip_scene` (absolute path) is left untouched so a dirty editor can remap in memory.
pub fn remap_authoring_files(
    game_dir: &Path,
    skip_scene: Option<&Path>,
    mut f: impl FnMut(&mut Scene) -> usize,
) -> Result<()> {
    let skip = skip_scene.map(dunce_norm);
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
