//! CLI twin: `asset blend-tree` writes the same controller file as the editor Inspector
//! (both call `wiimaker_assets::write_state_blend_tree`), and animator-status reports blend state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use wiimaker_assets::{BlendDimension, BlendMotion, BlendTreeMeta};
use wiimaker_scene::{add_entity, save_project, save_scene, GameProject, MutateOpts, Scene};

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn run(args: &[&str]) -> std::process::Output {
    let out = wiimaker().args(args).output().expect("spawn wiimaker");
    assert!(
        out.status.success(),
        "wiimaker {args:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-cli-blend-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-blend");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn author_base(game: &str) {
    for (name, cells) in [("idle", "i0,i1"), ("walk", "w0,w1"), ("up", "u0,u1")] {
        run(&["asset", "anim", game, name, "--cells", cells, "--json"]);
    }
    run(&[
        "asset",
        "controller",
        game,
        "player",
        "--default",
        "Locomotion",
        "--states",
        "Locomotion:idle",
        "--param",
        "Speed:Float=0",
        "--param",
        "Heading:Float=0",
        "--json",
    ]);
    run(&[
        "entity",
        "add-component",
        game,
        "--name",
        "Player",
        "Animator",
        "--controller",
        "player",
        "--json",
    ]);
}

fn controller_path(game: &Path) -> PathBuf {
    game.join("assets").join("player.controller.json")
}

#[test]
fn cli_blend_tree_matches_library_write() {
    let dir = tmp_game("parity");
    let game = dir.to_str().unwrap();
    author_base(game);

    let mirror = std::env::temp_dir().join(format!("wiimaker-blend-mirror-{}", std::process::id()));
    let _ = fs::remove_dir_all(&mirror);
    fs::create_dir_all(mirror.join("assets")).unwrap();
    fs::copy(
        controller_path(&dir),
        mirror.join("assets/player.controller.json"),
    )
    .unwrap();

    run(&[
        "asset",
        "blend-tree",
        game,
        "player",
        "Locomotion",
        "--type",
        "1D",
        "--params",
        "Speed",
        "--motion",
        "idle:0",
        "--motion",
        "walk:1",
        "--json",
    ]);

    let tree = BlendTreeMeta {
        dimension: BlendDimension::OneD,
        params: vec!["Speed".into()],
        motions: vec![
            BlendMotion {
                clip: "idle".into(),
                threshold: Some(0.0),
                position: None,
            },
            BlendMotion {
                clip: "walk".into(),
                threshold: Some(1.0),
                position: None,
            },
        ],
    };
    wiimaker_assets::write_state_blend_tree(&mirror.join("assets"), "player", "Locomotion", tree)
        .unwrap();

    let cli = fs::read(controller_path(&dir)).unwrap();
    let lib = fs::read(mirror.join("assets/player.controller.json")).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&cli),
        String::from_utf8_lossy(&lib),
        "CLI and library (GUI path) produced different controller files"
    );
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

#[test]
fn fixture_hydrates_and_doctor_is_clean_for_blend() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blend-tree");
    let dir =
        std::env::temp_dir().join(format!("wiimaker-cli-blend-fixture-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy_dir(&fixture, &dir);
    let game = dir.to_str().unwrap();

    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("\"type\": \"1D\""), "{text}");
    assert!(text.contains("\"active\": \"idle\""), "{text}");

    let gunner = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Gunner",
        "--json",
    ]);
    let text = String::from_utf8_lossy(&gunner.stdout);
    assert!(text.contains("\"type\": \"2D\""), "{text}");
    assert!(text.contains("\"active\": \"walk\""), "{text}");

    let doctor = run(&["doctor", game]);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert!(!out.contains("blend state"), "{out}");
}

#[test]
fn cli_blend_tree_2d_and_status_json() {
    let dir = tmp_game("status");
    let game = dir.to_str().unwrap();
    author_base(game);

    run(&[
        "asset",
        "blend-tree",
        game,
        "player",
        "Locomotion",
        "--type",
        "2D",
        "--params",
        "Speed,Heading",
        "--motion",
        "idle:0,0",
        "--motion",
        "walk:1,0",
        "--motion",
        "up:0,1",
        "--json",
    ]);

    let body = fs::read_to_string(controller_path(&dir)).unwrap();
    assert!(body.contains("\"type\": \"2D\""), "{body}");
    assert!(body.contains("\"position\""), "{body}");

    run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--float",
        "Speed=1",
        "--float",
        "Heading=0",
        "--json",
    ]);
    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("\"blend\""), "{text}");
    assert!(text.contains("\"type\": \"2D\""), "{text}");
    assert!(text.contains("\"active\": \"walk\""), "{text}");
}
