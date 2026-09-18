use std::path::Path;

use anyhow::Result;
use wiimaker_play::{build_play_plugin, inspect_play_plugin};
use wiimaker_scene::{
    find_game_dir, load_editor_prefs, load_project, set_project_view, set_scene_view,
    EDITOR_PREFS_REL,
};

use crate::args::EditorCmd;
use crate::util::emit_ok;

pub fn editor_cmd(root: &Path, cmd: EditorCmd, json: bool) -> Result<()> {
    match cmd {
        EditorCmd::PlayStatus { game, build } => {
            let game_dir = find_game_dir(root, &game)?;
            let project = load_project(&game_dir)?;
            let mut built = None;
            if build {
                built = Some(build_play_plugin(root, &project.name)?);
            }
            let st = inspect_play_plugin(root, &project.name, &game_dir);
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "package": st.package,
                        "backend": st.backend,
                        "cdylib": st.cdylib,
                        "dylib": st.dylib.to_string_lossy(),
                        "dylib_exists": st.dylib_exists,
                        "abi": st.abi,
                        "force_fallback": st.force_fallback,
                        "hint": st.hint,
                        "built": built,
                        "note": "CLI `run` is the external host twin. No new prefs — Play uses the game App plugin when present.",
                    })
                );
            } else {
                println!("play-status {}", st.package);
                println!("  backend: {}", st.backend);
                println!("  cdylib: {}", st.cdylib);
                println!(
                    "  dylib: {} (exists={})",
                    st.dylib.display(),
                    st.dylib_exists
                );
                if let Some(abi) = st.abi {
                    println!("  abi: {abi}");
                }
                if let Some(h) = &st.hint {
                    println!("  hint: {h}");
                }
                if let Some(b) = &built {
                    println!("  {b}");
                }
            }
            Ok(())
        }
        EditorCmd::Prefs { game } => {
            let game_dir = find_game_dir(root, &game)?;
            let prefs = load_editor_prefs(&game_dir)?;
            let path = game_dir.join(EDITOR_PREFS_REL);
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "path": path.to_string_lossy(),
                        "exists": path.is_file(),
                        "prefs": prefs,
                    })
                );
            } else {
                let kind = if path.is_file() { "file" } else { "defaults" };
                println!("{} ({kind})", path.display());
                println!(
                    "scene: zoom={:.2} pan=({:.1},{:.1}) grid={} gizmos={} snap={} snap_size={:.0} 2d={}",
                    prefs.scene_view.zoom,
                    prefs.scene_view.pan_x,
                    prefs.scene_view.pan_y,
                    prefs.scene_view.grid_overlay,
                    prefs.scene_view.gizmos,
                    prefs.scene_view.snap,
                    prefs.scene_view.snap_size,
                    prefs.scene_view.mode_2d,
                );
                println!(
                    "game: preset={} aspect={:?} {}x{} scale={:.2}",
                    prefs.game_view.preset.as_str(),
                    prefs.game_view.aspect,
                    prefs.game_view.width,
                    prefs.game_view.height,
                    prefs.game_view.scale,
                );
                if prefs.project_view.collapsed.is_empty() {
                    println!("project: collapsed=(all expanded)");
                } else {
                    println!(
                        "project: collapsed={}",
                        prefs.project_view.collapsed.join(",")
                    );
                }
            }
            Ok(())
        }
        EditorCmd::SetSceneView {
            game,
            zoom,
            pan_x,
            pan_y,
            grid,
            gizmos,
            snap,
            snap_size,
            mode_2d,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            if zoom.is_none()
                && pan_x.is_none()
                && pan_y.is_none()
                && grid.is_none()
                && gizmos.is_none()
                && snap.is_none()
                && snap_size.is_none()
                && mode_2d.is_none()
            {
                anyhow::bail!("set-scene-view needs at least one flag (--zoom, --grid, …)");
            }
            let prefs = set_scene_view(
                &game_dir, zoom, pan_x, pan_y, grid, gizmos, snap, snap_size, mode_2d,
            )?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": "scene view prefs updated",
                        "path": game_dir.join(EDITOR_PREFS_REL).to_string_lossy(),
                        "prefs": prefs,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, "scene view prefs updated")
            }
        }
        EditorCmd::SetProjectView {
            game,
            collapse,
            expand,
            clear_collapsed,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let mutate = clear_collapsed || !collapse.is_empty() || !expand.is_empty();
            let prefs = if mutate {
                set_project_view(&game_dir, &collapse, &expand, clear_collapsed)?
            } else {
                load_editor_prefs(&game_dir)?
            };
            let collapsed = &prefs.project_view.collapsed;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": if mutate {
                            "project view prefs updated"
                        } else {
                            "project view prefs"
                        },
                        "path": game_dir.join(EDITOR_PREFS_REL).to_string_lossy(),
                        "collapsed": collapsed,
                        "prefs": prefs,
                    })
                );
                Ok(())
            } else if mutate {
                emit_ok(json, "project view prefs updated")?;
                if collapsed.is_empty() {
                    println!("collapsed=(all expanded)");
                } else {
                    println!("collapsed={}", collapsed.join(","));
                }
                Ok(())
            } else if collapsed.is_empty() {
                println!("project: collapsed=(all expanded)");
                Ok(())
            } else {
                println!("project: collapsed={}", collapsed.join(","));
                Ok(())
            }
        }
    }
}
