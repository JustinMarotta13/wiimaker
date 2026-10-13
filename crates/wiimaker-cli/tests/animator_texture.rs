//! Sprite + Animator hydrate uses the cooked `.wpack` texture map.
//!
//! `animator-set --json` validates that hydrate before writing the scene.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{add_entity, save_project, save_scene, GameProject, MutateOpts, Scene};

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-anim-tex-{tag}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-anim-tex");
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

fn cook_orb(dir: &std::path::Path) {
    let png = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../wiimaker-assets/fixtures/hud_font.png");
    fs::copy(&png, dir.join("assets/orb.png")).expect("copy orb png");
    let game = dir.to_str().unwrap();
    run(&["cook", game]);
    assert!(dir.join("assets.wpack").is_file(), "cook wrote assets.wpack");
}

fn author_player(game: &str, texture: &str) {
    run(&["asset", "anim", game, "idle", "--cells", "i0,i1", "--json"]);
    run(&[
        "asset",
        "controller",
        game,
        "player",
        "--default",
        "Idle",
        "--states",
        "Idle:idle",
        "--param",
        "Moving:Bool=false",
        "--json",
    ]);
    run(&[
        "entity",
        "add-component",
        game,
        "--name",
        "Player",
        "Sprite",
        "--texture",
        texture,
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

#[test]
fn animator_status_and_set_json_use_wpack_textures() {
    let dir = tmp_game("ok");
    let game = dir.to_str().unwrap();
    cook_orb(&dir);
    author_player(game, "orb");

    let status = run(&[
        "entity",
        "animator-status",
        game,
        "--name",
        "Player",
        "--json",
    ]);
    let body = String::from_utf8_lossy(&status.stdout);
    assert!(body.contains("\"controller\": \"player\""), "{body}");
    assert!(body.contains("Idle"), "{body}");

    let setp = run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--bool",
        "Moving=true",
        "--json",
    ]);
    let set_body = String::from_utf8_lossy(&setp.stdout);
    assert!(set_body.contains("\"ok\": true"), "{set_body}");
    let scene = fs::read_to_string(dir.join("scenes/main.scene.json")).unwrap();
    assert!(
        scene.contains("\"Moving\"") && scene.contains("true"),
        "{scene}"
    );

    run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--bool",
        "Moving=false",
    ]);
    let scene = fs::read_to_string(dir.join("scenes/main.scene.json")).unwrap();
    assert!(scene.contains("false"), "{scene}");

    run(&["doctor", game]);
}

#[test]
fn animator_set_json_does_not_write_when_hydrate_fails() {
    let dir = tmp_game("miss");
    let game = dir.to_str().unwrap();
    cook_orb(&dir);
    author_player(game, "ghost");

    let scene_path = dir.join("scenes/main.scene.json");
    let before = fs::read(&scene_path).unwrap();

    let out = wiimaker()
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
        .expect("spawn wiimaker");
    assert!(
        !out.status.success(),
        "animator-set --json should fail when the sprite texture is missing from the wpack:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("not found in wpack"),
        "stderr should name the wpack miss: {err}"
    );
    let after = fs::read(&scene_path).unwrap();
    assert_eq!(
        before, after,
        "hydrate failure must not leave the animator mutation on disk"
    );

    let plain = run(&[
        "entity",
        "animator-set",
        game,
        "--name",
        "Player",
        "--bool",
        "Moving=true",
    ]);
    assert!(plain.status.success());
    let scene = fs::read_to_string(&scene_path).unwrap();
    assert!(
        scene.contains("\"Moving\"") && scene.contains("true"),
        "non-json animator-set still persists: {scene}"
    );
}
