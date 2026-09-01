use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use wiimaker_assets::{list_anim_clips, set_sprite_pivot, slice_sheet, write_anim_clip, SpriteCatalog};
use wiimaker_scene::{find_game_dir, load_project};

use crate::args::AssetCmd;

pub fn asset_cmd(root: &Path, cmd: AssetCmd, json: bool) -> Result<()> {
    match cmd {
        AssetCmd::List { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let mut names = Vec::new();
            if assets.is_dir() {
                for entry in fs::read_dir(&assets)? {
                    let path = entry?.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("png") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
            names.sort();
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AssetCmd::Import { game, path, name } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            fs::create_dir_all(&assets)?;
            let stem = name.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("tex")
                    .to_string()
            });
            let dest = assets.join(format!("{stem}.png"));
            fs::copy(&path, &dest)
                .with_context(|| format!("copy {} → {}", path.display(), dest.display()))?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    imported: String,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        imported: dest.display().to_string()
                    })?
                );
            } else {
                println!("imported {}", dest.display());
            }
            Ok(())
        }
        AssetCmd::Slice {
            game,
            sheet,
            cols,
            rows,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let (path, meta, warnings) = slice_sheet(&assets, &sheet, cols, rows)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    columns: u32,
                    rows: u32,
                    sprites: Vec<String>,
                    warnings: Vec<String>,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        columns: meta.columns,
                        rows: meta.rows,
                        sprites: meta.sprites.iter().map(|s| s.name.clone()).collect(),
                        warnings,
                    })?
                );
            } else {
                for w in &warnings {
                    println!("warn {w}");
                }
                println!(
                    "sliced {} → {} ({} cells)",
                    sheet,
                    path.display(),
                    meta.sprites.len()
                );
            }
            Ok(())
        }
        AssetCmd::SetPivot { game, sprite, x, y } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let path = set_sprite_pivot(&assets, &sprite, [x, y])?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    sprite: String,
                    pivot: [f32; 2],
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        sprite,
                        pivot: [x, y],
                    })?
                );
            } else {
                println!("set pivot {sprite} → ({x}, {y}) in {}", path.display());
            }
            Ok(())
        }
        AssetCmd::ListSprites { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let catalog = SpriteCatalog::load_dir(&assets, |_| None)?;
            let names = catalog.names();
            if json {
                println!("{}", serde_json::to_string_pretty(names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AssetCmd::Anim {
            game,
            name,
            cells,
            fps,
            r#loop,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let cell_list: Vec<String> = cells
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let (path, meta) = write_anim_clip(&assets, &name, cell_list, fps, r#loop)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    name: String,
                    fps: f32,
                    #[serde(rename = "loop")]
                    loop_: bool,
                    cells: Vec<String>,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        name,
                        fps: meta.fps,
                        loop_: meta.loop_,
                        cells: meta.cells,
                    })?
                );
            } else {
                println!(
                    "wrote anim {} ({} cells @ {} fps, loop={}) → {}",
                    name,
                    meta.cells.len(),
                    meta.fps,
                    meta.loop_,
                    path.display()
                );
            }
            Ok(())
        }
        AssetCmd::ListAnims { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let names = list_anim_clips(&assets)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
    }
}
