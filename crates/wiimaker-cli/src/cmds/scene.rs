use std::path::{Path, PathBuf};

use anyhow::Result;
use wiimaker_scene::{
    add_build_scene, create_named_scene, find_game_dir, list_build_scenes, list_scenes,
    load_project, load_scene, remove_build_scene, save_scene, set_default_scene, set_scene_clear,
    GameProject, Scene,
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
        SceneCmd::New { game, name } => {
            let game_dir = find_game_dir(root, &game)?;
            let rel = create_named_scene(&game_dir, &name)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("created {}", rel.display()),
                        "path": rel.to_string_lossy(),
                        "name": name,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("created {}", rel.display()))
            }
        }
        SceneCmd::SetDefault { game, scene } => {
            let game_dir = find_game_dir(root, &game)?;
            let rel = set_default_scene(&game_dir, &scene)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("default scene → {}", rel.display()),
                        "default_scene": rel.to_string_lossy(),
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("default scene → {}", rel.display()))
            }
        }
        SceneCmd::BuildList { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let scenes = list_build_scenes(&game_dir)?;
            let names: Vec<String> = scenes
                .iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "scenes": names,
                        "default_scene": project.default_scene,
                    })
                );
            } else if names.is_empty() {
                println!("(no scenes in build list — authoring uses scene list / filesystem)");
                println!("default: {}", project.default_scene);
            } else {
                for n in &names {
                    if n == &project.default_scene {
                        println!("* {n}  (default)");
                    } else {
                        println!("  {n}");
                    }
                }
            }
            Ok(())
        }
        SceneCmd::BuildAdd { game, scene } => {
            let game_dir = find_game_dir(root, &game)?;
            let rel = add_build_scene(&game_dir, &scene)?;
            let project = load_project(&game_dir)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("build scenes += {}", rel.display()),
                        "path": rel.to_string_lossy(),
                        "scenes": project.scenes,
                        "default_scene": project.default_scene,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("build scenes += {}", rel.display()))
            }
        }
        SceneCmd::BuildRemove { game, scene } => {
            let game_dir = find_game_dir(root, &game)?;
            let rel = remove_build_scene(&game_dir, &scene)?;
            let project = load_project(&game_dir)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("build scenes -= {}", rel.display()),
                        "path": rel.to_string_lossy(),
                        "scenes": project.scenes,
                        "default_scene": project.default_scene,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("build scenes -= {}", rel.display()))
            }
        }
        SceneCmd::SetClear { game, rgb } => {
            let (_gd, _p, path, mut scene) = open_scene(root, &game, None)?;
            set_scene_clear(&mut scene, rgb);
            save_scene(&path, &scene)?;
            emit_ok(json, "scene clear updated")
        }
    }
}
