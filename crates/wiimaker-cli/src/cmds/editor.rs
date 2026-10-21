use std::path::Path;

use anyhow::Result;
use wiimaker_core::input::Input;
use wiimaker_play::{build_play_plugin, headless_play_steps, inspect_play_plugin, PlayStart};
use wiimaker_scene::{
    find_game_dir, load_editor_prefs, load_project, set_console, set_project_view, set_scene_view,
    ConsolePatch, EDITOR_PREFS_REL,
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
        EditorCmd::PlayStep { game, frames } => play_step(root, &game, frames, json),
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
                    "scene: zoom={:.2} pan=({:.1},{:.1}) grid={} gizmos={} snap={} snap_size={:.0} 2d={} tool={} handles={}/{}",
                    prefs.scene_view.zoom,
                    prefs.scene_view.pan_x,
                    prefs.scene_view.pan_y,
                    prefs.scene_view.grid_overlay,
                    prefs.scene_view.gizmos,
                    prefs.scene_view.snap,
                    prefs.scene_view.snap_size,
                    prefs.scene_view.mode_2d,
                    prefs.scene_view.tool.as_str(),
                    prefs.scene_view.handle_position.as_str(),
                    prefs.scene_view.handle_rotation.as_str(),
                );
                println!(
                    "game: preset={} aspect={:?} {}x{} scale={:.2} play={} mute={} stats={}",
                    prefs.game_view.preset.as_str(),
                    prefs.game_view.aspect,
                    prefs.game_view.width,
                    prefs.game_view.height,
                    prefs.game_view.scale,
                    prefs.game_view.play_mode.as_str(),
                    if prefs.game_view.mute_audio {
                        "on"
                    } else {
                        "off"
                    },
                    if prefs.game_view.stats { "on" } else { "off" },
                );
                if prefs.project_view.collapsed.is_empty() {
                    println!("project: collapsed=(all expanded)");
                } else {
                    println!(
                        "project: collapsed={}",
                        prefs.project_view.collapsed.join(",")
                    );
                }
                println!(
                    "console: clear-on-play={} clear-on-build={} collapse={} error-pause={}",
                    on_off(prefs.console.clear_on_play),
                    on_off(prefs.console.clear_on_build),
                    on_off(prefs.console.collapse),
                    on_off(prefs.console.error_pause),
                );
            }
            Ok(())
        }
        EditorCmd::SetConsole {
            game,
            clear_on_play,
            clear_on_build,
            collapse,
            error_pause,
        } => {
            let game_dir = find_game_dir(root, &game)?;
            let prefs = set_console(
                &game_dir,
                ConsolePatch {
                    clear_on_play: clear_on_play.as_deref(),
                    clear_on_build: clear_on_build.as_deref(),
                    collapse: collapse.as_deref(),
                    error_pause: error_pause.as_deref(),
                },
            )?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "message": "console prefs updated",
                        "path": game_dir.join(EDITOR_PREFS_REL).to_string_lossy(),
                        "console": prefs.console,
                        "prefs": prefs,
                    })
                );
                Ok(())
            } else {
                emit_ok(json, "console prefs updated")?;
                println!(
                    "console: clear-on-play={} clear-on-build={} collapse={} error-pause={}",
                    on_off(prefs.console.clear_on_play),
                    on_off(prefs.console.clear_on_build),
                    on_off(prefs.console.collapse),
                    on_off(prefs.console.error_pause),
                );
                Ok(())
            }
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
            handle_position,
            handle_rotation,
            tool,
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
                && handle_position.is_none()
                && handle_rotation.is_none()
                && tool.is_none()
            {
                anyhow::bail!(
                    "set-scene-view needs at least one flag (--zoom, --grid, --handle-position, --tool, …)"
                );
            }
            let prefs = set_scene_view(
                &game_dir,
                zoom,
                pan_x,
                pan_y,
                grid,
                gizmos,
                snap,
                snap_size,
                mode_2d,
                handle_position.as_deref(),
                handle_rotation.as_deref(),
                tool.as_deref(),
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

fn play_step(root: &Path, game: &str, frames: u32, json: bool) -> Result<()> {
    let game_dir = find_game_dir(root, game)?;
    let project = load_project(&game_dir)?;
    let input = Input::new();
    let prefs = load_editor_prefs(&game_dir)?;
    let report = headless_play_steps(
        PlayStart {
            workspace: root,
            package: &project.name,
            game_dir: &game_dir,
            scene_json: None,
            force_fallback: false,
            build_if_needed: false,
        },
        frames,
        &input,
        prefs.console.error_pause,
    )?;
    let positions: Vec<_> = report
        .positions
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "x": e.x,
                "y": e.y,
            })
        })
        .collect();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "backend": report.backend,
                "frames": report.frames,
                "entities": report.entities,
                "draw_commands": report.draw.commands,
                "sprites": report.draw.sprites,
                "text": report.draw.text,
                "quads": report.draw.quads,
                "discs": report.draw.discs,
                "positions": positions,
                "note": report.note,
                "paused_on_error": report.paused_on_error,
                "error": report.error,
            })
        );
    } else {
        println!("play-step {}", project.name);
        println!("  backend: {}", report.backend);
        println!("  frames: {}", report.frames);
        println!("  entities: {}", report.entities);
        println!(
            "  draw_commands: {} (sprites {} text {} quads {} discs {})",
            report.draw.commands,
            report.draw.sprites,
            report.draw.text,
            report.draw.quads,
            report.draw.discs
        );
        if !report.note.is_empty() {
            println!("  note: {}", report.note);
        }
        if report.paused_on_error {
            println!(
                "  paused on error: {}",
                report.error.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}
