use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    add_component_animation, add_component_audio_source, add_component_camera,
    add_component_collider, add_component_disc, add_component_follow, add_component_grid_mover,
    add_component_sprite, add_component_text, add_component_tilemap, add_entity, apply_prefab,
    attach_prefab_instance, duplicate_entity, entities_overlap, entity_overlaps, entity_to_prefab,
    entity_triggers_entered, instantiate_prefab, load_prefab, load_prefab_for_instance,
    normalize_prefab_source, prefab_overrides, remove_component_animation,
    remove_component_audio_source, remove_component_camera, remove_component_collider,
    remove_component_disc, remove_component_follow, remove_component_grid_mover,
    remove_component_sprite, remove_component_text, remove_component_tilemap, remove_entity,
    rename_entity, resolve_prefab_asset, revert_prefab_instance, save_prefab, save_scene,
    set_component_enabled, set_entity_anim, set_entity_audio_source, set_entity_follow,
    set_entity_grid_mover, set_entity_parent, set_entity_rotation_z, set_entity_scale,
    set_entity_sorting, set_entity_text, set_entity_transform, unpack_prefab_instance, MutateOpts,
    Scene, SceneColliderKind, SceneDir, SceneTextAlign,
};

use crate::args::EntityCmd;
use crate::cmds::scene::open_scene;
use crate::util::emit_ok;

pub fn entity_cmd(root: &Path, cmd: EntityCmd, json: bool) -> Result<()> {
    match cmd {
        EntityCmd::List { game, scene } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&sc.entities)?);
            } else {
                fn print_tree(sc: &Scene, name: &str, depth: usize) {
                    let indent = "  ".repeat(depth);
                    println!("{indent}{name}");
                    for child in sc.child_names(name) {
                        print_tree(sc, &child, depth + 1);
                    }
                }
                for root_name in sc.root_names() {
                    print_tree(&sc, &root_name, 0);
                }
            }
            Ok(())
        }
        EntityCmd::Add {
            game,
            name,
            sprite,
            x,
            y,
            radius,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            add_entity(
                &mut sc,
                &name,
                &MutateOpts {
                    x,
                    y,
                    sprite,
                    radius,
                    ..Default::default()
                },
            )?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("added entity {name}"))
        }
        EntityCmd::Set {
            game,
            name,
            x,
            y,
            sx,
            sy,
            rotation_deg,
            tag,
            follow,
            lerp,
            cell,
            speed,
            queued_dir,
            audio_clip,
            volume,
            play_on_awake,
            text,
            size,
            color,
            align,
            sorting_layer,
            order_in_layer,
            scene,
        } => {
            let (_gd, project, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            if x.is_none()
                && y.is_none()
                && sx.is_none()
                && sy.is_none()
                && rotation_deg.is_none()
                && tag.is_none()
                && follow.is_none()
                && lerp.is_none()
                && cell.is_none()
                && speed.is_none()
                && queued_dir.is_none()
                && audio_clip.is_none()
                && volume.is_none()
                && play_on_awake.is_none()
                && text.is_none()
                && size.is_none()
                && color.is_none()
                && align.is_none()
                && sorting_layer.is_none()
                && order_in_layer.is_none()
            {
                bail!("entity set: pass at least one of --x --y --sx --sy --rotation-deg --tag --follow --lerp --cell --speed --queued-dir --audio-clip --volume --play-on-awake --text --size --color --align --sorting-layer --order-in-layer");
            }
            if x.is_some() || y.is_some() {
                set_entity_transform(&mut sc, &name, x, y)?;
            }
            if sx.is_some() || sy.is_some() {
                let ent = sc
                    .find_entity(&name)
                    .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
                let cur_sx = ent.transform.scale[0];
                let cur_sy = ent.transform.scale[1];
                set_entity_scale(&mut sc, &name, sx.unwrap_or(cur_sx), sy.unwrap_or(cur_sy))?;
            }
            if let Some(deg) = rotation_deg {
                set_entity_rotation_z(&mut sc, &name, deg.to_radians())?;
            }
            if let Some(tag) = tag {
                let ent = sc
                    .entities
                    .iter_mut()
                    .find(|e| e.name == name)
                    .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
                ent.tag = tag;
            }
            if follow.is_some() || lerp.is_some() {
                set_entity_follow(&mut sc, &name, follow.as_deref(), lerp)?;
            }
            if cell.is_some() || speed.is_some() || queued_dir.is_some() {
                let q = match queued_dir.as_deref() {
                    None => None,
                    Some("") => Some(None),
                    Some(s) => Some(Some(SceneDir::parse(s).ok_or_else(|| {
                        anyhow::anyhow!("unknown --queued-dir '{s}' (Up|Down|Left|Right)")
                    })?)),
                };
                set_entity_grid_mover(&mut sc, &name, cell, speed, q)?;
            }
            if audio_clip.is_some() || volume.is_some() || play_on_awake.is_some() {
                set_entity_audio_source(
                    &mut sc,
                    &name,
                    audio_clip.as_deref(),
                    volume,
                    play_on_awake,
                )?;
            }
            if text.is_some() || size.is_some() || color.is_some() || align.is_some() {
                let align = match align.as_deref() {
                    None => None,
                    Some(s) => Some(SceneTextAlign::parse(s).ok_or_else(|| {
                        anyhow::anyhow!("unknown --align '{s}' (Left|Center|Right)")
                    })?),
                };
                set_entity_text(&mut sc, &name, text.as_deref(), size, color, align)?;
            }
            if sorting_layer.is_some() || order_in_layer.is_some() {
                if let Some(layer) = sorting_layer.as_deref() {
                    wiimaker_scene::require_sorting_layer(&project, layer)?;
                }
                set_entity_sorting(&mut sc, &name, sorting_layer.as_deref(), order_in_layer)?;
            }
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("updated entity {name}"))
        }
        EntityCmd::AddComponent {
            game,
            name,
            kind,
            texture,
            width,
            height,
            radius,
            cell,
            cols,
            rows,
            shape,
            solid,
            trigger,
            filter,
            clip,
            fps,
            r#loop,
            target,
            lerp,
            speed,
            queued_dir,
            volume,
            play_on_awake,
            text,
            size,
            color,
            align,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            match kind.to_ascii_lowercase().as_str() {
                "sprite" => {
                    let tex =
                        texture.ok_or_else(|| anyhow::anyhow!("--texture required for Sprite"))?;
                    add_component_sprite(&mut sc, &name, &tex, [width, height])?;
                }
                "disc" => {
                    add_component_disc(&mut sc, &name, radius, [72, 210, 160, 255])?;
                }
                "tilemap" => {
                    add_component_tilemap(&mut sc, &name, cols, rows, cell)?;
                }
                "collider" | "trigger" => {
                    let shape = match shape.to_ascii_lowercase().as_str() {
                        "circle" => SceneColliderKind::Circle,
                        "aabb" | "box" => SceneColliderKind::Aabb,
                        other => bail!("unknown collider --shape '{other}' (Aabb|Circle)"),
                    };
                    let is_trigger = trigger || kind.eq_ignore_ascii_case("trigger");
                    let solid = if is_trigger { false } else { solid };
                    add_component_collider(
                        &mut sc,
                        &name,
                        shape,
                        [width, height],
                        radius,
                        solid,
                        is_trigger,
                        filter,
                    )?;
                }
                "animation" => {
                    let clip =
                        clip.ok_or_else(|| anyhow::anyhow!("--clip required for Animation"))?;
                    add_component_animation(&mut sc, &name, &clip, fps, r#loop)?;
                }
                "camera" => {
                    add_component_camera(&mut sc, &name, true)?;
                    if target.is_some() || lerp.is_some() {
                        set_entity_follow(&mut sc, &name, target.as_deref(), lerp)?;
                    }
                }
                "follow" => {
                    let target = target.ok_or_else(|| {
                        anyhow::anyhow!("--target required for Follow (entity name to track)")
                    })?;
                    add_component_follow(&mut sc, &name, &target, lerp)?;
                }
                "gridmover" | "grid_mover" | "grid-mover" => {
                    add_component_grid_mover(&mut sc, &name, cell, speed)?;
                    if let Some(q) = queued_dir {
                        let dir = SceneDir::parse(&q).ok_or_else(|| {
                            anyhow::anyhow!("unknown --queued-dir '{q}' (Up|Down|Left|Right)")
                        })?;
                        set_entity_grid_mover(&mut sc, &name, None, None, Some(Some(dir)))?;
                    }
                }
                "audiosource" | "audio_source" | "audio-source" | "audio" => {
                    let clip = clip.unwrap_or_default();
                    add_component_audio_source(&mut sc, &name, &clip, volume, play_on_awake)?;
                }
                "text" | "label" | "hud" => {
                    let body = text.unwrap_or_else(|| "Text".into());
                    let color = color.unwrap_or([255, 255, 255, 255]);
                    let align = match align.as_deref() {
                        None => SceneTextAlign::Left,
                        Some(s) => SceneTextAlign::parse(s).ok_or_else(|| {
                            anyhow::anyhow!("unknown --align '{s}' (Left|Center|Right)")
                        })?,
                    };
                    add_component_text(&mut sc, &name, &body, size, color, align)?;
                }
                other => {
                    bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider|Trigger|Animation|Camera|Follow|GridMover|AudioSource|Text)")
                }
            }
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("added {kind} to {name}"))
        }
        EntityCmd::Remove { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            remove_entity(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("removed entity {name}"))
        }
        EntityCmd::Duplicate { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let new_name = duplicate_entity(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    source: &'a str,
                    name: &'a str,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        source: &name,
                        name: &new_name,
                    })?
                );
                Ok(())
            } else {
                println!("duplicated {name} → {new_name}");
                Ok(())
            }
        }
        EntityCmd::Rename {
            game,
            old,
            new,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            rename_entity(&mut sc, &old, &new)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    old: &'a str,
                    name: &'a str,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        old: &old,
                        name: &new,
                    })?
                );
                Ok(())
            } else {
                println!("renamed {old} → {new}");
                Ok(())
            }
        }
        EntityCmd::SetParent {
            game,
            name,
            parent,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            set_entity_parent(&mut sc, &name, parent.as_deref())?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    parent: Option<&'a str>,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        parent: parent.as_deref(),
                    })?
                );
                Ok(())
            } else {
                match parent {
                    Some(p) => println!("parented {name} → {p}"),
                    None => println!("unparented {name} (scene root)"),
                }
                Ok(())
            }
        }
        EntityCmd::RemoveComponent {
            game,
            name,
            kind,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            match kind.to_ascii_lowercase().as_str() {
                "sprite" => remove_component_sprite(&mut sc, &name)?,
                "disc" => remove_component_disc(&mut sc, &name)?,
                "tilemap" => remove_component_tilemap(&mut sc, &name)?,
                "collider" => remove_component_collider(&mut sc, &name)?,
                "animation" => remove_component_animation(&mut sc, &name)?,
                "camera" => remove_component_camera(&mut sc, &name)?,
                "follow" => remove_component_follow(&mut sc, &name)?,
                "gridmover" | "grid_mover" | "grid-mover" => {
                    remove_component_grid_mover(&mut sc, &name)?
                }
                "audiosource" | "audio_source" | "audio-source" | "audio" => {
                    remove_component_audio_source(&mut sc, &name)?
                }
                "text" | "label" | "hud" => remove_component_text(&mut sc, &name)?,
                other => bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider|Animation|Camera|Follow|GridMover|AudioSource|Text)"),
            }
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("removed {kind} from {name}"))
        }
        EntityCmd::SetComponentEnabled {
            game,
            name,
            kind,
            enabled,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            set_component_enabled(&mut sc, &name, &kind, enabled)?;
            save_scene(&path, &sc)?;
            emit_ok(
                json,
                &format!(
                    "{} {kind} on {name}",
                    if enabled { "enabled" } else { "disabled" }
                ),
            )
        }
        EntityCmd::Overlaps {
            game,
            name,
            other,
            scene,
        } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            match other {
                Some(other) => {
                    let hit = entities_overlap(&sc, &name, &other)?;
                    if json {
                        #[derive(Serialize)]
                        struct Out<'a> {
                            ok: bool,
                            name: &'a str,
                            other: &'a str,
                            overlaps: bool,
                        }
                        println!(
                            "{}",
                            serde_json::to_string(&Out {
                                ok: true,
                                name: &name,
                                other: &other,
                                overlaps: hit,
                            })?
                        );
                    } else {
                        println!("{name} overlaps {other}: {hit}");
                    }
                    Ok(())
                }
                None => {
                    let hits = entity_overlaps(&sc, &name)?;
                    if json {
                        #[derive(Serialize)]
                        struct Out<'a> {
                            ok: bool,
                            name: &'a str,
                            overlaps: &'a [String],
                        }
                        println!(
                            "{}",
                            serde_json::to_string(&Out {
                                ok: true,
                                name: &name,
                                overlaps: &hits,
                            })?
                        );
                    } else if hits.is_empty() {
                        println!("{name}: no overlaps");
                    } else {
                        println!("{name} overlaps: {}", hits.join(", "));
                    }
                    Ok(())
                }
            }
        }
        EntityCmd::Triggers { game, name, scene } => {
            let (_gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let hits = entity_triggers_entered(&sc, &name)?;
            if json {
                #[derive(Serialize)]
                struct Out<'a> {
                    ok: bool,
                    name: &'a str,
                    triggers: &'a [String],
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: &name,
                        triggers: &hits,
                    })?
                );
                Ok(())
            } else if hits.is_empty() {
                println!("{name}: no triggers entered");
                Ok(())
            } else {
                println!("{name} triggers: {}", hits.join(", "));
                Ok(())
            }
        }
        EntityCmd::SetAnim {
            game,
            name,
            clip,
            fps,
            r#loop,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            set_entity_anim(&mut sc, &name, &clip, fps, r#loop)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("set anim {name} → {clip}"))
        }
        EntityCmd::Despawn { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            remove_entity(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("despawned entity {name}"))
        }
        EntityCmd::CreatePrefab {
            game,
            name,
            as_name,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let prefab = entity_to_prefab(&sc, &name)?;
            let stem = as_name.unwrap_or_else(|| name.clone());
            let dest = gd
                .join("assets")
                .join("prefabs")
                .join(format!("{stem}.prefab.json"));
            save_prefab(&dest, &prefab)?;
            let link = normalize_prefab_source(&stem);
            attach_prefab_instance(&mut sc, &name, &link)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    path: String,
                    prefab: String,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        path: dest.display().to_string(),
                        prefab: link,
                    })?
                );
                Ok(())
            } else {
                println!("wrote {} (instance {name})", dest.display());
                Ok(())
            }
        }
        EntityCmd::InstantiatePrefab {
            game,
            prefab,
            x,
            y,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let prefab_path = resolve_prefab_path(&gd, &prefab)?;
            let pf = load_prefab(&prefab_path)?;
            let link = normalize_prefab_source(&prefab);
            let new_name = instantiate_prefab(&mut sc, &pf, &link, x, y);
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                    prefab: String,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: new_name,
                        prefab: link,
                    })?
                );
                Ok(())
            } else {
                println!("instantiated → {new_name} ({link})");
                Ok(())
            }
        }
        EntityCmd::ApplyPrefab {
            game,
            name,
            prefab,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let source = instance_prefab_source(&sc, &name, prefab.as_deref())?;
            let prefab_path = resolve_prefab_path(&gd, &source)?;
            let mut pf = load_prefab(&prefab_path)?;
            apply_prefab(&mut sc, &name, &mut pf, &source)?;
            save_prefab(&prefab_path, &pf)?;
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    message: String,
                    prefab: String,
                    path: String,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        message: format!("applied prefab to {name}"),
                        prefab: normalize_prefab_source(&source),
                        path: prefab_path.display().to_string(),
                    })?
                );
                Ok(())
            } else {
                println!("applied {name} → {}", prefab_path.display());
                Ok(())
            }
        }
        EntityCmd::RevertPrefab {
            game,
            name,
            prefab,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let source = instance_prefab_source(&sc, &name, prefab.as_deref())?;
            let prefab_path = resolve_prefab_path(&gd, &source)?;
            let pf = load_prefab(&prefab_path)?;
            revert_prefab_instance(&mut sc, &name, &pf)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("reverted {name} from prefab"))
        }
        EntityCmd::UnpackPrefab { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            unpack_prefab_instance(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("unpacked {name}"))
        }
        EntityCmd::PrefabStatus { game, name, scene } => {
            let (gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let ent = sc
                .find_entity(&name)
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
            let source = ent.prefab.clone();
            let (instance, overrides) = if source.is_some() {
                match load_prefab_for_instance(&gd, ent) {
                    Ok(pf) => (true, prefab_overrides(ent, &pf.entity).fields),
                    Err(_) => (true, Vec::new()),
                }
            } else {
                (false, Vec::new())
            };
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                    instance: bool,
                    prefab: Option<String>,
                    overrides: Vec<String>,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name,
                        instance,
                        prefab: source,
                        overrides,
                    })?
                );
                Ok(())
            } else if instance {
                if overrides.is_empty() {
                    println!("{name} instance of {}", source.unwrap_or_default());
                } else {
                    println!(
                        "{name} instance of {} · {} overrides: {}",
                        source.unwrap_or_default(),
                        overrides.len(),
                        overrides.join(", ")
                    );
                }
                Ok(())
            } else {
                println!("{name} is not a prefab instance");
                Ok(())
            }
        }
    }
}

fn instance_prefab_source(scene: &Scene, name: &str, explicit: Option<&str>) -> Result<String> {
    if let Some(p) = explicit.filter(|s| !s.is_empty()) {
        return Ok(p.to_string());
    }
    scene
        .find_entity(name)
        .and_then(|e| e.prefab.clone())
        .ok_or_else(|| {
            anyhow::anyhow!("entity '{name}' has no prefab link (pass a prefab stem or path)")
        })
}

fn resolve_prefab_path(game_dir: &Path, prefab: &str) -> Result<std::path::PathBuf> {
    resolve_prefab_asset(game_dir, prefab)
}
