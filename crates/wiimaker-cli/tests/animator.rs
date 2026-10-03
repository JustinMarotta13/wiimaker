//! CLI twin: `asset controller` + Animator component + animator-status.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{add_entity, save_project, save_scene, GameProject, MutateOpts, Scene};

fn tmp_game() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-cli-animator-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-anim");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn controller_and_animator_status_json() {
    let dir = tmp_game();
    let game = dir.to_str().unwrap();

    let idle = wiimaker()
        .args(["asset", "anim", game, "idle", "--cells", "i0,i1", "--json"])
        .output()
        .expect("anim idle");
    assert!(
        idle.status.success(),
        "{}",
        String::from_utf8_lossy(&idle.stderr)
    );
    let walk = wiimaker()
        .args(["asset", "anim", game, "walk", "--cells", "w0,w1", "--json"])
        .output()
        .expect("anim walk");
    assert!(
        walk.status.success(),
        "{}",
        String::from_utf8_lossy(&walk.stderr)
    );

    let ctrl = wiimaker()
        .args([
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
            "--transition",
            "Walk>Idle:Moving=false",
            "--json",
        ])
        .output()
        .expect("controller");
    assert!(
        ctrl.status.success(),
        "controller failed: {}",
        String::from_utf8_lossy(&ctrl.stderr)
    );
    let stdout = String::from_utf8_lossy(&ctrl.stdout);
    assert!(stdout.contains("player"), "{stdout}");

    let list = wiimaker()
        .args(["asset", "list-controllers", game, "--json"])
        .output()
        .expect("list");
    assert!(list.status.success());
    let listed = String::from_utf8_lossy(&list.stdout);
    assert!(listed.contains("player"), "{listed}");

    let add = wiimaker()
        .args([
            "entity",
            "add-component",
            game,
            "--name",
            "Player",
            "Animator",
            "--controller",
            "player",
            "--json",
        ])
        .output()
        .expect("add animator");
    assert!(
        add.status.success(),
        "add-component: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let status = wiimaker()
        .args([
            "entity",
            "animator-status",
            game,
            "--name",
            "Player",
            "--json",
        ])
        .output()
        .expect("status");
    assert!(
        status.status.success(),
        "status: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let body = String::from_utf8_lossy(&status.stdout);
    assert!(
        body.contains("\"controller\": \"player\"") || body.contains("\"controller\":\"player\""),
        "{body}"
    );
    assert!(body.contains("Idle"), "{body}");

    let setp = wiimaker()
        .args([
            "entity",
            "animator-set",
            game,
            "--name",
            "Player",
            "--bool",
            "Moving=true",
            "--json",
        ])
        .output()
        .expect("set");
    assert!(
        setp.status.success(),
        "animator-set: {}",
        String::from_utf8_lossy(&setp.stderr)
    );
}
