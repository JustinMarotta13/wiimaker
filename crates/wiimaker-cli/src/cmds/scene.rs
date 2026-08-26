use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_scene::{
    find_game_dir, list_scenes, load_project, load_scene, save_scene, set_scene_clear, GameProject,
    Scene,
};

use crate::args::SceneCmd;
use crate::util::emit_ok;

pub fn open_scene(
    root: &Path,
    game: &str,
    scene_rel: Option<&str>,
) -> Result<(PathBuf, GameProject, PathBuf, Scene)> {
    let game_dir = find_game_dir(root, game)?;
    let project = load_project(&game_dir)?;
    let scene_path = match scene_rel {
        Some(rel) => game_dir.join(rel),
        None => project.scene_path(&game_dir),
    };
    let scene = load_scene(&scene_path)?;
    Ok((game_dir, project, scene_path, scene))
}

pub fn scene_cmd(root: &Path, cmd: SceneCmd, json: bool) -> Result<()> {
    match cmd {
        SceneCmd::List { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let scenes = list_scenes(&game_dir)?;
            let names: Vec<String> = scenes
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in &names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        SceneCmd::Show { game, scene } => {
            let (_gd, _p, path, scene) = open_scene(root, &game, scene.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&scene)?);
            } else {
                println!("{} ({})", scene.name, path.display());
                println!("clear {:?}", scene.clear_color);
                for e in &scene.entities {
                    println!("  - {}", e.name);
                }
            }
            Ok(())
        }
        SceneCmd::SetClear { game, rgb } => {
            let (_gd, _p, path, mut scene) = open_scene(root, &game, None)?;
            set_scene_clear(&mut scene, rgb);
            save_scene(&path, &scene)?;
            emit_ok(json, "scene clear updated")
        }
    }
}
