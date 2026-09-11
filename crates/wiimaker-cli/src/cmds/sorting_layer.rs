use std::path::Path;

use anyhow::Result;
use wiimaker_scene::{
    add_sorting_layer, find_game_dir, list_sorting_layers, load_project, move_sorting_layer,
    remove_sorting_layer, rename_sorting_layer,
};

use crate::args::SortingLayerCmd;
use crate::util::emit_ok;

pub fn sorting_layer_cmd(root: &Path, cmd: SortingLayerCmd, json: bool) -> Result<()> {
    match cmd {
        SortingLayerCmd::List { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let layers = list_sorting_layers(&game_dir)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "sorting_layers": layers,
                        "from_file": !project.sorting_layers.is_empty(),
                    })
                );
            } else {
                for (i, name) in layers.iter().enumerate() {
                    println!("{i}  {name}");
                }
            }
            Ok(())
        }
        SortingLayerCmd::Add { game, name, index } => {
            let game_dir = find_game_dir(root, &game)?;
            let layers = add_sorting_layer(&game_dir, &name, index)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("sorting layers += {name}"),
                        "sorting_layers": layers,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("sorting layers += {name}"))
            }
        }
        SortingLayerCmd::Rename { game, from, to } => {
            let game_dir = find_game_dir(root, &game)?;
            let layers = rename_sorting_layer(&game_dir, &from, &to, None)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("sorting layer '{from}' → '{to}'"),
                        "sorting_layers": layers,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("sorting layer '{from}' → '{to}'"))
            }
        }
        SortingLayerCmd::Move { game, name, index } => {
            let game_dir = find_game_dir(root, &game)?;
            let layers = move_sorting_layer(&game_dir, &name, index)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("sorting layer '{name}' → index {index}"),
                        "sorting_layers": layers,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("sorting layer '{name}' → index {index}"))
            }
        }
        SortingLayerCmd::Remove { game, name } => {
            let game_dir = find_game_dir(root, &game)?;
            let layers = remove_sorting_layer(&game_dir, &name, None)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": format!("sorting layers -= {name}"),
                        "sorting_layers": layers,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, &format!("sorting layers -= {name}"))
            }
        }
    }
}
