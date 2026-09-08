use std::path::Path;

use anyhow::Result;
use wiimaker_scene::{find_game_dir, load_editor_prefs, set_scene_view, EDITOR_PREFS_REL};

use crate::args::EditorCmd;
use crate::util::emit_ok;

pub fn editor_cmd(root: &Path, cmd: EditorCmd, json: bool) -> Result<()> {
    match cmd {
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
    }
}
