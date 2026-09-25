//! CLI twins: `entity set --rotation-deg` + `entity set-parent` under a rotated parent.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_entity, load_scene, save_project, save_scene, GameProject, MutateOpts, Scene,
};

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-parent-rot-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-parent-rot");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Parent",
        &MutateOpts {
            x: Some(100.0),
            y: Some(200.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Child",
        &MutateOpts {
            x: Some(110.0),
            y: Some(200.0),
            ..Default::default()
        },
    )
    .unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

#[test]
fn set_rotation_then_set_parent_preserves_world_and_orbits() {
    let dir = tmp_game("set-parent");
    let game = dir.to_str().unwrap();

    let rot = wiimaker()
        .args([
            "entity",
            "set",
            game,
            "--name",
            "Parent",
            "--rotation-deg",
            "90",
            "--json",
        ])
        .output()
        .expect("entity set");
    assert!(
        rot.status.success(),
        "entity set --rotation-deg failed: {}",
        String::from_utf8_lossy(&rot.stderr)
    );

    let parented = wiimaker()
        .args([
            "entity",
            "set-parent",
            game,
            "--name",
            "Child",
            "--parent",
            "Parent",
            "--json",
        ])
        .output()
        .expect("entity set-parent");
    assert!(
        parented.status.success(),
        "entity set-parent failed: {}",
        String::from_utf8_lossy(&parented.stderr)
    );
    let stdout = String::from_utf8_lossy(&parented.stdout);
    assert!(
        stdout.contains("\"ok\":true") || stdout.contains("\"ok\": true"),
        "{stdout}"
    );

    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let child = scene.find_entity("Child").unwrap();
    assert_eq!(child.parent.as_deref(), Some("Parent"));
    // World pose preserved: still (110, 200). Local is inverse-rotated (10, 0) → (0, -10).
    let world = scene.world_transform("Child").unwrap();
    near(world.translation[0], 110.0);
    near(world.translation[1], 200.0);
    near(child.transform.translation[0], 0.0);
    near(child.transform.translation[1], -10.0);

    // Rotating the parent after parenting orbits the child (local stays).
    let rot2 = wiimaker()
        .args([
            "entity",
            "set",
            game,
            "--name",
            "Parent",
            "--rotation-deg",
            "180",
            "--json",
        ])
        .output()
        .expect("entity set parent 180");
    assert!(
        rot2.status.success(),
        "second rotation failed: {}",
        String::from_utf8_lossy(&rot2.stderr)
    );
    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let world = scene.world_transform("Child").unwrap();
    // Local (0, -10) under 180°: (−0, 10) wait: 180° of (0, -10) = (0, 10)?
    // x' = x cos − y sin = 0 − (−10)(0) wait sin(180)=0, cos=-1
    // x' = 0*(-1) - (-10)*0 = 0
    // y' = 0*0 + (-10)*(-1) = 10
    // world = (100, 210)
    near(world.translation[0], 100.0);
    near(world.translation[1], 210.0);
}
