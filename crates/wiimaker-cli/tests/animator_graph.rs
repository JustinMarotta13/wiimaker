//! CLI twin for the Animator window: positions, state rename/remove, parameters.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
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

fn run_fail(args: &[&str]) -> std::process::Output {
    let out = wiimaker().args(args).output().expect("spawn wiimaker");
    assert!(
        !out.status.success(),
        "wiimaker {args:?} should have failed:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    out
}

fn tmp_game(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("wiimaker-cli-agraph-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-agraph");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("json stdout")
}

#[test]
fn cli_graph_positions_params_and_state_edits() {
    let dir = tmp_game("graph");
    let game = dir.to_str().unwrap();
    run(&[
        "asset",
        "controller",
        game,
        "player",
        "--default",
        "Idle",
        "--states",
        "Idle:idle,Walk:walk",
        "--param",
        "Moving:Bool=false",
        "--transition",
        "Idle>Walk:Moving=true",
        "--json",
    ]);
    let before = fs::read_to_string(dir.join("assets/player.controller.json")).unwrap();
    assert!(!before.contains("\"position\""));
    let listed = json(&run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "list",
        "--json",
    ]));
    let nodes = listed["layers"][0]["nodes"].as_array().unwrap();
    let idle = nodes.iter().find(|n| n["id"] == "Idle").unwrap();
    assert_eq!(idle["authored"], false);
    assert!(idle["x"].is_number() && idle["y"].is_number());

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "move-state",
        "--name",
        "Base",
        "--state",
        "Idle",
        "--pos",
        "180,40",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "move-state",
        "--name",
        "Base",
        "--state",
        "Entry",
        "--pos",
        "12,8",
        "--json",
    ]);
    let graph = json(&run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "graph",
        "--name",
        "Base",
        "--json",
    ]));
    let idle = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "Idle")
        .unwrap();
    assert_eq!(idle["x"], 180.0);
    assert_eq!(idle["y"], 40.0);
    assert_eq!(idle["authored"], true);
    let entry = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "Entry")
        .unwrap();
    assert_eq!(entry["authored"], true);

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "rename-state",
        "--name",
        "Base",
        "--state",
        "Idle",
        "--to",
        "Stand",
        "--json",
    ]);
    let text = fs::read_to_string(dir.join("assets/player.controller.json")).unwrap();
    assert!(text.contains("\"default\": \"Stand\""));
    assert!(text.contains("\"from\": \"Stand\""));
    assert!(text.contains("\"position\""));

    run(&[
        "asset",
        "controller-param",
        game,
        "player",
        "add",
        "--name",
        "Speed",
        "--type",
        "Float",
        "--default",
        "0",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-param",
        game,
        "player",
        "rename",
        "--name",
        "Moving",
        "--to",
        "Running",
        "--json",
    ]);
    let text = fs::read_to_string(dir.join("assets/player.controller.json")).unwrap();
    assert!(text.contains("\"Running\""));
    assert!(!text.contains("\"Moving\""));

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "set-transition",
        "--name",
        "Base",
        "--index",
        "0",
        "--has-exit-time",
        "true",
        "--json",
    ]);
    let text = fs::read_to_string(dir.join("assets/player.controller.json")).unwrap();
    assert!(text.contains("\"has_exit_time\": true"));

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "state",
        "--name",
        "Base",
        "--state",
        "Walk",
        "--remove",
        "--json",
    ]);
    let text = fs::read_to_string(dir.join("assets/player.controller.json")).unwrap();
    assert!(!text.contains("\"Walk\""));
    assert!(!text.contains("\"to\": \"Walk\""));

    let err = run_fail(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "weight",
        "--name",
        "Base",
        "--weight",
        "2.5",
    ]);
    let err_text = format!(
        "{}{}",
        String::from_utf8_lossy(&err.stdout),
        String::from_utf8_lossy(&err.stderr)
    );
    assert!(err_text.contains("does not affect playback"), "{err_text}");
    let _ = fs::remove_dir_all(&dir);
}
