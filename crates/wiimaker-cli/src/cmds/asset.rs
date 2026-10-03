use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use wiimaker_assets::{
    inspect_wav, list_anim_clips, list_animator_controllers, list_wav_clips, resolve_wav,
    set_sprite_pivot, slice_sheet, spawn_wav_player, write_anim_clip, write_animator_controller,
    AnimatorControllerMeta, ControllerCondition, ControllerParam, ControllerParamType,
    ControllerState, ControllerTransition, SpriteCatalog,
};
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
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let dest = match ext.as_str() {
                "wav" => assets.join(format!("{stem}.wav")),
                "png" | "" => assets.join(format!("{stem}.png")),
                other => anyhow::bail!("asset import: expected .png or .wav, got .{other}"),
            };
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
        AssetCmd::Controller {
            game,
            name,
            default_state,
            states,
            params,
            transition,
            stdin,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let meta = if stdin {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                    .context("read controller JSON from stdin")?;
                serde_json::from_str(&buf).context("parse controller JSON from stdin")?
            } else {
                parse_controller_flags(default_state, states, params, transition)?
            };
            let (path, meta) = write_animator_controller(&assets, &name, meta)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    name: String,
                    #[serde(rename = "default")]
                    default_state: String,
                    states: usize,
                    parameters: usize,
                    transitions: usize,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        name,
                        default_state: meta.default_state,
                        states: meta.states.len(),
                        parameters: meta.parameters.len(),
                        transitions: meta.transitions.len(),
                    })?
                );
            } else {
                println!(
                    "wrote controller {} ({} states, {} params, {} transitions) → {}",
                    name,
                    meta.states.len(),
                    meta.parameters.len(),
                    meta.transitions.len(),
                    path.display()
                );
            }
            Ok(())
        }
        AssetCmd::ListControllers { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let names = list_animator_controllers(&assets)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AssetCmd::ListWavs { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let names = list_wav_clips(&assets)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&names)?);
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AssetCmd::Play {
            game,
            name,
            volume,
            wait,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let path = resolve_wav(&assets, &name)?;
            inspect_wav(&path)?;
            let disabled = std::env::var("WIIMAKER_AUDIO")
                .ok()
                .map(|v| {
                    let v = v.trim();
                    v == "0" || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("false")
                })
                .unwrap_or(false);
            let skipped = if disabled {
                true
            } else {
                spawn_wav_player(&path)?.is_none()
            };
            if wait && !skipped {
                if let Ok(info) = inspect_wav(&path) {
                    let secs = info.duration_secs().min(5.0);
                    if secs > 0.0 {
                        std::thread::sleep(std::time::Duration::from_secs_f32(secs + 0.05));
                    }
                }
            }
            if json {
                #[derive(Serialize)]
                struct Out {
                    played: String,
                    path: String,
                    volume: f32,
                    skipped: bool,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        played: name,
                        path: path.display().to_string(),
                        volume,
                        skipped,
                    })?
                );
            } else if skipped {
                println!(
                    "audio skipped (no device / WIIMAKER_AUDIO=0) {}",
                    path.display()
                );
            } else {
                println!("played {} ({})", name, path.display());
            }
            Ok(())
        }
    }
}

fn parse_controller_flags(
    default_state: Option<String>,
    states: Option<String>,
    params: Vec<String>,
    transitions: Vec<String>,
) -> Result<AnimatorControllerMeta> {
    let states_spec = states.ok_or_else(|| {
        anyhow::anyhow!("asset controller: pass --states Name:clip,... or --stdin")
    })?;
    let mut parsed_states = Vec::new();
    for part in states_spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (sname, clip) = part
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("--states entry '{part}' must be Name:clip"))?;
        parsed_states.push(ControllerState {
            name: sname.trim().to_string(),
            clip: clip.trim().to_string(),
            speed: 1.0,
        });
    }
    if parsed_states.is_empty() {
        anyhow::bail!("asset controller: --states must list at least one Name:clip");
    }
    let default_state = default_state.unwrap_or_else(|| parsed_states[0].name.clone());
    let mut parameters = Vec::new();
    for spec in params {
        // Name:Bool=false  or Name:Float=0
        let spec = spec.trim();
        let (head, def) = spec.split_once('=').unwrap_or((spec, ""));
        let (pname, ty) = head.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("--param '{spec}' must be Name:Bool[=false] or Name:Float[=0]")
        })?;
        let kind = ControllerParamType::parse(ty)
            .ok_or_else(|| anyhow::anyhow!("--param '{spec}' unknown type (Bool|Float|Trigger)"))?;
        let default = if def.is_empty() {
            None
        } else {
            match kind {
                ControllerParamType::Float => {
                    let n: f64 = def
                        .trim()
                        .parse()
                        .map_err(|_| anyhow::anyhow!("--param '{spec}' default is not a number"))?;
                    Some(serde_json::json!(n))
                }
                ControllerParamType::Bool | ControllerParamType::Trigger => {
                    let b = match def.trim().to_ascii_lowercase().as_str() {
                        "true" | "1" | "yes" => true,
                        "false" | "0" | "no" => false,
                        other => anyhow::bail!("--param '{spec}' bool default '{other}'"),
                    };
                    Some(serde_json::json!(b))
                }
            }
        };
        parameters.push(ControllerParam {
            name: pname.trim().to_string(),
            kind,
            default,
        });
    }
    let mut parsed_transitions = Vec::new();
    for spec in transitions {
        // Idle>Walk:Moving=true
        let spec = spec.trim();
        let (edge, conds) = spec.split_once(':').unwrap_or((spec, ""));
        let (from, to) = edge.split_once('>').ok_or_else(|| {
            anyhow::anyhow!("--transition '{spec}' must be From>To[:Param=value,...]")
        })?;
        let mut conditions = Vec::new();
        if !conds.is_empty() {
            for c in conds.split(',') {
                let c = c.trim();
                if c.is_empty() {
                    continue;
                }
                let (param, val) = c.split_once('=').ok_or_else(|| {
                    anyhow::anyhow!("--transition condition '{c}' must be Param=value")
                })?;
                let val = val.trim();
                let mut cc = ControllerCondition {
                    param: param.trim().to_string(),
                    ..Default::default()
                };
                if val.eq_ignore_ascii_case("true") {
                    cc.equals = Some(serde_json::json!(true));
                } else if val.eq_ignore_ascii_case("false") {
                    cc.equals = Some(serde_json::json!(false));
                } else if let Ok(n) = val.parse::<f64>() {
                    cc.equals = Some(serde_json::json!(n));
                } else {
                    anyhow::bail!("--transition condition '{c}' value must be bool or number");
                }
                conditions.push(cc);
            }
        }
        parsed_transitions.push(ControllerTransition {
            from: from.trim().to_string(),
            to: to.trim().to_string(),
            conditions,
            has_exit_time: false,
        });
    }
    Ok(AnimatorControllerMeta {
        default_state,
        parameters,
        states: parsed_states,
        transitions: parsed_transitions,
    })
}
