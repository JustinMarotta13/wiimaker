//! `editor set-visibility` / `editor set-scene-view --scene-visibility` and prefs.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_entity, save_project, save_scene, set_entity_parent, GameProject, MutateOpts, Scene,
};

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn tmp_game() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-visibility-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-visibility");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Parent",
        &MutateOpts {
            x: Some(10.0),
            y: Some(10.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Child",
        &MutateOpts {
            x: Some(40.0),
            y: Some(10.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(&mut scene, "Other", &MutateOpts::default()).unwrap();
    set_entity_parent(&mut scene, "Child", Some("Parent")).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn json(cmd: &mut Command) -> serde_json::Value {
    let out = cmd.output().expect("run");
    assert!(
        out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn old_prefs_default_and_set_visibility_roundtrip() {
    let dir = tmp_game();
    let game = dir.to_str().unwrap();
    fs::create_dir_all(dir.join(".wiimaker")).unwrap();
    fs::write(
        dir.join(".wiimaker/prefs.toml"),
        "[game_view]\npreset = \"free\"\nscale = 1.0\n",
    )
    .unwrap();

    let before = json(wiimaker().args(["editor", "prefs", game, "--json"]));
    assert_eq!(before["prefs"]["scene_visibility"]["enabled"], true);
    assert_eq!(
        before["prefs"]["scene_visibility"]["scenes"],
        serde_json::json!([])
    );

    let set = json(wiimaker().args([
        "editor",
        "set-visibility",
        game,
        "--scene",
        "main",
        "--entity",
        "Parent",
        "--hidden",
        "on",
        "--pickable",
        "off",
        "--json",
    ]));
    assert_eq!(set["ok"], true);
    assert_eq!(set["scene"], "scenes/main.scene.json");
    assert_eq!(set["recursive"], true);
    assert_eq!(set["affected"], serde_json::json!(["Child", "Parent"]));
    assert_eq!(set["hidden"], serde_json::json!(["Child", "Parent"]));
    assert_eq!(set["unpickable"], serde_json::json!(["Child", "Parent"]));
    assert_eq!(set["prefs"]["scene_visibility"]["enabled"], true);

    let isolate = json(wiimaker().args([
        "editor",
        "set-visibility",
        game,
        "--entity",
        "Child",
        "--hidden",
        "off",
        "--recursive",
        "off",
        "--json",
    ]));
    assert_eq!(isolate["affected"], serde_json::json!(["Child"]));
    assert_eq!(isolate["hidden"], serde_json::json!(["Parent"]));
    assert_eq!(
        isolate["unpickable"],
        serde_json::json!(["Child", "Parent"])
    );

    let toggled = json(wiimaker().args([
        "editor",
        "set-scene-view",
        game,
        "--scene-visibility",
        "off",
        "--json",
    ]));
    assert_eq!(toggled["ok"], true);
    assert_eq!(toggled["scene_visibility"], false);
    assert_eq!(
        toggled["prefs"]["scene_visibility"]["scenes"][0]["hidden"],
        serde_json::json!(["Parent"])
    );

    let after = json(wiimaker().args(["editor", "prefs", game, "--json"]));
    assert_eq!(after["prefs"]["scene_visibility"]["enabled"], false);

    let text = wiimaker()
        .args(["editor", "prefs", game])
        .output()
        .expect("prefs text");
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(
        stdout.contains("scene-visibility: enabled=off") && stdout.contains("hidden=Parent"),
        "{stdout}"
    );

    let bad = wiimaker()
        .args([
            "editor",
            "set-visibility",
            game,
            "--entity",
            "Parent",
            "--hidden",
            "maybe",
        ])
        .output()
        .expect("bad flag");
    assert!(!bad.status.success());

    let missing = wiimaker()
        .args([
            "editor",
            "set-visibility",
            game,
            "--entity",
            "Nope",
            "--hidden",
            "on",
        ])
        .output()
        .expect("missing entity");
    assert!(!missing.status.success());

    let empty = wiimaker()
        .args(["editor", "set-visibility", game, "--entity", "Parent"])
        .output()
        .expect("no flags");
    assert!(!empty.status.success());
}

#[test]
fn rename_and_remove_prune_visibility_names() {
    let dir = tmp_game();
    let game = dir.to_str().unwrap();
    json(wiimaker().args([
        "editor",
        "set-visibility",
        game,
        "--entity",
        "Other",
        "--hidden",
        "on",
        "--pickable",
        "off",
        "--json",
    ]));
    json(wiimaker().args([
        "editor",
        "set-visibility",
        game,
        "--entity",
        "Parent",
        "--hidden",
        "on",
        "--recursive",
        "on",
        "--json",
    ]));

    let renamed = wiimaker()
        .args(["entity", "rename", game, "Other", "Renamed"])
        .output()
        .expect("rename");
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let after_rename = json(wiimaker().args(["editor", "prefs", game, "--json"]));
    let scenes = &after_rename["prefs"]["scene_visibility"]["scenes"][0];
    let hidden = scenes["hidden"].as_array().unwrap();
    assert!(hidden.iter().any(|n| n == "Renamed"));
    assert!(!hidden.iter().any(|n| n == "Other"));
    assert!(hidden.iter().any(|n| n == "Parent"));

    let removed = wiimaker()
        .args(["entity", "remove", game, "--name", "Parent"])
        .output()
        .expect("remove");
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    let after_remove = json(wiimaker().args(["editor", "prefs", game, "--json"]));
    let hidden = &after_remove["prefs"]["scene_visibility"]["scenes"][0]["hidden"];
    assert_eq!(hidden, &serde_json::json!(["Renamed"]));
}
