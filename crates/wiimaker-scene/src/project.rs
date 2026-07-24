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
