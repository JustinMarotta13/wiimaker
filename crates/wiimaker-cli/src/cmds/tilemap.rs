use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    save_scene, tilemap_autotile_mask, tilemap_fill, tilemap_from_ascii_path, tilemap_set_cell,
    tilemap_set_palette, tilemap_stamp, tilemap_stamp_ascii, AsciiCharMap, TilePaletteOpts,
};

use crate::args::TilemapCmd;
use crate::cmds::scene::open_scene;

pub fn tilemap_cmd(root: &Path, cmd: TilemapCmd, json: bool) -> Result<()> {
    match cmd {
        TilemapCmd::Set {
            game,
            name,
            x,
            y,
            id,
            solid,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let is_solid = solid.unwrap_or(id != 0);
            let prev = tilemap_set_cell(&mut sc, &name, x, y, id, is_solid)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    x: i32,
                    y: i32,
                    id: u16,
                    solid: bool,
                    prev_id: u16,
                    prev_solid: bool,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        x,
                        y,
                        id,
                        solid: is_solid,
                        prev_id: prev.0,
                        prev_solid: prev.1,
                    })?
                );
                Ok(())
            } else {
                println!("{name} ({x},{y}) → id {id} solid {is_solid}");
                Ok(())
            }
        }
        TilemapCmd::Fill {
            game,
            name,
            x,
            y,
            w,
            h,
            id,
            solid,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let is_solid = solid.unwrap_or(id != 0);
            let n = tilemap_fill(&mut sc, &name, x, y, w, h, id, is_solid)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    filled: u32,
                    id: u16,
                    solid: bool,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        filled: n,
                        id,
                        solid: is_solid,
                    })?
                );
                Ok(())
            } else {
                println!("filled {n} cells on {name}");
                Ok(())
            }
        }
        TilemapCmd::Stamp {
            game,
            name,
            x,
            y,
            ascii,
            cells,
            width,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let n = if let Some(ascii) = ascii {
                tilemap_stamp_ascii(&mut sc, &name, x, y, &ascii)?
            } else if let Some(cells) = cells {
                let parsed: Result<Vec<u16>, _> =
                    cells.split(',').map(|s| s.trim().parse::<u16>()).collect();
                let parsed = parsed.map_err(|e| anyhow::anyhow!("--cells: {e}"))?;
                let w = width.ok_or_else(|| anyhow::anyhow!("--width required with --cells"))?;
                tilemap_stamp(&mut sc, &name, x, y, w, &parsed, None)?
            } else {
                bail!("tilemap stamp: pass --ascii or --cells + --width");
            };
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    stamped: u32,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        stamped: n,
                    })?
                );
                Ok(())
            } else {
                println!("stamped {n} cells on {name}");
                Ok(())
            }
        }
        TilemapCmd::FromAscii {
            game,
            file,
            name,
            x,
            y,
            resize,
            map,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let ascii_path = resolve_ascii_path(&gd, &file)?;
            let char_map = match map.as_deref() {
                Some(spec) if !spec.trim().is_empty() => Some(AsciiCharMap::parse(spec)?),
                _ => None,
            };
            let out = tilemap_from_ascii_path(
                &mut sc,
                &name,
                &ascii_path,
                x,
                y,
                resize,
                char_map.as_ref(),
            )?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    file: String,
                    stamped: u32,
                    width: u32,
                    height: u32,
                    x: i32,
                    y: i32,
                    resized: bool,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        file: ascii_path.display().to_string(),
                        stamped: out.stamped,
                        width: out.width,
                        height: out.height,
                        x,
                        y,
                        resized: out.resized,
                    })?
                );
                Ok(())
            } else {
                println!(
                    "from-ascii {} → {name} {}×{} ({} cells{})",
                    ascii_path.display(),
                    out.width,
                    out.height,
                    out.stamped,
                    if out.resized { ", resized" } else { "" }
                );
                Ok(())
            }
        }
        TilemapCmd::Get {
            game,
            name,
            x,
            y,
            scene,
        } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let ent = sc
                .find_entity(&name)
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
            let tm = ent
                .components
                .tilemap
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Tilemap"))?;
            match (x, y) {
                (Some(x), Some(y)) => {
                    let (id, solid, mask, rule) = tilemap_autotile_mask(&sc, &name, x, y)?;
                    if json {
                        #[derive(Serialize)]
                        struct Out<'a> {
                            name: &'a str,
                            x: i32,
                            y: i32,
                            id: u16,
                            solid: bool,
                            mask: u8,
                            auto_tile: Option<&'a str>,
                            cell: f32,
                            width: u32,
                            height: u32,
                        }
                        println!(
                            "{}",
                            serde_json::to_string(&Out {
                                name: &name,
                                x,
                                y,
                                id,
                                solid,
                                mask,
                                auto_tile: rule.map(|m| m.as_str()),
                                cell: tm.cell,
                                width: tm.width,
                                height: tm.height,
                            })?
                        );
                    } else {
                        let extra = rule
                            .map(|m| format!(" auto-tile={} mask={mask}", m.as_str()))
                            .unwrap_or_else(|| format!(" mask={mask}"));
                        println!("{name} ({x},{y}) id={id} solid={solid}{extra}");
                    }
                }
                (None, None) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(tm)?);
                    } else {
                        println!(
                            "{name} {}x{} cell={} origin={:?} z={}",
                            tm.width, tm.height, tm.cell, tm.origin, tm.z
                        );
                        let occupied = tm.cells.iter().filter(|c| **c != 0).count();
                        let solid_n = tm.solid.iter().filter(|s| **s != 0).count();
                        println!("  occupied {occupied} · solid {solid_n}");
                    }
                }
                _ => bail!("tilemap get: pass both --x and --y, or neither"),
            }
            Ok(())
        }
        TilemapCmd::SetPalette {
            game,
            name,
            id,
            sprite,
            color,
            anim,
            fps,
            auto_tile,
            auto_sprites,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let sprites = auto_sprites.map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .collect::<Vec<_>>()
            });
            let pal = tilemap_set_palette(
                &mut sc,
                &name,
                id,
                &TilePaletteOpts {
                    sprite,
                    color,
                    anim,
                    anim_fps: fps,
                    auto_tile,
                    auto_sprites: sprites,
                },
            )?;
            save_scene(&path, &sc)?;
            if json {
                println!("{}", serde_json::to_string(&pal)?);
            } else {
                let anim = pal.anim_clip().unwrap_or("-");
                let rule = pal.auto_tile.map(|m| m.as_str()).unwrap_or("off");
                println!(
                    "{name} palette id {} sprite={} anim={anim} auto-tile={rule}",
                    pal.id,
                    pal.sprite.as_deref().unwrap_or("(color)")
                );
            }
            Ok(())
        }
        TilemapCmd::Mask {
            game,
            name,
            x,
            y,
            scene,
        } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let (id, solid, mask, rule) = tilemap_autotile_mask(&sc, &name, x, y)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    name: &'a str,
                    x: i32,
                    y: i32,
                    id: u16,
                    solid: bool,
                    mask: u8,
                    n: bool,
                    e: bool,
                    s: bool,
                    w: bool,
                    auto_tile: Option<&'a str>,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        name: &name,
                        x,
                        y,
                        id,
                        solid,
                        mask,
                        n: mask & 1 != 0,
                        e: mask & 2 != 0,
                        s: mask & 4 != 0,
                        w: mask & 8 != 0,
                        auto_tile: rule.map(|m| m.as_str()),
                    })?
                );
            } else {
                let rule = rule.map(|m| m.as_str()).unwrap_or("id");
                println!("{name} ({x},{y}) id={id} solid={solid} auto-tile={rule} mask={mask}");
            }
            Ok(())
        }
    }
}

fn resolve_ascii_path(game_dir: &Path, file: &Path) -> Result<PathBuf> {
    if file.is_file() {
        return Ok(file.to_path_buf());
    }
    let under_game = game_dir.join(file);
    if under_game.is_file() {
        return Ok(under_game);
    }
    let under_assets = game_dir.join("assets").join(file);
    if under_assets.is_file() {
        return Ok(under_assets);
    }
    let name = file
        .file_name()
        .map(|n| game_dir.join("assets").join(n))
        .filter(|p| p.is_file());
    if let Some(p) = name {
        return Ok(p);
    }
    bail!(
        "ASCII map not found: {} (tried cwd, {}, {})",
        file.display(),
        under_game.display(),
        under_assets.display()
    );
}
