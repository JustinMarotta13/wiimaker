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
    /// Ordered Scenes in Build list (`game.toml` `scenes = [...]`). Empty = unset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<String>,
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
            scenes: Vec::new(),
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

/// Simple scene stem: no path separators or dots (extension lives on the file).
pub fn validate_scene_stem(name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("scene name required");
    }
    if name.contains('/') || name.contains('\\') || name.contains('.') {
        bail!("scene name must be a simple stem (no path or extension)");
    }
    Ok(())
}

/// Create `scenes/<name>.scene.json`. Returns the path relative to `game_dir`.
pub fn create_named_scene(game_dir: &Path, name: &str) -> Result<PathBuf> {
    validate_scene_stem(name)?;
    let stem = name.trim();
    let rel = PathBuf::from("scenes").join(format!("{stem}.scene.json"));
    let abs = game_dir.join(&rel);
    if abs.exists() {
        bail!("scene already exists: {}", rel.display());
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::scene::save_scene(&abs, &crate::scene::Scene::new(stem))?;
    Ok(rel)
}

/// Resolve a scene stem or relative path to a path relative to `game_dir`.
pub fn resolve_scene_rel(game_dir: &Path, scene: &str) -> Result<PathBuf> {
    let scene = scene.trim();
    if scene.is_empty() {
        bail!("scene name required");
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if scene.ends_with(".scene.json") {
        let as_path = PathBuf::from(scene);
        candidates.push(as_path.clone());
        if as_path.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
            candidates.push(PathBuf::from("scenes").join(&as_path));
        }
    } else {
        candidates.push(PathBuf::from("scenes").join(format!("{scene}.scene.json")));
        candidates.push(PathBuf::from(format!("{scene}.scene.json")));
    }
    for rel in &candidates {
        if game_dir.join(rel).is_file() {
            return Ok(rel.clone());
        }
    }
    bail!("scene not found: {scene}");
}

fn norm_rel(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn listed_matches(listed: &str, key: &str) -> bool {
    let a = listed.replace('\\', "/");
    let b = key.replace('\\', "/");
    if a == b {
        return true;
    }
    let a_stem = Path::new(&a)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(a.as_str());
    let b_stem = Path::new(&b)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(b.as_str());
    a_stem == b_stem || a_stem.trim_end_matches(".scene.json") == b.trim_end_matches(".scene.json")
}

/// Persist `game.toml` `default_scene` (Build Settings analogue).
pub fn set_default_scene(game_dir: &Path, scene: &str) -> Result<PathBuf> {
    let rel = resolve_scene_rel(game_dir, scene)?;
    let mut project = load_project(game_dir)?;
    project.default_scene = norm_rel(&rel);
    save_project(game_dir, &project)?;
    Ok(rel)
}

/// Ordered `game.toml` `scenes` list (empty when omitted).
pub fn list_build_scenes(game_dir: &Path) -> Result<Vec<PathBuf>> {
    let project = load_project(game_dir)?;
    Ok(project.scenes.iter().map(PathBuf::from).collect())
}

/// Append a scene to the Build Settings list and save `game.toml`.
pub fn add_build_scene(game_dir: &Path, scene: &str) -> Result<PathBuf> {
    let rel = resolve_scene_rel(game_dir, scene)?;
    let key = norm_rel(&rel);
    let mut project = load_project(game_dir)?;
    if !project.scenes.iter().any(|s| listed_matches(s, &key)) {
        project.scenes.push(key);
        save_project(game_dir, &project)?;
    }
    Ok(rel)
}

/// Remove a scene from the Build Settings list and save `game.toml`.
pub fn remove_build_scene(game_dir: &Path, scene: &str) -> Result<PathBuf> {
    let mut project = load_project(game_dir)?;
    let key = match resolve_scene_rel(game_dir, scene) {
        Ok(rel) => norm_rel(&rel),
        Err(_) => scene.trim().replace('\\', "/"),
    };
    let before = project.scenes.len();
    let mut removed = key.clone();
    project.scenes.retain(|s| {
        if listed_matches(s, &key) {
            removed = s.replace('\\', "/");
            false
        } else {
            true
        }
    });
    if project.scenes.len() == before {
        bail!("scene not in build list: {scene}");
    }
    save_project(game_dir, &project)?;
    Ok(PathBuf::from(removed))
}

/// Replace the Build Settings list (each entry must resolve to an existing scene file).
pub fn set_build_scenes(game_dir: &Path, scenes: &[impl AsRef<str>]) -> Result<Vec<PathBuf>> {
    let mut resolved = Vec::new();
    let mut keys = Vec::new();
    for scene in scenes {
        let rel = resolve_scene_rel(game_dir, scene.as_ref())?;
        let key = norm_rel(&rel);
        if !keys.iter().any(|k: &String| listed_matches(k, &key)) {
            keys.push(key);
            resolved.push(rel);
        }
    }
    let mut project = load_project(game_dir)?;
    project.scenes = keys;
    save_project(game_dir, &project)?;
    Ok(resolved)
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

    #[test]
    fn create_named_scene_writes_json_under_scenes() {
        let dir = tempfile_dir("create-named-scene");
        fs::write(
            dir.join("game.toml"),
            "name = \"t\"\ndefault_scene = \"scenes/main.scene.json\"\n",
        )
        .unwrap();
        let rel = create_named_scene(&dir, "win").unwrap();
        assert_eq!(rel, PathBuf::from("scenes/win.scene.json"));
        assert!(dir.join(&rel).is_file());
        let scene = crate::scene::load_scene(&dir.join(&rel)).unwrap();
        assert_eq!(scene.name, "win");
        assert!(create_named_scene(&dir, "win").is_err());
        assert!(create_named_scene(&dir, "bad.name").is_err());
    }

    #[test]
    fn set_default_scene_updates_game_toml() {
        let dir = tempfile_dir("set-default-scene");
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::write(dir.join("scenes/main.scene.json"), "{\"name\":\"main\"}\n").unwrap();
        fs::write(dir.join("scenes/menu.scene.json"), "{\"name\":\"menu\"}\n").unwrap();
        fs::write(
            dir.join("game.toml"),
            "name = \"t\"\ndefault_scene = \"scenes/main.scene.json\"\n",
        )
        .unwrap();

        let rel = set_default_scene(&dir, "menu").unwrap();
        assert_eq!(rel, PathBuf::from("scenes/menu.scene.json"));
        let project = load_project(&dir).unwrap();
        assert_eq!(project.default_scene, "scenes/menu.scene.json");

        let rel = set_default_scene(&dir, "scenes/main.scene.json").unwrap();
        assert_eq!(rel, PathBuf::from("scenes/main.scene.json"));
        assert!(set_default_scene(&dir, "missing").is_err());
    }

    #[test]
    fn build_scenes_roundtrip_and_omit_when_empty() {
        let dir = tempfile_dir("build-scenes");
        fs::create_dir_all(dir.join("scenes")).unwrap();
        fs::write(dir.join("scenes/main.scene.json"), "{\"name\":\"main\"}\n").unwrap();
        fs::write(dir.join("scenes/menu.scene.json"), "{\"name\":\"menu\"}\n").unwrap();
        fs::write(
            dir.join("game.toml"),
            "name = \"t\"\ndefault_scene = \"scenes/main.scene.json\"\n",
        )
        .unwrap();

        let project = load_project(&dir).unwrap();
        assert!(project.scenes.is_empty());
        assert!(list_build_scenes(&dir).unwrap().is_empty());

        add_build_scene(&dir, "menu").unwrap();
        add_build_scene(&dir, "scenes/main.scene.json").unwrap();
        add_build_scene(&dir, "menu").unwrap(); // idempotent
        let listed = list_build_scenes(&dir).unwrap();
        assert_eq!(
            listed,
            vec![
                PathBuf::from("scenes/menu.scene.json"),
                PathBuf::from("scenes/main.scene.json"),
            ]
        );
        let toml_text = fs::read_to_string(dir.join("game.toml")).unwrap();
        assert!(toml_text.contains("scenes"));

        remove_build_scene(&dir, "menu").unwrap();
        assert_eq!(
            list_build_scenes(&dir).unwrap(),
            vec![PathBuf::from("scenes/main.scene.json")]
        );
        set_build_scenes(&dir, &["menu", "main"]).unwrap();
        assert_eq!(list_build_scenes(&dir).unwrap().len(), 2);
        assert!(set_build_scenes(&dir, &["nope"]).is_err());
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
