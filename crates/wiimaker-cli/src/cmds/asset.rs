use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use wiimaker_assets::{
    add_controller_layer, add_layer_transition, inspect_wav, list_anim_clips,
    list_animator_controllers, list_timelines, list_wav_clips, remove_controller_layer,
    remove_layer_state, remove_layer_transition, rename_controller_layer, resolve_wav,
    set_controller_layer_weight, set_layer_default, set_sprite_pivot, slice_sheet,
    spawn_wav_player, write_anim_clip, write_animator_controller, write_layer_state,
    write_layer_state_blend_tree, write_layer_state_clip, write_timeline, AnimatorControllerMeta,
    BlendDimension, BlendMotion, BlendTreeMeta, ControllerCondition, ControllerParam,
    ControllerParamType, ControllerState, ControllerTransition, CurveEdit, CurveInterp, CurveProp,
    SpriteCatalog, TimelineClip, TimelineMeta, TimelineTrack, TimelineTrackKind,
};
use wiimaker_scene::{find_game_dir, load_project};

use crate::args::{AssetCmd, ControllerLayerCmd, TimelineCurveCmd};

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
        AssetCmd::BlendTree {
            game,
            controller,
            state,
            dimension,
            params,
            motions,
            as_clip,
            layer,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let layer_ref = layer.as_deref();
            let (path, meta) = match as_clip {
                Some(clip) => {
                    write_layer_state_clip(&assets, &controller, layer_ref, &state, &clip)?
                }
                None => {
                    let tree = parse_blend_flags(&dimension, params.as_deref(), &motions)?;
                    write_layer_state_blend_tree(&assets, &controller, layer_ref, &state, tree)?
                }
            };
            let saved = layer_state(&meta, layer_ref, &state)
                .ok_or_else(|| anyhow::anyhow!("state '{state}' missing after write"))?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    controller: String,
                    state: String,
                    clip: String,
                    blend_tree: Option<wiimaker_assets::BlendTreeMeta>,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        controller: controller.clone(),
                        state: state.clone(),
                        clip: saved.clip.clone(),
                        blend_tree: saved.blend_tree.clone(),
                    })?
                );
            } else if let Some(tree) = &saved.blend_tree {
                println!(
                    "wrote blend tree {} on {}.{} ({} param(s), {} motion(s)) → {}",
                    tree.dimension.as_str(),
                    controller,
                    state,
                    tree.params.len(),
                    tree.motions.len(),
                    path.display()
                );
            } else {
                println!(
                    "wrote state {}.{} as clip {} → {}",
                    controller,
                    state,
                    saved.clip,
                    path.display()
                );
            }
            Ok(())
        }
        AssetCmd::ControllerLayer {
            game,
            controller,
            cmd,
        } => controller_layer_cmd(root, &game, &controller, cmd, json),
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
        AssetCmd::Timeline {
            game,
            name,
            duration,
            tracks,
            stdin,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let meta = if stdin {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                    .context("read timeline JSON from stdin")?;
                serde_json::from_str(&buf).context("parse timeline JSON from stdin")?
            } else {
                parse_timeline_flags(duration, &tracks)?
            };
            let (path, meta) = write_timeline(&assets, &name, meta)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    path: String,
                    name: String,
                    duration: f32,
                    tracks: usize,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        path: path.display().to_string(),
                        name,
                        duration: meta.duration,
                        tracks: meta.tracks.len(),
                    })?
                );
            } else {
                println!(
                    "wrote timeline {} ({:.2}s, {} tracks) → {}",
                    name,
                    meta.duration,
                    meta.tracks.len(),
                    path.display()
                );
            }
            Ok(())
        }
        AssetCmd::TimelineCurve { cmd } => timeline_curve_cmd(root, cmd, json),
        AssetCmd::ListTimelines { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let assets = project.assets_path(&game_dir);
            let names = list_timelines(&assets)?;
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

fn layer_state<'a>(
    meta: &'a AnimatorControllerMeta,
    layer: Option<&str>,
    state: &str,
) -> Option<&'a ControllerState> {
    match layer
        .map(str::trim)
        .filter(|n| !wiimaker_assets::is_base_layer_name(n))
    {
        None => meta.state(state),
        Some(name) => meta
            .layer(name)
            .and_then(|l| l.states.iter().find(|s| s.name == state)),
    }
}

fn controller_layer_cmd(
    root: &Path,
    game: &str,
    controller: &str,
    cmd: ControllerLayerCmd,
    json: bool,
) -> Result<()> {
    let game_dir = find_game_dir(root, game)?;
    let project = load_project(&game_dir)?;
    let assets = project.assets_path(&game_dir);
    match cmd {
        ControllerLayerCmd::List => {
            let path = AnimatorControllerMeta::path(&assets, controller);
            let meta = AnimatorControllerMeta::load(&path)?;
            if json {
                #[derive(Serialize)]
                struct Row {
                    name: String,
                    weight: f32,
                    #[serde(rename = "default")]
                    default_state: String,
                    states: usize,
                    transitions: usize,
                }
                #[derive(Serialize)]
                struct Out {
                    controller: String,
                    layers: Vec<Row>,
                }
                let mut layers = vec![Row {
                    name: wiimaker_assets::BASE_LAYER_NAME.into(),
                    weight: meta.weight,
                    default_state: meta.default_state.clone(),
                    states: meta.states.len(),
                    transitions: meta.transitions.len(),
                }];
                for layer in &meta.layers {
                    layers.push(Row {
                        name: layer.name.clone(),
                        weight: layer.weight,
                        default_state: layer.default_state.clone(),
                        states: layer.states.len(),
                        transitions: layer.transitions.len(),
                    });
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        controller: controller.to_string(),
                        layers,
                    })?
                );
            } else {
                println!(
                    "Base weight={} default={} states={} transitions={}",
                    meta.weight,
                    meta.default_state,
                    meta.states.len(),
                    meta.transitions.len()
                );
                for layer in &meta.layers {
                    println!(
                        "{} weight={} default={} states={} transitions={}",
                        layer.name,
                        layer.weight,
                        layer.default_state,
                        layer.states.len(),
                        layer.transitions.len()
                    );
                }
            }
            Ok(())
        }
        ControllerLayerCmd::Add { name, weight } => {
            let (path, meta) = add_controller_layer(&assets, controller, &name, weight)?;
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("added layer {name}"),
            )
        }
        ControllerLayerCmd::Remove { name } => {
            let (path, meta) = remove_controller_layer(&assets, controller, &name)?;
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("removed layer {name}"),
            )
        }
        ControllerLayerCmd::Rename { name, to } => {
            let (path, meta) = rename_controller_layer(&assets, controller, &name, &to)?;
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("renamed layer {name} → {to}"),
            )
        }
        ControllerLayerCmd::Weight { name, weight } => {
            let (path, meta) = set_controller_layer_weight(&assets, controller, &name, weight)?;
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("set {name} weight {weight}"),
            )
        }
        ControllerLayerCmd::State {
            name,
            state,
            clip,
            remove,
        } => {
            let layer = Some(name.as_str());
            let (path, meta) = if remove {
                remove_layer_state(&assets, controller, layer, &state)?
            } else {
                let clip = clip.ok_or_else(|| {
                    anyhow::anyhow!("asset controller-layer state: pass --clip or --remove")
                })?;
                write_layer_state(&assets, controller, layer, &state, &clip)?
            };
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("updated state {state} on {name}"),
            )
        }
        ControllerLayerCmd::SetDefault { name, state } => {
            let (path, meta) = set_layer_default(&assets, controller, Some(&name), &state)?;
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("default {name} → {state}"),
            )
        }
        ControllerLayerCmd::Transition {
            name,
            from,
            to,
            conditions,
            has_exit_time,
            remove_index,
        } => {
            let (path, meta) = if let Some(index) = remove_index {
                remove_layer_transition(&assets, controller, Some(&name), index)?
            } else {
                let from = from.ok_or_else(|| {
                    anyhow::anyhow!("asset controller-layer transition: pass --from and --to")
                })?;
                let to = to.ok_or_else(|| {
                    anyhow::anyhow!("asset controller-layer transition: pass --from and --to")
                })?;
                add_layer_transition(
                    &assets,
                    controller,
                    Some(&name),
                    ControllerTransition {
                        from,
                        to,
                        conditions: parse_conditions(&conditions)?,
                        has_exit_time,
                    },
                )?
            };
            emit_layer(
                json,
                &path,
                controller,
                &meta,
                &format!("updated transitions on {name}"),
            )
        }
    }
}

fn emit_layer(
    json: bool,
    path: &Path,
    controller: &str,
    meta: &AnimatorControllerMeta,
    message: &str,
) -> Result<()> {
    if json {
        #[derive(Serialize)]
        struct Out {
            path: String,
            controller: String,
            layers: usize,
            message: String,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                path: path.display().to_string(),
                controller: controller.to_string(),
                layers: meta.layers.len(),
                message: message.to_string(),
            })?
        );
    } else {
        println!("{message} → {}", path.display());
    }
    Ok(())
}

fn parse_conditions(specs: &[String]) -> Result<Vec<ControllerCondition>> {
    let mut conditions = Vec::new();
    for spec in specs {
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        let (param, val) = spec
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--condition '{spec}' must be Param=value"))?;
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
            anyhow::bail!("--condition '{spec}' value must be bool or number");
        }
        conditions.push(cc);
    }
    Ok(conditions)
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
        parsed_states.push(ControllerState::plain(sname.trim(), clip.trim()));
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
        weight: 1.0,
        layers: Vec::new(),
    })
}

/// `--type 1D --params Speed --motion idle:0 --motion walk:1` or `--type 2D --params DirX,DirY --motion up:0,-1`.
fn parse_blend_flags(
    dimension: &str,
    params: Option<&str>,
    motions: &[String],
) -> Result<BlendTreeMeta> {
    let dimension = BlendDimension::parse(dimension)
        .ok_or_else(|| anyhow::anyhow!("asset blend-tree: --type must be 1D or 2D"))?;
    let params_spec = params
        .ok_or_else(|| anyhow::anyhow!("asset blend-tree: pass --params (Speed or DirX,DirY)"))?;
    let params: Vec<String> = params_spec
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let mut parsed = Vec::new();
    for spec in motions {
        let (clip, value) = spec.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("--motion '{spec}' must be clip:threshold or clip:x,y")
        })?;
        let clip = clip.trim().to_string();
        let value = value.trim();
        let motion = match dimension {
            BlendDimension::OneD => {
                let t: f32 = value
                    .parse()
                    .map_err(|_| anyhow::anyhow!("--motion '{spec}' threshold is not a number"))?;
                BlendMotion {
                    clip,
                    threshold: Some(t),
                    position: None,
                }
            }
            BlendDimension::TwoD => {
                let (x, y) = value.split_once(',').ok_or_else(|| {
                    anyhow::anyhow!("--motion '{spec}' 2D position must be clip:x,y")
                })?;
                let x: f32 = x
                    .trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("--motion '{spec}' x is not a number"))?;
                let y: f32 = y
                    .trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("--motion '{spec}' y is not a number"))?;
                BlendMotion {
                    clip,
                    threshold: None,
                    position: Some([x, y]),
                }
            }
        };
        parsed.push(motion);
    }
    Ok(BlendTreeMeta {
        dimension,
        params,
        motions: parsed,
    })
}

fn parse_timeline_flags(duration: f32, tracks: &[String]) -> Result<TimelineMeta> {
    let mut out: Vec<TimelineTrack> = Vec::new();
    for spec in tracks {
        let mut parts = spec.splitn(5, ':');
        let name = parts.next().unwrap_or("").trim();
        let kind_s = parts.next().unwrap_or("").trim();
        let binding_s = parts.next().unwrap_or("").trim();
        let range = parts.next().unwrap_or("").trim();
        let payload = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            anyhow::bail!("--track '{spec}' needs Name:Kind:Binding:start-end:payload");
        }
        let kind = TimelineTrackKind::parse(kind_s).ok_or_else(|| {
            anyhow::anyhow!(
                "--track '{spec}' kind must be Activation|Animation|Audio|Transform|Signal|Control|Float"
            )
        })?;
        let (start_s, end_s) = range.split_once('-').ok_or_else(|| {
            anyhow::anyhow!("--track '{spec}' range must be start-end (e.g. 0.5-4)")
        })?;
        let start: f32 = start_s
            .trim()
            .parse()
            .with_context(|| format!("--track '{spec}' start"))?;
        let end: f32 = end_s
            .trim()
            .parse()
            .with_context(|| format!("--track '{spec}' end"))?;
        let binding = if binding_s.is_empty() || binding_s == "-" {
            None
        } else {
            Some(binding_s.to_string())
        };
        let (payload, curve_spec) = match kind {
            TimelineTrackKind::Transform | TimelineTrackKind::Float => {
                match payload.split_once('|') {
                    Some((head, rest)) => (head.trim(), Some(rest)),
                    None => (payload, None),
                }
            }
            _ => (payload, None),
        };
        let mut property = None;
        let mut clip = match kind {
            TimelineTrackKind::Activation => {
                let active = if payload.is_empty() {
                    true
                } else {
                    payload.eq_ignore_ascii_case("true") || payload == "1"
                };
                TimelineClip::activation(start, end, active)
            }
            TimelineTrackKind::Animation => {
                if payload.is_empty() {
                    anyhow::bail!("--track '{spec}' animation payload is a clip stem");
                }
                TimelineClip::animation(start, end, payload)
            }
            TimelineTrackKind::Audio => {
                let (stem, volume) = if let Some((stem, vol)) = payload.split_once(':') {
                    let volume: f32 = vol
                        .trim()
                        .parse()
                        .with_context(|| format!("--track '{spec}' volume"))?;
                    (stem.trim(), volume)
                } else {
                    (payload, 1.0)
                };
                if stem.is_empty() {
                    anyhow::bail!("--track '{spec}' audio payload is a wav stem");
                }
                TimelineClip::audio(start, end, stem, volume)
            }
            TimelineTrackKind::Transform => {
                let (from_s, to_s) = payload.split_once('>').ok_or_else(|| {
                    anyhow::anyhow!("--track '{spec}' transform payload must be x,y>x,y")
                })?;
                TimelineClip::transform(start, end, parse_xy(from_s)?, parse_xy(to_s)?)
            }
            TimelineTrackKind::Float => {
                if payload.is_empty() {
                    anyhow::bail!(
                        "--track '{spec}' float payload is a property (Transform.rotation|value:...)"
                    );
                }
                property = Some(payload.to_string());
                TimelineClip::control(start, end)
            }
            TimelineTrackKind::Signal => {
                let (name, extra) = payload.split_once('|').unwrap_or((payload, ""));
                if name.trim().is_empty() {
                    anyhow::bail!(
                        "--track '{spec}' signal payload is SignalName or SignalName|text (range is t-t)"
                    );
                }
                let extra = extra.trim();
                let payload = if extra.is_empty() { None } else { Some(extra) };
                TimelineClip::signal(start, name.trim(), payload)
            }
            TimelineTrackKind::Control => TimelineClip::control(start, end),
        };
        if let Some(spec) = curve_spec {
            wiimaker_assets::apply_curve_suffix(&mut clip, spec)
                .with_context(|| format!("--track '{spec}' curves"))?;
        }
        if let Some(existing) = out.iter_mut().find(|t| {
            t.name == name && t.kind == kind && t.binding == binding && t.property == property
        }) {
            existing.clips.push(clip);
        } else {
            out.push(TimelineTrack {
                name: name.to_string(),
                kind,
                binding,
                property,
                clips: vec![clip],
            });
        }
    }
    Ok(TimelineMeta {
        duration,
        tracks: out,
    })
}

fn timeline_curve_cmd(root: &Path, cmd: TimelineCurveCmd, json: bool) -> Result<()> {
    match cmd {
        TimelineCurveCmd::Sample {
            game,
            timeline,
            track,
            clip,
            prop,
            from,
            to,
            steps,
        } => {
            let (_path, meta) = load_timeline(root, &game, &timeline)?;
            let prop = parse_curve_prop(&prop)?;
            let samples =
                wiimaker_assets::curve_sample(&meta, &track, clip, prop, from, to, steps)?;
            if json {
                #[derive(Serialize)]
                struct Sample {
                    t: f32,
                    v: f32,
                }
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    timeline: String,
                    track: String,
                    clip: usize,
                    prop: String,
                    samples: Vec<Sample>,
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        ok: true,
                        timeline,
                        track,
                        clip,
                        prop: prop.as_str().to_string(),
                        samples: samples
                            .into_iter()
                            .map(|s| Sample { t: s.t, v: s.v })
                            .collect(),
                    })?
                );
            } else {
                for s in samples {
                    println!("{:.4}\t{:.4}", s.t, s.v);
                }
            }
            Ok(())
        }
        other => {
            let (game, timeline) = {
                let (game, timeline) = curve_target(&other);
                (game.to_string(), timeline.to_string())
            };
            let (path, mut meta) = load_timeline(root, &game, &timeline)?;
            let edit = match other {
                TimelineCurveCmd::AddKey {
                    track,
                    clip,
                    prop,
                    t,
                    v,
                    interp,
                    ..
                } => wiimaker_assets::curve_add_key(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                    t,
                    v,
                    parse_curve_interp(&interp)?,
                )?,
                TimelineCurveCmd::RemoveKey {
                    track,
                    clip,
                    prop,
                    index,
                    t,
                    ..
                } => wiimaker_assets::curve_remove_key(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                    index,
                    t,
                )?,
                TimelineCurveCmd::MoveKey {
                    track,
                    clip,
                    prop,
                    index,
                    t,
                    v,
                    ..
                } => wiimaker_assets::curve_move_key(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                    index,
                    t,
                    v,
                )?,
                TimelineCurveCmd::SetInterp {
                    track,
                    clip,
                    prop,
                    index,
                    interp,
                    ..
                } => wiimaker_assets::curve_set_interp(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                    index,
                    parse_curve_interp(&interp)?,
                )?,
                TimelineCurveCmd::AddCurve {
                    track, clip, prop, ..
                } => wiimaker_assets::curve_add_curve(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                )?,
                TimelineCurveCmd::RemoveCurve {
                    track, clip, prop, ..
                } => wiimaker_assets::curve_remove_curve(
                    &mut meta,
                    &track,
                    clip,
                    parse_curve_prop(&prop)?,
                )?,
                TimelineCurveCmd::Sample { .. } => unreachable!(),
            };
            meta.save(&path)?;
            print_curve_edit(json, &timeline, &path, &edit)
        }
    }
}

fn curve_target(cmd: &TimelineCurveCmd) -> (&str, &str) {
    match cmd {
        TimelineCurveCmd::AddKey { game, timeline, .. }
        | TimelineCurveCmd::RemoveKey { game, timeline, .. }
        | TimelineCurveCmd::MoveKey { game, timeline, .. }
        | TimelineCurveCmd::SetInterp { game, timeline, .. }
        | TimelineCurveCmd::AddCurve { game, timeline, .. }
        | TimelineCurveCmd::RemoveCurve { game, timeline, .. }
        | TimelineCurveCmd::Sample { game, timeline, .. } => (game, timeline),
    }
}

fn load_timeline(
    root: &Path,
    game: &str,
    name: &str,
) -> Result<(std::path::PathBuf, TimelineMeta)> {
    let game_dir = find_game_dir(root, game)?;
    let project = load_project(&game_dir)?;
    let assets = project.assets_path(&game_dir);
    let path = TimelineMeta::path(&assets, name);
    let meta = TimelineMeta::load(&path)?;
    Ok((path, meta))
}

fn parse_curve_prop(s: &str) -> Result<CurveProp> {
    CurveProp::parse(s).ok_or_else(|| anyhow::anyhow!("--prop must be x|y|value, got '{s}'"))
}

fn parse_curve_interp(s: &str) -> Result<CurveInterp> {
    let interp = CurveInterp::parse(s);
    if !interp.is_known() {
        anyhow::bail!("--interp must be linear|constant|ease, got '{s}'");
    }
    Ok(interp)
}

fn print_curve_edit(json: bool, timeline: &str, path: &Path, edit: &CurveEdit) -> Result<()> {
    if json {
        #[derive(Serialize)]
        struct KeyOut {
            t: f32,
            v: f32,
            interp: String,
        }
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            path: String,
            timeline: String,
            track: String,
            clip: usize,
            prop: String,
            index: Option<usize>,
            replaced: bool,
            keys: Vec<KeyOut>,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                ok: true,
                path: path.display().to_string(),
                timeline: timeline.to_string(),
                track: edit.track.clone(),
                clip: edit.clip,
                prop: edit.prop.as_str().to_string(),
                index: edit.index,
                replaced: edit.replaced,
                keys: edit
                    .keys
                    .iter()
                    .map(|k| KeyOut {
                        t: k.t,
                        v: k.v,
                        interp: k.interp.as_str().to_string(),
                    })
                    .collect(),
            })?
        );
    } else {
        let note = if edit.replaced { " replaced" } else { "" };
        println!(
            "{} clip {} {} ({} keys){note}",
            edit.track,
            edit.clip,
            edit.prop.as_str(),
            edit.keys.len()
        );
    }
    Ok(())
}

fn parse_xy(s: &str) -> Result<[f32; 2]> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| anyhow::anyhow!("expected x,y, got '{s}'"))?;
    Ok([
        x.trim().parse().with_context(|| format!("x in '{s}'"))?,
        y.trim().parse().with_context(|| format!("y in '{s}'"))?,
    ])
}
