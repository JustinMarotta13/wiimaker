//! CLI smoke: `tilemap from-ascii maze.txt` (`--json`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_entity, load_scene, save_project, save_scene, tilemap_get_cell, GameProject, MutateOpts,
    Scene,
};

fn tmp_game() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-from-ascii-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-ascii");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Maze", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    fs::write(
        dir.join("assets/maze.txt"),
        "#####\n#...#\n#####\n",
    )
    .unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn from_ascii_file_json_and_scene_roundtrip() {
    let dir = tmp_game();
    let out = wiimaker()
        .args([
            "tilemap",
            "from-ascii",
            dir.to_str().unwrap(),
            "maze.txt",
            "--name",
            "Maze",
            "--json",
        ])
        .output()
        .expect("run wiimaker");
    assert!(
        out.status.success(),
        "from-ascii failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"ok\":true") || stdout.contains("\"ok\": true"),
        "{stdout}"
    );
    assert!(stdout.contains("\"width\":5") || stdout.contains("\"width\": 5"), "{stdout}");
    assert!(
        stdout.contains("\"height\":3") || stdout.contains("\"height\": 3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resized\":true") || stdout.contains("\"resized\": true"),
        "{stdout}"
    );

    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let tm = scene
        .find_entity("Maze")
        .unwrap()
        .components
        .tilemap
        .as_ref()
        .unwrap();
    assert_eq!((tm.width, tm.height), (5, 3));
    assert_eq!(tilemap_get_cell(&scene, "Maze", 0, 0).unwrap(), (1, true));
    assert_eq!(tilemap_get_cell(&scene, "Maze", 1, 1).unwrap(), (0, false));
    assert_eq!(tilemap_get_cell(&scene, "Maze", 4, 2).unwrap(), (1, true));
}

#[test]
fn from_ascii_custom_map_json() {
    let dir = tmp_game();
    fs::write(dir.join("assets/dots.txt"), "###\n#P#\n###\n").unwrap();
    let out = wiimaker()
        .args([
            "tilemap",
            "from-ascii",
            dir.to_str().unwrap(),
            "assets/dots.txt",
            "--name",
            "Maze",
            "--map",
            "#=1,P=2:0",
            "--json",
        ])
        .output()
        .expect("run wiimaker map");
    assert!(
        out.status.success(),
        "from-ascii --map failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let scene = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    assert_eq!(tilemap_get_cell(&scene, "Maze", 1, 1).unwrap(), (2, false));
    assert_eq!(tilemap_get_cell(&scene, "Maze", 0, 0).unwrap(), (1, true));
}
