//! CLI twin: `asset controller-layer` authors override layers; status reports them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let dir =
        std::env::temp_dir().join(format!("wiimaker-cli-layers-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-layers");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

#[test]
fn cli_authors_override_layer_and_status_json() {
    let dir = tmp_game("author");
    let game = dir.to_str().unwrap();
    run(&["asset", "anim", game, "walk", "--cells", "w0,w1", "--json"]);
    run(&["asset", "anim", game, "aim", "--cells", "a0", "--json"]);
    run(&[
        "asset",
        "controller",
        game,
        "player",
        "--default",
        "Walk",
        "--states",
        "Walk:walk",
        "--param",
        "Aiming:Bool=false",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "add",
        "--name",
        "UpperBody",
        "--weight",
        "1",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "state",
        "--name",
        "UpperBody",
        "--state",
        "Aim",
        "--clip",
        "aim",
        "--json",
    ]);
    let listed = run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "list",
        "--json",
    ]);
    let body = String::from_utf8_lossy(&listed.stdout);
    assert!(body.contains("UpperBody"), "{body}");
    assert!(body.contains("Base"), "{body}");

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
    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    let body = String::from_utf8_lossy(&status.stdout);
    assert!(body.contains("\"name\": \"UpperBody\""), "{body}");
    assert!(body.contains("\"state\": \"Aim\""), "{body}");
    assert!(body.contains("\"clip\": \"aim\""), "{body}");

    run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--layer",
        "UpperBody",
        "--weight",
        "0",
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
    let body = String::from_utf8_lossy(&status.stdout);
    assert!(body.contains("\"clip\": \"walk\""), "{body}");
    assert!(body.contains("\"layer\": \"Base\""), "{body}");

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "weight",
        "--name",
        "UpperBody",
        "--weight",
        "2",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "add",
        "--name",
        "Empty",
        "--json",
    ]);
    let doctor = run(&["doctor", game]);
    let text = String::from_utf8_lossy(&doctor.stdout);
    assert!(text.contains("outside 0..1"), "{text}");
    assert!(text.contains("layer 'Empty' is empty"), "{text}");
}

fn run_fail(args: &[&str]) -> std::process::Output {
    let out = wiimaker().args(args).output().expect("spawn wiimaker");
    assert!(
        !out.status.success(),
        "wiimaker {args:?} should have failed:\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    out
}

fn animator_game(tag: &str) -> PathBuf {
    let dir = tmp_game(tag);
    let game = dir.to_str().unwrap();
    run(&["asset", "anim", game, "walk", "--cells", "w0,w1", "--json"]);
    run(&["asset", "anim", game, "aim", "--cells", "a0", "--json"]);
    run(&[
        "asset",
        "controller",
        game,
        "player",
        "--default",
        "Walk",
        "--states",
        "Walk:walk",
        "--param",
        "Aiming:Bool=false",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "add",
        "--name",
        "UpperBody",
        "--weight",
        "1",
        "--json",
    ]);
    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "state",
        "--name",
        "UpperBody",
        "--state",
        "Aim",
        "--clip",
        "aim",
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
    dir
}

#[test]
fn controller_layer_weight_rejects_non_finite_and_stays_loadable() {
    let dir = animator_game("nonfinite-controller");
    let game = dir.to_str().unwrap();
    let ctrl = dir.join("assets/player.controller.json");
    let before = fs::read_to_string(&ctrl).unwrap();
    for bad in ["NaN", "inf", "-inf"] {
        let weight = format!("--weight={bad}");
        let out = run_fail(&[
            "asset",
            "controller-layer",
            game,
            "player",
            "weight",
            "--name",
            "UpperBody",
            &weight,
            "--json",
        ]);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("must be a finite number"), "{bad}: {err}");
        assert_eq!(
            fs::read_to_string(&ctrl).unwrap(),
            before,
            "{bad} changed the controller file"
        );
    }
    let add = run_fail(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "add",
        "--name",
        "Face",
        "--weight=NaN",
        "--json",
    ]);
    assert!(String::from_utf8_lossy(&add.stderr).contains("must be a finite number"));
    let spaced = run_fail(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "weight",
        "--name",
        "UpperBody",
        "--weight",
        "-inf",
        "--json",
    ]);
    assert!(String::from_utf8_lossy(&spaced.stderr).contains("must be a finite number"));
    assert_eq!(fs::read_to_string(&ctrl).unwrap(), before);
    assert!(!before.contains("null"));

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "list",
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
    assert!(String::from_utf8_lossy(&status.stdout).contains("\"clip\": \"aim\""));

    run(&[
        "asset",
        "controller-layer",
        game,
        "player",
        "weight",
        "--name",
        "UpperBody",
        "--weight",
        "-0.5",
        "--json",
    ]);
    assert!(fs::read_to_string(&ctrl)
        .unwrap()
        .contains("\"weight\": -0.5"));
}

#[test]
fn animator_set_layer_weight_rejects_non_finite_and_scene_stays_loadable() {
    let dir = animator_game("nonfinite-scene");
    let game = dir.to_str().unwrap();
    let scene_path = dir.join("scenes/main.scene.json");
    let before = fs::read_to_string(&scene_path).unwrap();
    for bad in ["NaN", "inf", "-inf"] {
        let weight = format!("--weight={bad}");
        let out = run_fail(&[
            "entity",
            "animator-set",
            game,
            "--name",
            "Player",
            "--layer",
            "UpperBody",
            &weight,
            "--json",
        ]);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("must be a finite number"), "{bad}: {err}");
        assert_eq!(
            fs::read_to_string(&scene_path).unwrap(),
            before,
            "{bad} changed the scene"
        );
    }
    let spaced = run_fail(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--layer",
        "UpperBody",
        "--weight",
        "-inf",
        "--json",
    ]);
    assert!(String::from_utf8_lossy(&spaced.stderr).contains("must be a finite number"));
    assert_eq!(fs::read_to_string(&scene_path).unwrap(), before);
    assert!(!before.contains("null"));

    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("\"clip\": \"aim\""));
}

#[test]
fn animator_set_unknown_layer_lists_valid_names() {
    let dir = animator_game("unknown-layer");
    let game = dir.to_str().unwrap();
    let scene_path = dir.join("scenes/main.scene.json");
    let before = fs::read_to_string(&scene_path).unwrap();
    let out = run_fail(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--layer",
        "Typo",
        "--weight",
        "0.5",
        "--json",
    ]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("layer 'Typo'"), "{err}");
    assert!(err.contains("(valid: Base, UpperBody)"), "{err}");
    assert_eq!(fs::read_to_string(&scene_path).unwrap(), before);

    run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--layer",
        "base",
        "--weight",
        "0.5",
        "--json",
    ]);
    run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--layer",
        "UpperBody",
        "--weight",
        "0",
        "--json",
    ]);
}

#[test]
fn doctor_warns_when_override_layer_has_no_default() {
    let dir = animator_game("no-default");
    let game = dir.to_str().unwrap();
    let healthy = run(&["doctor", game]);
    assert!(!String::from_utf8_lossy(&healthy.stdout).contains("has no default state"));

    let ctrl = dir.join("assets/player.controller.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ctrl).unwrap()).unwrap();
    doc["layers"][0].as_object_mut().unwrap().remove("default");
    fs::write(&ctrl, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let broken = run(&["doctor", game]);
    let text = String::from_utf8_lossy(&broken.stdout);
    assert!(
        text.contains("layer 'UpperBody' has no default state"),
        "{text}"
    );
}

#[test]
fn fixture_override_aim_status() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layers");
    let dir = tmp_game("fixture");
    copy_dir(&src, &dir);
    let game = dir.to_str().unwrap();
    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    let body = String::from_utf8_lossy(&status.stdout);
    assert!(body.contains("UpperBody"), "{body}");
    assert!(body.contains("\"clip\": \"aim\""), "{body}");
    assert!(
        body.contains("\"state\": \"Walk\"") || body.contains("Walk"),
        "{body}"
    );
}

fn copy_dir(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&to).unwrap();
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}
