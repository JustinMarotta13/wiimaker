use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    add_component_animation, add_component_animator, add_component_audio_source,
    add_component_camera, add_component_collider, add_component_disc, add_component_follow,
    add_component_grid_mover, add_component_playable_director, add_component_sprite,
    add_component_text, add_component_tilemap, add_entity, apply_prefab, attach_prefab_instance,
    create_prefab_variant, duplicate_entity, entities_overlap, entity_overlaps, entity_to_prefab,
    entity_triggers_entered, instantiate_prefab, load_prefab, load_prefab_for_instance,
    normalize_prefab_source, prefab_tree_overrides, refresh_variant_overrides,
    remove_component_animation, remove_component_animator, remove_component_audio_source,
    remove_component_camera, remove_component_collider, remove_component_disc,
    remove_component_follow, remove_component_grid_mover, remove_component_playable_director,
    remove_component_sprite, remove_component_text, remove_component_tilemap, remove_entity,
    rename_entity, resolve_prefab, resolve_prefab_asset, revert_prefab_instance, save_prefab,
    save_scene, set_component_enabled, set_entity_anim, set_entity_animator_bool,
    set_entity_animator_float, set_entity_audio_source, set_entity_controller, set_entity_follow,
    set_entity_grid_mover, set_entity_parent, set_entity_playable_director, set_entity_rotation_z,
    set_entity_scale, set_entity_sorting, set_entity_sprite_pivot, set_entity_text,
    set_entity_transform, unpack_prefab_instance, variant_from_instance, MutateOpts, Scene,
    SceneColliderKind, SceneDir, SceneTextAlign,
};

use crate::args::EntityCmd;
use crate::cmds::scene::open_scene;
use crate::util::emit_ok;

fn timeline_transport(
    root: &Path,
    game: &str,
    name: &str,
    scene: Option<&str>,
    json: bool,
    play: bool,
    stop: bool,
) -> Result<()> {
    let (gd, project, _path, sc) = open_scene(root, game, scene)?;
    let ent = sc
        .find_entity(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let scene_d = ent
        .components
        .playable_director
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no PlayableDirector"))?;
    let assets = project.assets_path(&gd);
    let anims = wiimaker_assets::AnimClipCatalog::load_dir(&assets).unwrap_or_default();
    let timelines = wiimaker_assets::TimelineCatalog::load_dir(&assets).unwrap_or_default();
    let mut world = wiimaker_scene::hydrate_lenient_with_all_catalogs(
        &sc,
        &wiimaker_scene::TextureMap::new(),
        None,
        Some(&anims),
        None,
        None,
        Some(&timelines),
    );
    let id = world
        .find_by_name(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' missing after hydrate"))?;
    if play {
        world.play_timeline(id);
    } else if stop {
        world.stop_timeline(id);
    }
    let d = world
        .director(id)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no runtime PlayableDirector"))?;
    #[derive(Serialize)]
    struct Out {
        ok: bool,
        name: String,
        timeline: String,
        time: f32,
        playing: bool,
        finished: bool,
        #[serde(rename = "play_on_awake")]
        play_on_awake: bool,
        #[serde(rename = "loop")]
        loop_: bool,
    }
    let out = Out {
        ok: true,
        name: name.to_string(),
        timeline: d.timeline.clone(),
        time: d.time,
        playing: d.playing,
        finished: d.finished,
        play_on_awake: scene_d.play_on_awake,
        loop_: scene_d.loop_,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "{name}: timeline={} time={:.3} playing={} finished={}",
            out.timeline, out.time, out.playing, out.finished
        );
    }
    Ok(())
}

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
            pivot_x,
            pivot_y,
            clear_pivot,
            controller,
            timeline,
            timeline_loop,
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
                && pivot_x.is_none()
                && pivot_y.is_none()
                && !clear_pivot
                && controller.is_none()
                && timeline.is_none()
                && timeline_loop.is_none()
            {
                bail!("entity set: pass at least one of --x --y --sx --sy --rotation-deg --tag --follow --lerp --cell --speed --queued-dir --audio-clip --volume --play-on-awake --text --size --color --align --sorting-layer --order-in-layer --pivot-x --pivot-y --clear-pivot --controller --timeline --loop");
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
            if audio_clip.is_some()
                || volume.is_some()
                || (play_on_awake.is_some() && timeline.is_none() && timeline_loop.is_none())
            {
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
            if clear_pivot || pivot_x.is_some() || pivot_y.is_some() {
                if clear_pivot && (pivot_x.is_some() || pivot_y.is_some()) {
                    bail!("entity set: cannot combine --clear-pivot with --pivot-x/--pivot-y");
                }
                set_entity_sprite_pivot(&mut sc, &name, pivot_x, pivot_y, clear_pivot)?;
            }
            if let Some(ctrl) = controller {
                set_entity_controller(&mut sc, &name, &ctrl)?;
            }
            if timeline.is_some() || play_on_awake.is_some() || timeline_loop.is_some() {
                let has_director = sc
                    .find_entity(&name)
                    .and_then(|e| e.components.playable_director.as_ref())
                    .is_some();
                if timeline.is_some() || timeline_loop.is_some() || has_director {
                    set_entity_playable_director(
                        &mut sc,
                        &name,
                        timeline.as_deref(),
                        play_on_awake,
                        timeline_loop,
                    )?;
                }
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
            pivot_x,
            pivot_y,
            controller,
            timeline,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            match kind.to_ascii_lowercase().as_str() {
                "sprite" => {
                    let tex =
                        texture.ok_or_else(|| anyhow::anyhow!("--texture required for Sprite"))?;
                    add_component_sprite(&mut sc, &name, &tex, [width, height])?;
                    if pivot_x.is_some() || pivot_y.is_some() {
                        set_entity_sprite_pivot(&mut sc, &name, pivot_x, pivot_y, false)?;
                    }
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
                    add_component_animation(&mut sc, &name, &clip, fps, r#loop.unwrap_or(true))?;
                }
                "animator" => {
                    let controller = controller
                        .ok_or_else(|| anyhow::anyhow!("--controller required for Animator"))?;
                    add_component_animator(&mut sc, &name, &controller)?;
                }
                "playabledirector" | "playable_director" | "playable-director" | "timeline" => {
                    let timeline = timeline.ok_or_else(|| {
                        anyhow::anyhow!("--timeline required for PlayableDirector")
                    })?;
                    add_component_playable_director(
                        &mut sc,
                        &name,
                        &timeline,
                        play_on_awake.unwrap_or(true),
                        r#loop.unwrap_or(false),
                    )?;
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
                    add_component_audio_source(
                        &mut sc,
                        &name,
                        &clip,
                        volume,
                        play_on_awake.unwrap_or(false),
                    )?;
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
                    bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider|Trigger|Animation|Animator|PlayableDirector|Camera|Follow|GridMover|AudioSource|Text)")
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
                "animator" => remove_component_animator(&mut sc, &name)?,
                "playabledirector" | "playable_director" | "playable-director" | "timeline" => {
                    remove_component_playable_director(&mut sc, &name)?
                }
                "camera" => remove_component_camera(&mut sc, &name)?,
                "follow" => remove_component_follow(&mut sc, &name)?,
                "gridmover" | "grid_mover" | "grid-mover" => {
                    remove_component_grid_mover(&mut sc, &name)?
                }
                "audiosource" | "audio_source" | "audio-source" | "audio" => {
                    remove_component_audio_source(&mut sc, &name)?
                }
                "text" | "label" | "hud" => remove_component_text(&mut sc, &name)?,
                other => bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider|Animation|Animator|PlayableDirector|Camera|Follow|GridMover|AudioSource|Text)"),
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
        EntityCmd::AnimatorSet {
            game,
            name,
            bools,
            floats,
            triggers,
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            if bools.is_empty() && floats.is_empty() && triggers.is_empty() {
                bail!("entity animator-set: pass --bool Name=true, --float Name=1, and/or --trigger Name");
            }
            for spec in bools {
                let (pname, raw) = split_kv(&spec, "--bool")?;
                let val = parse_bool_flag(&raw)?;
                set_entity_animator_bool(&mut sc, &name, &pname, val)?;
            }
            for spec in floats {
                let (pname, raw) = split_kv(&spec, "--float")?;
                let val: f32 = raw.parse().map_err(|_| {
                    anyhow::anyhow!("entity animator-set: --float '{spec}' value is not a number")
                })?;
                set_entity_animator_float(&mut sc, &name, &pname, val)?;
            }
            for spec in triggers {
                set_entity_animator_bool(&mut sc, &name, spec.trim(), true)?;
            }
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("set animator params on {name}"))
        }
        EntityCmd::AnimatorStatus { game, name, scene } => {
            let (gd, project, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let ent = sc
                .find_entity(&name)
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
            let scene_a = ent
                .components
                .animator
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Animator"))?;
            let assets = project.assets_path(&gd);
            let anims = wiimaker_assets::AnimClipCatalog::load_dir(&assets)?;
            let controllers = wiimaker_assets::AnimatorControllerCatalog::load_dir(&assets)?;
            let world = wiimaker_scene::hydrate_with_all_catalogs(
                &sc,
                &wiimaker_scene::TextureMap::new(),
                None,
                Some(&anims),
                Some(&controllers),
            )?;
            let id = world
                .find_by_name(&name)
                .ok_or_else(|| anyhow::anyhow!("entity '{name}' not in world"))?;
            let rt = world.animator(id);
            #[derive(Serialize)]
            struct ParamOut {
                name: String,
                #[serde(rename = "type")]
                kind: String,
                bool_value: bool,
                float_value: f32,
            }
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                name: String,
                controller: String,
                state: String,
                enabled: bool,
                parameters: Vec<ParamOut>,
            }
            let parameters: Vec<ParamOut> = rt
                .map(|a| {
                    a.parameters
                        .iter()
                        .map(|p| ParamOut {
                            name: p.name.clone(),
                            kind: match p.kind {
                                wiimaker_core::AnimatorParamKind::Bool => "Bool".into(),
                                wiimaker_core::AnimatorParamKind::Float => "Float".into(),
                                wiimaker_core::AnimatorParamKind::Trigger => "Trigger".into(),
                            },
                            bool_value: p.bool_value,
                            float_value: p.float_value,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let state = rt.map(|a| a.state.clone()).unwrap_or_default();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Out {
                        ok: true,
                        name: name.clone(),
                        controller: scene_a.controller.clone(),
                        state,
                        enabled: scene_a.enabled,
                        parameters,
                    })?
                );
            } else {
                println!(
                    "{name}: controller={} state={} params={}",
                    scene_a.controller,
                    rt.map(|a| a.state.as_str()).unwrap_or("?"),
                    parameters.len()
                );
            }
            Ok(())
        }
        EntityCmd::TimelinePlay { game, name, scene } => {
            timeline_transport(root, &game, &name, scene.as_deref(), json, true, false)
        }
        EntityCmd::TimelineStop { game, name, scene } => {
            timeline_transport(root, &game, &name, scene.as_deref(), json, false, true)
        }
        EntityCmd::TimelineStatus { game, name, scene } => {
            timeline_transport(root, &game, &name, scene.as_deref(), json, false, false)
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
            let child_count = prefab.children.len();
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
                    children: usize,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        path: dest.display().to_string(),
                        prefab: link,
                        children: child_count,
                    })?
                );
                Ok(())
            } else if child_count == 0 {
                println!("wrote {} (instance {name})", dest.display());
                Ok(())
            } else {
                println!(
                    "wrote {} (instance {name}, {child_count} nested)",
                    dest.display()
                );
                Ok(())
            }
        }
        EntityCmd::CreateVariant {
            game,
            from,
            name,
            as_name,
            relink,
            scene,
        } => {
            let (gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            let (dest, link, overrides_len, base_link) = if let Some(from) = from {
                let (dest, variant) = create_prefab_variant(&gd, &from, &as_name)?;
                let link = normalize_prefab_source(&as_name);
                let base = variant.base.clone().unwrap_or_default();
                (dest, link, variant.overrides.len(), base)
            } else if let Some(name) = name {
                let mut variant = variant_from_instance(&gd, &sc, &name)?;
                refresh_variant_overrides(&gd, &mut variant)?;
                let dest = gd
                    .join("assets")
                    .join("prefabs")
                    .join(format!("{as_name}.prefab.json"));
                save_prefab(&dest, &variant)?;
                let link = normalize_prefab_source(&as_name);
                let base = variant.base.clone().unwrap_or_default();
                let ov = variant.overrides.len();
                if relink {
                    attach_prefab_instance(&mut sc, &name, &link)?;
                    save_scene(&path, &sc)?;
                }
                (dest, link, ov, base)
            } else {
                bail!("create-variant requires --from <base> or --name <instance>");
            };
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    path: String,
                    prefab: String,
                    base: String,
                    overrides: usize,
                    variant: bool,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        path: dest.display().to_string(),
                        prefab: link,
                        base: base_link,
                        overrides: overrides_len,
                        variant: true,
                    })?
                );
                Ok(())
            } else {
                println!(
                    "wrote variant {} (base {base_link}, {overrides_len} overrides)",
                    dest.display()
                );
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
            let raw = load_prefab(&prefab_path)?;
            let pf = resolve_prefab(&gd, &raw)?;
            let link = normalize_prefab_source(&prefab);
            let child_count = pf.children.len();
            let new_name = instantiate_prefab(&mut sc, &pf, &link, x, y);
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                    prefab: String,
                    children: usize,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: new_name,
                        prefab: link,
                        children: child_count,
                    })?
                );
                Ok(())
            } else if child_count == 0 {
                println!("instantiated → {new_name} ({link})");
                Ok(())
            } else {
                println!("instantiated → {new_name} ({link}, {child_count} nested)");
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
            refresh_variant_overrides(&gd, &mut pf)?;
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
            let (instance, overrides, base, is_variant) = if source.is_some() {
                let raw = source
                    .as_deref()
                    .and_then(|s| resolve_prefab_asset(&gd, s).ok())
                    .and_then(|p| load_prefab(&p).ok());
                let base = raw.as_ref().and_then(|p| p.base.clone());
                let is_variant = raw.as_ref().is_some_and(|p| p.is_variant());
                match load_prefab_for_instance(&gd, ent) {
                    Ok(pf) => (
                        true,
                        prefab_tree_overrides(&sc, &name, &pf).fields,
                        base,
                        is_variant,
                    ),
                    Err(_) => (true, Vec::new(), base, is_variant),
                }
            } else {
                (false, Vec::new(), None, false)
            };
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                    instance: bool,
                    prefab: Option<String>,
                    base: Option<String>,
                    variant: bool,
                    overrides: Vec<String>,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name,
                        instance,
                        prefab: source,
                        base,
                        variant: is_variant,
                        overrides,
                    })?
                );
                Ok(())
            } else if instance {
                let kind = if is_variant { "variant" } else { "instance" };
                let base_note = base
                    .as_deref()
                    .map(|b| format!(" (base {b})"))
                    .unwrap_or_default();
                if overrides.is_empty() {
                    println!("{name} {kind} of {}{base_note}", source.unwrap_or_default());
                } else {
                    println!(
                        "{name} {kind} of {}{base_note} · {} overrides: {}",
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

fn split_kv(spec: &str, flag: &str) -> Result<(String, String)> {
    let spec = spec.trim();
    let Some((k, v)) = spec.split_once('=') else {
        bail!("{flag} expects Name=value, got '{spec}'");
    };
    let k = k.trim();
    if k.is_empty() {
        bail!("{flag} parameter name is empty");
    }
    Ok((k.to_string(), v.trim().to_string()))
}

fn parse_bool_flag(raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => bail!("expected true/false, got '{other}'"),
    }
}
