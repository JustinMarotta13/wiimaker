use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    add_component_animation, add_component_animator, add_component_audio_source,
    add_component_camera, add_component_collider, add_component_disc, add_component_follow,
    add_component_grid_mover, add_component_playable_director, add_component_sprite,
    add_component_text, add_component_tilemap, add_entity, apply_instance_fields_to_prefab,
    apply_instance_to_base, apply_prefab, apply_prefab_scene_inherit,
    apply_variant_override_to_base, apply_variant_override_to_base_in_scene,
    attach_prefab_instance, create_prefab_variant, duplicate_entity, entities_overlap,
    entity_overlaps, entity_to_prefab, entity_triggers_entered, find_game_dir, instantiate_prefab,
    load_prefab, load_prefab_for_instance, normalize_prefab_source, plan_prefab_scene_inherit,
    prefab_chain_status, prefab_tree_overrides, refresh_variant_overrides,
    remove_component_animation, remove_component_animator, remove_component_audio_source,
    remove_component_camera, remove_component_collider, remove_component_disc,
    remove_component_follow, remove_component_grid_mover, remove_component_playable_director,
    remove_component_sprite, remove_component_text, remove_component_tilemap, remove_entity,
    rename_entity, resolve_prefab, resolve_prefab_asset, revert_prefab_instance, save_prefab,
    save_scene, set_component_enabled, set_entity_anim, set_entity_animator_bool,
    set_entity_animator_float, set_entity_animator_layer_weight, set_entity_audio_source,
    set_entity_controller, set_entity_follow, set_entity_grid_mover, set_entity_parent,
    set_entity_playable_director, set_entity_rotation_z, set_entity_scale, set_entity_sorting,
    set_entity_sprite_pivot, set_entity_text, set_entity_transform, unpack_prefab_instance,
    variant_from_instance, ApplyBaseTarget, MutateOpts, Scene, SceneColliderKind, SceneDir,
    SceneTextAlign,
};

use crate::args::EntityCmd;
use crate::cmds::scene::open_scene;
use crate::util::emit_ok;

struct SignalQuery {
    dt: f32,
    steps: u32,
    play: bool,
}

fn timeline_transport(
    root: &Path,
    game: &str,
    name: &str,
    scene: Option<&str>,
    json: bool,
    play: bool,
    stop: bool,
    signals: Option<SignalQuery>,
    advance: Option<(f32, u32)>,
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
    let mut fired = Vec::new();
    if let Some(q) = signals {
        if q.play {
            world.play_timeline(id);
        }
        let steps = q.steps.max(1);
        for _ in 0..steps {
            world.tick_timelines(q.dt);
            fired.extend(world.take_timeline_signals());
        }
    } else if play {
        world.play_timeline(id);
        fired.extend(world.take_timeline_signals());
    } else if stop {
        world.stop_timeline(id);
        fired.extend(world.take_timeline_signals());
    } else {
        fired.extend(world.take_timeline_signals());
    }
    if let Some((dt, steps)) = advance {
        if steps > 0 {
            let playing = world.director(id).is_some_and(|d| d.playing);
            if !playing {
                world.play_timeline(id);
            }
            for _ in 0..steps {
                world.tick_timelines(dt);
                fired.extend(world.take_timeline_signals());
            }
        }
    }
    let d = world
        .director(id)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no runtime PlayableDirector"))?;
    #[derive(Serialize)]
    struct SignalOut {
        director: String,
        timeline: String,
        signal: String,
        payload: String,
        binding: String,
        time: f32,
    }
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
        signals: Vec<SignalOut>,
        pose: Vec<PoseOut>,
    }
    #[derive(Serialize)]
    struct PoseOut {
        name: String,
        x: f32,
        y: f32,
    }
    let signals_out: Vec<SignalOut> = fired
        .into_iter()
        .map(|s| SignalOut {
            director: s.director,
            timeline: s.timeline,
            signal: s.signal,
            payload: s.payload,
            binding: s.binding,
            time: s.time,
        })
        .collect();
    let signal_n = signals_out.len();
    let pose_names: Vec<String> = {
        let mut names = Vec::new();
        for track in &d.tracks {
            if track.kind == wiimaker_core::TimelineTrackKind::Transform
                && !track.binding.is_empty()
                && !names.iter().any(|n| n == &track.binding)
            {
                names.push(track.binding.clone());
            }
        }
        names
    };
    let timeline_name = d.timeline.clone();
    let time = d.time;
    let playing = d.playing;
    let finished = d.finished;
    let pose = pose_names
        .into_iter()
        .filter_map(|binding| {
            let eid = world.find_by_name(&binding)?;
            let xf = world.transform(eid)?;
            Some(PoseOut {
                name: binding,
                x: xf.translation.x,
                y: xf.translation.y,
            })
        })
        .collect();
    let out = Out {
        ok: true,
        name: name.to_string(),
        timeline: timeline_name,
        time,
        playing,
        finished,
        play_on_awake: scene_d.play_on_awake,
        loop_: scene_d.loop_,
        signals: signals_out,
        pose,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "{name}: timeline={} time={:.3} playing={} finished={} signals={signal_n}",
            out.timeline, out.time, out.playing, out.finished
        );
        for p in &out.pose {
            println!("  {} x={:.3} y={:.3}", p.name, p.x, p.y);
        }
        for sig in &out.signals {
            if sig.payload.is_empty() {
                println!(
                    "  signal {} @ {:.3} on {}",
                    sig.signal, sig.time, sig.binding
                );
            } else {
                println!(
                    "  signal {} @ {:.3} on {} · {}",
                    sig.signal, sig.time, sig.binding, sig.payload
                );
            }
        }
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
            let has_director = sc
                .find_entity(&name)
                .and_then(|e| e.components.playable_director.as_ref())
                .is_some();
            let has_audio = sc
                .find_entity(&name)
                .and_then(|e| e.components.audio_source.as_ref())
                .is_some();
            // `--play-on-awake` is shared with PlayableDirector. Only touch
            // AudioSource when the entity already has one, or when there is no
            // director (legacy AudioSource-only `entity set`).
            let audio_play_on_awake = play_on_awake.filter(|_| {
                (has_audio || !has_director) && timeline.is_none() && timeline_loop.is_none()
            });
            if audio_clip.is_some() || volume.is_some() || audio_play_on_awake.is_some() {
                set_entity_audio_source(
                    &mut sc,
                    &name,
                    audio_clip.as_deref(),
                    volume,
                    audio_play_on_awake,
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
            layer,
            weight,
            scene,
        } => {
            let (gd, project, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            if bools.is_empty() && floats.is_empty() && triggers.is_empty() && weight.is_none() {
                bail!("entity animator-set: pass --bool Name=true, --float Name=1, --trigger Name, and/or --layer Name --weight 0.5");
            }
            if weight.is_some() != layer.is_some() {
                bail!("entity animator-set: --layer and --weight are set together");
            }
            if let (Some(layer), Some(weight)) = (layer.as_deref(), weight) {
                set_entity_animator_layer_weight(&mut sc, &name, layer, weight)?;
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
            if json {
                #[derive(Serialize)]
                struct SetOut {
                    ok: bool,
                    name: String,
                    blend: Option<BlendOut>,
                }
                let world = hydrate_animator_world(&gd, &project, &sc)?;
                let blend = world
                    .find_by_name(&name)
                    .and_then(|id| world.animator(id))
                    .and_then(|a| a.current_blend())
                    .map(blend_out);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&SetOut {
                        ok: true,
                        name: name.clone(),
                        blend,
                    })?
                );
                Ok(())
            } else {
                emit_ok(json, &format!("set animator params on {name}"))
            }
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
            let world = hydrate_animator_world(&gd, &project, &sc)?;
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
            struct PlaybackOut {
                layer: String,
                state: String,
                clip: String,
            }
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                name: String,
                controller: String,
                state: String,
                enabled: bool,
                parameters: Vec<ParamOut>,
                blend: Option<BlendOut>,
                layers: Vec<LayerStatus>,
                playback: PlaybackOut,
            }
            let blend = rt.and_then(|a| a.current_blend()).map(blend_out);
            let layers = rt.map(layer_rows).unwrap_or_default();
            let playback = rt
                .map(|a| {
                    let index = a.winning_layer();
                    let clip = world
                        .animation(id)
                        .map(|anim| anim.clip.clone())
                        .unwrap_or_default();
                    PlaybackOut {
                        layer: a
                            .layer_name(index)
                            .unwrap_or(wiimaker_core::BASE_LAYER_NAME)
                            .to_string(),
                        state: a.layer_state_name(index).unwrap_or("").to_string(),
                        clip,
                    }
                })
                .unwrap_or(PlaybackOut {
                    layer: String::new(),
                    state: String::new(),
                    clip: String::new(),
                });
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
                        blend,
                        layers,
                        playback,
                    })?
                );
            } else {
                let layer_txt = rt
                    .map(|a| {
                        (0..a.layer_count())
                            .filter_map(|i| {
                                Some(format!(
                                    "{}:{}@{:.2}",
                                    a.layer_name(i)?,
                                    a.layer_state_name(i).unwrap_or("?"),
                                    a.layer_weight(i).unwrap_or(0.0)
                                ))
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                println!(
                    "{name}: controller={} state={} params={} layers=[{}]{}",
                    scene_a.controller,
                    rt.map(|a| a.state.as_str()).unwrap_or("?"),
                    parameters.len(),
                    layer_txt,
                    blend
                        .as_ref()
                        .map(|b| format!(" blend={} active={}", b.kind, b.active))
                        .unwrap_or_default()
                );
            }
            Ok(())
        }
        EntityCmd::TimelinePlay { game, name, scene } => timeline_transport(
            root,
            &game,
            &name,
            scene.as_deref(),
            json,
            true,
            false,
            None,
            None,
        ),
        EntityCmd::TimelineStop { game, name, scene } => timeline_transport(
            root,
            &game,
            &name,
            scene.as_deref(),
            json,
            false,
            true,
            None,
            None,
        ),
        EntityCmd::TimelineStatus {
            game,
            name,
            scene,
            steps,
            dt,
        } => timeline_transport(
            root,
            &game,
            &name,
            scene.as_deref(),
            json,
            false,
            false,
            None,
            Some((dt, steps)),
        ),
        EntityCmd::TimelineSignals {
            game,
            name,
            dt,
            steps,
            play,
            scene,
        } => timeline_transport(
            root,
            &game,
            &name,
            scene.as_deref(),
            json,
            false,
            false,
            Some(SignalQuery { dt, steps, play }),
            None,
        ),
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
            to_base,
            to_root,
            fields,
            scene,
        } => apply_prefab_cmd(
            root,
            &game,
            name.as_deref(),
            prefab.as_deref(),
            to_base,
            to_root,
            &fields,
            scene.as_deref(),
            json,
        ),
        EntityCmd::OpenBase {
            game,
            name,
            prefab,
            root: to_root,
            scene,
        } => open_base_cmd(
            root,
            &game,
            name.as_deref(),
            prefab.as_deref(),
            to_root,
            scene.as_deref(),
            json,
        ),
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
            let (instance, overrides, chain_status) = if let Some(src) = source.as_deref() {
                let status = prefab_chain_status(&gd, src).ok();
                let overrides = match load_prefab_for_instance(&gd, ent) {
                    Ok(pf) => prefab_tree_overrides(&sc, &name, &pf).fields,
                    Err(_) => Vec::new(),
                };
                (true, overrides, status)
            } else {
                (false, Vec::new(), None)
            };
            let base = chain_status.as_ref().and_then(|s| s.base.clone());
            let is_variant = chain_status.as_ref().is_some_and(|s| s.variant);
            let chain = chain_status
                .as_ref()
                .map(|s| s.chain.clone())
                .unwrap_or_default();
            let root_base = chain_status.as_ref().map(|s| s.root.clone());
            let variant_overrides = chain_status
                .as_ref()
                .map(|s| s.variant_overrides.clone())
                .unwrap_or_default();
            let base_overrides = chain_status
                .as_ref()
                .map(|s| s.base_overrides.clone())
                .unwrap_or_default();
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                    instance: bool,
                    prefab: Option<String>,
                    base: Option<String>,
                    root: Option<String>,
                    variant: bool,
                    chain: Vec<String>,
                    /// Instance diffs versus the resolved prefab.
                    overrides: Vec<String>,
                    /// Overrides stored on the variant asset.
                    variant_overrides: Vec<String>,
                    /// Overrides stored on the immediate base when that base is itself a variant.
                    base_overrides: Vec<String>,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name,
                        instance,
                        prefab: source,
                        base,
                        root: root_base,
                        variant: is_variant,
                        chain,
                        overrides,
                        variant_overrides,
                        base_overrides,
                    })?
                );
                Ok(())
            } else if instance {
                let kind = if is_variant { "variant" } else { "instance" };
                let base_note = base
                    .as_deref()
                    .map(|b| format!(" (base {b})"))
                    .unwrap_or_default();
                println!(
                    "{name} {kind} of {}{base_note}",
                    source.clone().unwrap_or_default()
                );
                if !chain.is_empty() {
                    println!("chain: {}", chain.join(" → "));
                }
                println!(
                    "variant overrides: {}",
                    if variant_overrides.is_empty() {
                        "(none)".to_string()
                    } else {
                        variant_overrides.join(", ")
                    }
                );
                println!(
                    "base overrides: {}",
                    if base_overrides.is_empty() {
                        "(none)".to_string()
                    } else {
                        base_overrides.join(", ")
                    }
                );
                if overrides.is_empty() {
                    println!("instance overrides: (none)");
                } else {
                    println!(
                        "instance overrides: {} ({})",
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

#[allow(clippy::too_many_arguments)]
fn apply_prefab_cmd(
    root: &Path,
    game: &str,
    name: Option<&str>,
    prefab: Option<&str>,
    to_base: bool,
    to_root: bool,
    fields: &[String],
    scene: Option<&str>,
    json: bool,
) -> Result<()> {
    if to_root && !to_base {
        bail!("--to-root requires --to-base");
    }
    let target = if to_root {
        ApplyBaseTarget::Root
    } else {
        ApplyBaseTarget::Immediate
    };
    if to_base {
        if name.is_some() && prefab.is_some() {
            bail!("apply-prefab --to-base accepts either --name or --prefab, not both");
        }
        let report = if let Some(name) = name {
            let (gd, _p, path, mut sc) = open_scene(root, game, scene)?;
            let report = apply_instance_to_base(&gd, &mut sc, name, fields, target)?;
            if !report.scene_synced.is_empty() {
                save_scene(&path, &sc)?;
            }
            report
        } else if let Some(prefab) = prefab {
            match open_scene(root, game, scene) {
                Ok((gd, _p, path, mut sc)) => {
                    let report = apply_variant_override_to_base_in_scene(
                        &gd, &mut sc, prefab, fields, target,
                    )?;
                    if !report.scene_synced.is_empty() {
                        save_scene(&path, &sc)?;
                    }
                    report
                }
                Err(e) if scene.is_none() => {
                    let gd = find_game_dir(root, game)?;
                    let _ = e;
                    apply_variant_override_to_base(&gd, prefab, fields, target)?
                }
                Err(e) => return Err(e),
            }
        } else {
            bail!("apply-prefab --to-base requires --name <entity> or --prefab <variant>");
        };
        return emit_apply_report(json, &report);
    }
    let Some(name) = name else {
        bail!("apply-prefab requires --name <entity> (or --to-base --prefab <variant>)");
    };
    let (gd, _p, path, mut sc) = open_scene(root, game, scene)?;
    let source = instance_prefab_source(&sc, name, prefab)?;
    let prefab_path = resolve_prefab_path(&gd, &source)?;
    if fields.is_empty() {
        let mut pf = load_prefab(&prefab_path)?;
        let resolved = resolve_prefab(&gd, &pf)?;
        let changed = prefab_tree_overrides(&sc, name, &resolved).fields;
        let slots = plan_prefab_scene_inherit(&gd, &sc, &source, &changed)?;
        apply_prefab(&mut sc, name, &mut pf, &source)?;
        refresh_variant_overrides(&gd, &mut pf)?;
        save_prefab(&prefab_path, &pf)?;
        let scene_synced = apply_prefab_scene_inherit(&gd, &mut sc, &slots)?;
        save_scene(&path, &sc)?;
        if json {
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                message: String,
                prefab: String,
                path: String,
                scene_synced: Vec<String>,
            }
            println!(
                "{}",
                serde_json::to_string(&Out {
                    ok: true,
                    message: format!("applied prefab to {name}"),
                    prefab: normalize_prefab_source(&source),
                    path: prefab_path.display().to_string(),
                    scene_synced,
                })?
            );
            Ok(())
        } else {
            println!("applied {name} → {}", prefab_path.display());
            Ok(())
        }
    } else {
        let report = apply_instance_fields_to_prefab(&gd, &mut sc, name, fields)?;
        if !report.scene_synced.is_empty() {
            save_scene(&path, &sc)?;
        }
        if json {
            #[derive(Serialize)]
            struct Out {
                ok: bool,
                message: String,
                prefab: String,
                path: String,
                applied: Vec<String>,
                variant_overrides: Vec<String>,
                scene_synced: Vec<String>,
            }
            println!(
                "{}",
                serde_json::to_string(&Out {
                    ok: true,
                    message: format!("applied {} field(s) to {name}", fields.len()),
                    prefab: normalize_prefab_source(&source),
                    path: prefab_path.display().to_string(),
                    applied: fields.to_vec(),
                    variant_overrides: report.variant_overrides,
                    scene_synced: report.scene_synced,
                })?
            );
            Ok(())
        } else {
            println!(
                "applied {} → {} ({})",
                fields.join(", "),
                prefab_path.display(),
                name
            );
            Ok(())
        }
    }
}

fn emit_apply_report(json: bool, report: &wiimaker_scene::ApplyToBaseReport) -> Result<()> {
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            message: String,
            variant: &'a str,
            base: &'a str,
            applied: &'a [String],
            variant_overrides: &'a [String],
            chain: &'a [String],
            scene_synced: &'a [String],
        }
        let message = if report.applied.is_empty() {
            "no overrides to apply to base".to_string()
        } else {
            format!(
                "applied {} to base {}",
                report.applied.join(", "),
                report.base
            )
        };
        println!(
            "{}",
            serde_json::to_string(&Out {
                ok: true,
                message,
                variant: &report.variant,
                base: &report.base,
                applied: &report.applied,
                variant_overrides: &report.variant_overrides,
                chain: &report.chain,
                scene_synced: &report.scene_synced,
            })?
        );
    } else if report.applied.is_empty() {
        println!("no overrides to apply to base {}", report.base);
    } else {
        println!(
            "applied {} → {} (variant {})",
            report.applied.join(", "),
            report.base,
            report.variant
        );
    }
    Ok(())
}

fn open_base_cmd(
    root: &Path,
    game: &str,
    name: Option<&str>,
    prefab: Option<&str>,
    to_root: bool,
    scene: Option<&str>,
    json: bool,
) -> Result<()> {
    if name.is_some() && prefab.is_some() {
        bail!("open-base accepts either --name or --prefab, not both");
    }
    let (gd, source) = if let Some(prefab) = prefab {
        let gd = find_game_dir(root, game)?;
        (gd, normalize_prefab_source(prefab))
    } else if let Some(name) = name {
        let (gd, _p, _path, sc) = open_scene(root, game, scene)?;
        let link = sc
            .find_entity(name)
            .and_then(|e| e.prefab.clone())
            .ok_or_else(|| anyhow::anyhow!("entity '{name}' is not a prefab instance"))?;
        (gd, normalize_prefab_source(&link))
    } else {
        bail!("open-base requires --name <entity> or --prefab <asset>");
    };
    let status = prefab_chain_status(&gd, &source)?;
    if !status.variant {
        bail!(
            "prefab '{}' is not a variant (no base to open)",
            status.prefab
        );
    }
    let target = if to_root {
        status.root.clone()
    } else {
        status
            .base
            .clone()
            .ok_or_else(|| anyhow::anyhow!("variant '{}' has no base", status.prefab))?
    };
    if json {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            prefab: String,
            base: Option<String>,
            root: String,
            target: String,
            chain: Vec<String>,
            variant_overrides: Vec<String>,
            base_overrides: Vec<String>,
        }
        println!(
            "{}",
            serde_json::to_string(&Out {
                ok: true,
                prefab: status.prefab,
                base: status.base,
                root: status.root,
                target,
                chain: status.chain,
                variant_overrides: status.variant_overrides,
                base_overrides: status.base_overrides,
            })?
        );
    } else {
        println!("base {target}");
        println!("chain: {}", status.chain.join(" → "));
    }
    Ok(())
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

#[derive(Serialize)]
struct BlendMotionOut {
    clip: String,
    threshold: Option<f32>,
    position: Option<[f32; 2]>,
    weight: f32,
}

#[derive(Serialize)]
struct BlendOut {
    #[serde(rename = "type")]
    kind: String,
    params: Vec<String>,
    active: String,
    motions: Vec<BlendMotionOut>,
}

fn layer_rows(a: &wiimaker_core::Animator) -> Vec<LayerStatus> {
    (0..a.layer_count())
        .filter_map(|i| {
            Some(LayerStatus {
                name: a.layer_name(i)?.to_string(),
                weight: a.layer_weight(i).unwrap_or(0.0),
                state: a.layer_state_name(i).unwrap_or("").to_string(),
                blend: a.layer_blend(i).map(blend_out),
            })
        })
        .collect()
}

#[derive(Serialize)]
struct LayerStatus {
    name: String,
    weight: f32,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blend: Option<BlendOut>,
}

fn blend_out(tree: &wiimaker_core::BlendTree) -> BlendOut {
    let one_d = tree.dimension == wiimaker_core::BlendDimension::OneD;
    BlendOut {
        kind: if one_d { "1D" } else { "2D" }.into(),
        params: tree.params.clone(),
        active: tree
            .active_motion()
            .map(|m| m.clip.clone())
            .unwrap_or_default(),
        motions: tree
            .motions
            .iter()
            .enumerate()
            .map(|(i, m)| BlendMotionOut {
                clip: m.clip.clone(),
                threshold: one_d.then_some(m.threshold),
                position: (!one_d).then_some(m.position),
                weight: tree.weights.get(i).copied().unwrap_or(0.0),
            })
            .collect(),
    }
}

fn hydrate_animator_world(
    game_dir: &Path,
    project: &wiimaker_scene::GameProject,
    sc: &Scene,
) -> Result<wiimaker_core::World> {
    let assets = project.assets_path(game_dir);
    let anims = wiimaker_assets::AnimClipCatalog::load_dir(&assets)?;
    let controllers = wiimaker_assets::AnimatorControllerCatalog::load_dir(&assets)?;
    wiimaker_scene::hydrate_with_all_catalogs(
        sc,
        &wiimaker_scene::TextureMap::new(),
        None,
        Some(&anims),
        Some(&controllers),
    )
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
