use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    save_scene, tilemap_fill, tilemap_get_cell, tilemap_set_cell, tilemap_stamp,
    tilemap_stamp_ascii,
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
                    let (id, solid) = tilemap_get_cell(&sc, &name, x, y)?;
                    if json {
                        #[derive(Serialize)]
                        struct Out<'a> {
                            name: &'a str,
                            x: i32,
                            y: i32,
                            id: u16,
                            solid: bool,
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
                                cell: tm.cell,
                                width: tm.width,
                                height: tm.height,
                            })?
                        );
                    } else {
                        println!("{name} ({x},{y}) id={id} solid={solid}");
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
    }
}
