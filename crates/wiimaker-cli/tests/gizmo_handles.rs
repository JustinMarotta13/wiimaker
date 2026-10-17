//! CLI twins for Scene handle prefs and gizmo rotate/scale deltas.
//!
//! `entity set --rotation-deg` / `--sx --sy` must write the same scene as the
//! gizmo helpers for the same delta.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_component_disc, add_entity, load_scene, rotation_after_drag, save_project, save_scene,
    scale_factors, set_entity_rotation_z, set_entity_scale, GameProject, MutateOpts, ScaleHandle,
    Scene, SCALE_MIN,
};

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-cli-gizmo-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-gizmo");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Orb",
        &MutateOpts {
            x: Some(120.0),
            y: Some(80.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_disc(&mut scene, "Orb", 28.0, [72, 210, 160, 255]).unwrap();
    set_entity_scale(&mut scene, "Orb", 1.5, 2.0).unwrap();
    set_entity_rotation_z(&mut scene, "Orb", 10.0_f32.to_radians()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn scene_json(dir: &std::path::Path) -> String {
    fs::read_to_string(dir.join("scenes/main.scene.json")).unwrap()
}

#[test]
fn set_scene_view_handle_flags_and_old_defaults() {
    let dir = tmp_game("prefs");
    fs::create_dir_all(dir.join(".wiimaker")).unwrap();
    fs::write(
        dir.join(".wiimaker/prefs.toml"),
        "[scene_view]\nzoom = 1.5\n",
    )
    .unwrap();

    let before = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap(), "--json"])
        .output()
        .expect("prefs");
    assert!(
        before.status.success(),
        "{}",
        String::from_utf8_lossy(&before.stderr)
    );
    let text = String::from_utf8_lossy(&before.stdout);
    assert!(
        text.contains("\"handle_position\":\"pivot\"")
            || text.contains("\"handle_position\": \"pivot\""),
        "{text}"
    );
    assert!(
        text.contains("\"handle_rotation\":\"local\"")
            || text.contains("\"handle_rotation\": \"local\""),
        "{text}"
    );
    assert!(
        text.contains("\"tool\":\"move\"") || text.contains("\"tool\": \"move\""),
        "{text}"
    );

    let set = wiimaker()
        .args([
            "editor",
            "set-scene-view",
            dir.to_str().unwrap(),
            "--handle-position",
            "center",
            "--handle-rotation",
            "global",
            "--tool",
            "rotate",
            "--json",
        ])
        .output()
        .expect("set-scene-view");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );
    let text = String::from_utf8_lossy(&set.stdout);
    assert!(
        text.contains("\"handle_position\":\"center\"")
            || text.contains("\"handle_position\": \"center\""),
        "{text}"
    );
    assert!(
        text.contains("\"handle_rotation\":\"global\"")
            || text.contains("\"handle_rotation\": \"global\""),
        "{text}"
    );
    assert!(
        text.contains("\"tool\":\"rotate\"") || text.contains("\"tool\": \"rotate\""),
        "{text}"
    );

    let again = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap(), "--json"])
        .output()
        .expect("prefs again");
    let text = String::from_utf8_lossy(&again.stdout);
    assert!(text.contains("center"), "{text}");
    assert!(text.contains("global"), "{text}");
    assert!(text.contains("rotate"), "{text}");
}

#[test]
fn cli_rotate_and_scale_match_gizmo_helper() {
    let dir = tmp_game("delta");
    let start_rot = 10.0_f32.to_radians();
    let origin = [120.0, 80.0];
    // 20° sweep from the entity's +X, which snaps to 15° steps → 30° absolute (10+20 → 30).
    let start_ptr = [origin[0] + 40.0, origin[1]];
    let sweep = 20.0_f32.to_radians();
    let ptr = [
        origin[0] + 40.0 * sweep.cos(),
        origin[1] + 40.0 * sweep.sin(),
    ];
    let new_rot = rotation_after_drag(start_rot, origin, start_ptr, ptr, true);
    assert!(
        (new_rot.to_degrees() - 30.0).abs() < 0.2,
        "{}",
        new_rot.to_degrees()
    );

    let factors = scale_factors(
        ScaleHandle::X,
        origin,
        0.0,
        [origin[0] + 40.0, origin[1]],
        [origin[0] + 80.0, origin[1] + 12.0],
    );
    let sx = wiimaker_scene::apply_axis_scale(1.5, factors[0]);
    let sy = wiimaker_scene::apply_axis_scale(2.0, factors[1]);
    assert!((sx - 3.0).abs() < 1e-3, "{sx}");
    assert!((sy - 2.0).abs() < 1e-3, "{sy}");
    assert!(sx >= SCALE_MIN && sy >= SCALE_MIN);

    let deg_arg = format!("{:.6}", new_rot.to_degrees());
    let sx_arg = format!("{:.6}", sx);
    let sy_arg = format!("{:.6}", sy);
    let deg: f32 = deg_arg.parse().unwrap();
    let sx_p: f32 = sx_arg.parse().unwrap();
    let sy_p: f32 = sy_arg.parse().unwrap();

    let mut via_helper = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    set_entity_rotation_z(&mut via_helper, "Orb", deg.to_radians()).unwrap();
    set_entity_scale(&mut via_helper, "Orb", sx_p, sy_p).unwrap();
    let helper_path = dir.join("scenes/helper.scene.json");
    save_scene(&helper_path, &via_helper).unwrap();
    let helper_json = fs::read_to_string(&helper_path).unwrap();

    let cli = wiimaker()
        .args([
            "entity",
            "set",
            dir.to_str().unwrap(),
            "--name",
            "Orb",
            "--rotation-deg",
            &deg_arg,
            "--sx",
            &sx_arg,
            "--sy",
            &sy_arg,
            "--json",
        ])
        .output()
        .expect("entity set");
    assert!(
        cli.status.success(),
        "stderr {}\nstdout {}",
        String::from_utf8_lossy(&cli.stderr),
        String::from_utf8_lossy(&cli.stdout)
    );
    let cli_json = scene_json(&dir);
    assert_eq!(
        cli_json, helper_json,
        "CLI scene JSON diverged from the gizmo helper\nCLI:\n{cli_json}\nhelper:\n{helper_json}"
    );
}
