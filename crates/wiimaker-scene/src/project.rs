//! `game.toml` project metadata.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameProject {
    pub name: String,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_scene")]
    pub default_scene: String,
    #[serde(default = "default_assets")]
    pub assets_dir: String,
    #[serde(default = "default_wpack")]
    pub wpack: String,
    #[serde(default = "default_wscn")]
    pub wscn: String,
}

fn default_title() -> String {
    "wiimaker".into()
}
fn default_scene() -> String {
    "scenes/main.scene.json".into()
}
fn default_assets() -> String {
    "assets".into()
}
fn default_wpack() -> String {
    "assets.wpack".into()
}
fn default_wscn() -> String {
    "scene.wscn".into()
}

impl GameProject {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            title: format!("wiimaker · {name}"),
            name,
            default_scene: default_scene(),
            assets_dir: default_assets(),
            wpack: default_wpack(),
            wscn: default_wscn(),
        }
    }

    pub fn scene_path(&self, game_dir: &Path) -> PathBuf {
        game_dir.join(&self.default_scene)
    }

    pub fn assets_path(&self, game_dir: &Path) -> PathBuf {
        game_dir.join(&self.assets_dir)
    }

    pub fn wpack_path(&self, game_dir: &Path) -> PathBuf {
        game_dir.join(&self.wpack)
    }

    pub fn wscn_path(&self, game_dir: &Path) -> PathBuf {
        game_dir.join(&self.wscn)
    }
}

pub fn load_project(game_dir: &Path) -> Result<GameProject> {
    let path = game_dir.join("game.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let project: GameProject = toml::from_str(&text).context("parse game.toml")?;
    Ok(project)
}

pub fn save_project(game_dir: &Path, project: &GameProject) -> Result<()> {
    fs::create_dir_all(game_dir)?;
    let path = game_dir.join("game.toml");
    let text = toml::to_string_pretty(project).context("serialize game.toml")?;
    fs::write(&path, text)?;
    Ok(())
}

fn is_scene_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".scene.json"))
}

/// Discover `*.scene.json` under `game_dir/scenes/`, plus `project.default_scene` if present.
/// Returns paths relative to `game_dir`, sorted.
pub fn list_scenes(game_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut scenes: Vec<PathBuf> = Vec::new();
    let scenes_dir = game_dir.join("scenes");
    if scenes_dir.is_dir() {
        for entry in fs::read_dir(&scenes_dir)
            .with_context(|| format!("read {}", scenes_dir.display()))?
        {
            let path = entry?.path();
            if path.is_file() && is_scene_file(&path) {
                if let Ok(rel) = path.strip_prefix(game_dir) {
                    scenes.push(rel.to_path_buf());
                }
            }
        }
    }

    if let Ok(project) = load_project(game_dir) {
        let default_rel = PathBuf::from(&project.default_scene);
        let default_abs = game_dir.join(&default_rel);
        if default_abs.is_file() && !scenes.iter().any(|s| s == &default_rel) {
            scenes.push(default_rel);
        }
    }

    scenes.sort();
    scenes.dedup();
    Ok(scenes)
}

/// Resolve `games/<name>` from workspace root or a path that already points at a game.
pub fn find_game_dir(root: &Path, name_or_path: &str) -> Result<PathBuf> {
    let as_path = Path::new(name_or_path);
    if as_path.join("game.toml").is_file() {
        return Ok(as_path.to_path_buf());
    }
    let under_games = root.join("games").join(name_or_path);
    if under_games.join("game.toml").is_file() {
        return Ok(under_games);
    }
    // Allow games that exist but lack game.toml yet (migration / new).
    if under_games.is_dir() {
        return Ok(under_games);
    }
    bail!("game not found: {name_or_path} (expected games/{name_or_path}/game.toml)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_scenes_finds_scene_json_under_scenes() {
        let dir = tempfile_dir("list-scenes");
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::write(dir.join("scenes/main.scene.json"), "{\"name\":\"main\"}\n").unwrap();
        fs::write(dir.join("scenes/menu.scene.json"), "{\"name\":\"menu\"}\n").unwrap();
        fs::write(dir.join("scenes/notes.json"), "{}\n").unwrap(); // not a scene
        fs::write(
            dir.join("game.toml"),
            "name = \"t\"\ndefault_scene = \"scenes/main.scene.json\"\n",
        )
        .unwrap();

        let listed = list_scenes(&dir).unwrap();
        assert_eq!(
            listed,
            vec![
                PathBuf::from("scenes/main.scene.json"),
                PathBuf::from("scenes/menu.scene.json"),
            ]
        );
    }

    #[test]
    fn list_scenes_includes_default_outside_scenes_dir() {
        let dir = tempfile_dir("list-scenes-default");
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::write(dir.join("scenes/main.scene.json"), "{\"name\":\"main\"}\n").unwrap();
        fs::write(dir.join("intro.scene.json"), "{\"name\":\"intro\"}\n").unwrap();
        fs::write(
            dir.join("game.toml"),
            "name = \"t\"\ndefault_scene = \"intro.scene.json\"\n",
        )
        .unwrap();

        let listed = list_scenes(&dir).unwrap();
        assert_eq!(
            listed,
            vec![
                PathBuf::from("intro.scene.json"),
                PathBuf::from("scenes/main.scene.json"),
            ]
        );
    }

    #[test]
    fn list_scenes_empty_when_no_scenes_dir() {
        let dir = tempfile_dir("list-scenes-empty");
        fs::write(dir.join("game.toml"), "name = \"t\"\n").unwrap();
        let listed = list_scenes(&dir).unwrap();
        assert!(listed.is_empty());
    }

    fn tempfile_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wiimaker-scene-{}-{}",
            label,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
