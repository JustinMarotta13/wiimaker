//! CLI twin: `entity set --pivot-x/--pivot-y` / `--clear-pivot` and add-component Sprite.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_entity, load_scene, save_project, save_scene, GameProject, MutateOpts, Scene,
};

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-sprite-pivot-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-pivot");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hero", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn add_component_sprite_pivot_set_and_clear_json() {
    let dir = tmp_game("set-clear");
    let add = wiimaker()
        .args([
            "entity",
            "add-component",
            dir.to_str().unwrap(),
            "--name",
            "Hero",
            "Sprite",
            "--texture",
            "hero_2",
            "--pivot-x",
            "0",
            "--pivot-y",
            "1",
            "--json",
        ])
        .output()
        .expect("run wiimaker");
    assert!(
        add.status.success(),
        "add-component failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let stdout = String::from_utf8_lossy(&add.stdout);
    assert!(
        stdout.contains("\"ok\":true") || stdout.contains("\"ok\": true"),
        "{stdout}"
    );
    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    assert_eq!(
        scene
            .find_entity("Hero")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap()
            .pivot,
        Some([0.0, 1.0])
    );

    let set = wiimaker()
        .args([
            "entity",
            "set",
            dir.to_str().unwrap(),
            "--name",
            "Hero",
            "--pivot-x",
            "0.25",
            "--pivot-y",
            "0.75",
            "--json",
        ])
        .output()
        .expect("run entity set");
    assert!(
        set.status.success(),
        "entity set failed: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    assert_eq!(
        scene
            .find_entity("Hero")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap()
            .pivot,
        Some([0.25, 0.75])
    );

    let clear = wiimaker()
        .args([
            "entity",
            "set",
            dir.to_str().unwrap(),
            "--name",
            "Hero",
            "--clear-pivot",
            "--json",
        ])
        .output()
        .expect("run clear-pivot");
    assert!(
        clear.status.success(),
        "clear-pivot failed: {}",
        String::from_utf8_lossy(&clear.stderr)
    );
    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    assert!(scene
        .find_entity("Hero")
        .unwrap()
        .components
        .sprite
        .as_ref()
        .unwrap()
        .pivot
        .is_none());
    let text = fs::read_to_string(dir.join("scenes/main.scene.json")).unwrap();
    assert!(!text.contains("pivot"), "{text}");
}

#[test]
fn set_pivot_errors_without_sprite() {
    let dir = tmp_game("no-sprite");
    let out = wiimaker()
        .args([
            "entity",
            "set",
            dir.to_str().unwrap(),
            "--name",
            "Hero",
            "--pivot-x",
            "0",
            "--pivot-y",
            "1",
        ])
        .output()
        .expect("run entity set");
    assert!(!out.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(err.contains("no Sprite"), "{err}");
}
