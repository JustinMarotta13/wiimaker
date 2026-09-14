//! CLI twin smoke: `entity add-component Text` + `entity set --text` (`--json`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{add_entity, save_project, save_scene, GameProject, MutateOpts, Scene};

fn tmp_game() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-cli-text-hud-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-text");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hud", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn add_component_text_and_set_json() {
    let dir = tmp_game();
    let add = wiimaker()
        .args([
            "entity",
            "add-component",
            dir.to_str().unwrap(),
            "--name",
            "Hud",
            "Text",
            "--text",
            "Score: 0",
            "--size",
            "16",
            "--color",
            "255,220,64",
            "--align",
            "left",
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

    let set = wiimaker()
        .args([
            "entity",
            "set",
            dir.to_str().unwrap(),
            "--name",
            "Hud",
            "--text",
            "Score: 1",
            "--align",
            "center",
            "--json",
        ])
        .output()
        .expect("run wiimaker set");
    assert!(
        set.status.success(),
        "entity set failed: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let scene = fs::read_to_string(dir.join("scenes/main.scene.json")).unwrap();
    assert!(scene.contains("Score: 1"), "{scene}");
    assert!(
        scene.contains("Center") || scene.contains("\"align\": \"Center\""),
        "{scene}"
    );
}
