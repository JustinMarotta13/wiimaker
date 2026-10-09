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
