use std::path::Path;

use anyhow::{bail, Result};
use serde::Serialize;
use wiimaker_scene::{
    add_component_collider, add_component_disc, add_component_sprite, add_component_tilemap,
    add_entity, apply_prefab, duplicate_entity, entities_overlap, entity_overlaps,
    entity_to_prefab, instantiate_prefab, load_prefab, remove_component_collider,
    remove_component_disc, remove_component_sprite, remove_component_tilemap, remove_entity,
    rename_entity, save_prefab, save_scene, set_component_enabled, set_entity_parent,
    set_entity_rotation_z, set_entity_scale, set_entity_transform, unpack_prefab_instance,
    MutateOpts, Scene, SceneColliderKind,
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
            scene,
        } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            if x.is_none() && y.is_none() && sx.is_none() && sy.is_none() && rotation_deg.is_none()
            {
                bail!("entity set: pass at least one of --x --y --sx --sy --rotation-deg");
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
                "collider" => {
                    let shape = match shape.to_ascii_lowercase().as_str() {
                        "circle" => SceneColliderKind::Circle,
                        "aabb" | "box" => SceneColliderKind::Aabb,
                        other => bail!("unknown collider --shape '{other}' (Aabb|Circle)"),
                    };
                    add_component_collider(&mut sc, &name, shape, [width, height], radius, solid)?;
                }
                other => bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider)"),
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
                other => bail!("unknown component kind '{other}' (Sprite|Disc|Tilemap|Collider)"),
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
        EntityCmd::CreatePrefab {
            game,
            name,
            as_name,
            scene,
        } => {
            let (gd, _p, _path, sc) = open_scene(root, &game, scene.as_deref())?;
            let prefab = entity_to_prefab(&sc, &name)?;
            let stem = as_name.unwrap_or_else(|| name.clone());
            let dest = gd
                .join("assets")
                .join("prefabs")
                .join(format!("{stem}.prefab.json"));
            save_prefab(&dest, &prefab)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    path: String,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        path: dest.display().to_string(),
                    })?
                );
                Ok(())
            } else {
                println!("wrote {}", dest.display());
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
            let new_name = instantiate_prefab(&mut sc, &pf, x, y);
            save_scene(&path, &sc)?;
            if json {
                #[derive(Serialize)]
                struct Out {
                    ok: bool,
                    name: String,
                }
                println!(
                    "{}",
                    serde_json::to_string(&Out {
                        ok: true,
                        name: new_name,
                    })?
                );
                Ok(())
            } else {
                println!("instantiated → {new_name}");
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
            let prefab_path = resolve_prefab_path(&gd, &prefab)?;
            let pf = load_prefab(&prefab_path)?;
            apply_prefab(&mut sc, &name, &pf)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("applied prefab to {name}"))
        }
        EntityCmd::UnpackPrefab { game, name, scene } => {
            let (_gd, _p, path, mut sc) = open_scene(root, &game, scene.as_deref())?;
            unpack_prefab_instance(&mut sc, &name)?;
            save_scene(&path, &sc)?;
            emit_ok(json, &format!("unpacked {name}"))
        }
    }
}

fn resolve_prefab_path(game_dir: &Path, prefab: &str) -> Result<std::path::PathBuf> {
    let direct = game_dir.join(prefab);
    if direct.is_file() {
        return Ok(direct);
    }
    let with_ext = if prefab.ends_with(".prefab.json") {
        game_dir.join("assets").join("prefabs").join(prefab)
    } else {
        game_dir
            .join("assets")
            .join("prefabs")
            .join(format!("{prefab}.prefab.json"))
    };
    if with_ext.is_file() {
        return Ok(with_ext);
    }
    bail!(
        "prefab not found: {prefab} (tried {}, {})",
        direct.display(),
        with_ext.display()
    )
}
