//! `scene set-game-view` play chrome + `editor play-step` against the CI scaffold.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_play::{dylib_path, example_dylib_path};
use wiimaker_scene::{
    add_component_sprite, add_entity, save_project, save_scene, GameProject, MutateOpts, Scene,
};

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn build_play_probe_dylib(root: &PathBuf) {
    let status = Command::new("cargo")
        .args(["build", "-p", "wiimaker-play", "--example", "play_probe"])
        .current_dir(root)
        .status()
        .expect("spawn cargo build play_probe");
    assert!(status.success(), "play_probe example failed to build");
    let src = example_dylib_path(root, "play_probe");
    assert!(src.is_file(), "missing probe dylib at {}", src.display());
    let dest = dylib_path(root, "play-probe");
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::copy(&src, &dest).unwrap();
}

fn tmp_play_probe_game(root: &PathBuf) -> PathBuf {
    build_play_probe_dylib(root);
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-play-probe-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scenes")).unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"play-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n",
    )
    .unwrap();
    fs::write(dir.join("src/lib.rs"), "// probe dylib supplied by test\n").unwrap();
    let mut project = GameProject::new("play-probe");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &Scene::new("main")).unwrap();
    dir
}

fn tmp_game() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-play-step-prefs-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-play-step");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &Scene::new("main")).unwrap();
    dir
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn set_game_view_play_chrome_roundtrip_and_old_defaults() {
    let dir = tmp_game();
    fs::create_dir_all(dir.join(".wiimaker")).unwrap();
    fs::write(
        dir.join(".wiimaker/prefs.toml"),
        "[game_view]\npreset = \"free\"\nscale = 1.0\n",
    )
    .unwrap();

    let before = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap(), "--json"])
        .output()
        .expect("prefs");
    assert!(
        before.status.success(),
        "{}",
        String::from_utf8_lossy(&before.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
    let gv = &v["prefs"]["game_view"];
    assert_eq!(gv["play_mode"], "focused");
    assert_eq!(gv["mute_audio"], false);
    assert_eq!(gv["stats"], false);

    let set = wiimaker()
        .args([
            "scene",
            "set-game-view",
            dir.to_str().unwrap(),
            "--play-mode",
            "maximized",
            "--mute-audio",
            "on",
            "--stats",
            "on",
            "--json",
        ])
        .output()
        .expect("set-game-view");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );
    let set_v: serde_json::Value = serde_json::from_slice(&set.stdout).unwrap();
    assert_eq!(set_v["prefs"]["game_view"]["play_mode"], "maximized");
    assert_eq!(set_v["prefs"]["game_view"]["mute_audio"], true);
    assert_eq!(set_v["prefs"]["game_view"]["stats"], true);

    let after = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap(), "--json"])
        .output()
        .expect("prefs again");
    assert!(after.status.success());
    let after_v: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    assert_eq!(after_v["prefs"]["game_view"]["play_mode"], "maximized");
    assert_eq!(after_v["prefs"]["game_view"]["mute_audio"], true);
    assert_eq!(after_v["prefs"]["game_view"]["stats"], true);

    let off = wiimaker()
        .args([
            "scene",
            "set-game-view",
            dir.to_str().unwrap(),
            "--play-mode",
            "focused",
            "--mute-audio",
            "off",
            "--stats",
            "off",
        ])
        .output()
        .expect("set off");
    assert!(
        off.status.success(),
        "{}",
        String::from_utf8_lossy(&off.stderr)
    );
    let text = fs::read_to_string(dir.join(".wiimaker/prefs.toml")).unwrap();
    assert!(text.contains("focused"));
    assert!(text.contains("mute_audio = false") || text.contains("mute_audio = false\n"));
    assert!(!text.contains("maximized"));
}

#[test]
fn play_step_cli_on_ci_scaffold() {
    let root = workspace_root();
    let status = Command::new("bash")
        .arg(root.join("tools/ci-scaffold-hello-orb.sh"))
        .status()
        .expect("scaffold");
    assert!(status.success());

    let out = wiimaker()
        .args([
            "editor",
            "play-step",
            "hello-orb",
            "--frames",
            "4",
            "--json",
        ])
        .env("WIIMAKER_AUDIO", "0")
        .output()
        .expect("play-step");
    assert!(
        out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["frames"], 4);
    assert_eq!(v["paused_on_error"], false);
    assert!(v["error"].is_null());
    let entities = v["entities"].as_u64().unwrap();
    assert!(entities >= 1, "{v}");
    assert!(v["draw_commands"].as_u64().unwrap() >= 1, "{v}");
    let backend = v["backend"].as_str().unwrap();
    assert!(backend == "plugin" || backend == "fallback", "{backend}");
    let positions = v["positions"].as_array().unwrap();
    assert_eq!(positions.len() as u64, entities);
    assert!(positions.iter().any(|p| {
        p["name"].as_str().is_some_and(|n| !n.is_empty())
            && p["x"].as_f64().is_some()
            && p["y"].as_f64().is_some()
    }));

    let text = wiimaker()
        .args(["editor", "play-step", "hello-orb", "--frames", "1"])
        .env("WIIMAKER_AUDIO", "0")
        .output()
        .expect("play-step text");
    assert!(
        text.status.success(),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("frames: 1"), "{stdout}");
    assert!(stdout.contains("backend:"), "{stdout}");
    assert!(stdout.contains("draw_commands:"), "{stdout}");
    assert!(!stdout.contains("\"positions\""), "{stdout}");
}

#[test]
fn play_step_cli_sprite_missing_from_wpack() {
    let dir = tmp_game();
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Hero",
        &MutateOpts {
            x: Some(100.0),
            y: Some(50.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_sprite(&mut scene, "Hero", "basic_space_suit", [32.0, 32.0]).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let out = wiimaker()
        .args([
            "editor",
            "play-step",
            dir.to_str().unwrap(),
            "--frames",
            "2",
            "--json",
        ])
        .env("WIIMAKER_AUDIO", "0")
        .output()
        .expect("play-step sprite");
    assert!(
        out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["frames"], 2);
    assert!(v["entities"].as_u64().unwrap() >= 1);
}

#[test]
fn play_step_error_pause_pauses_on_failed_plugin_tick() {
    let root = workspace_root();
    let dir = tmp_play_probe_game(&root);
    wiimaker()
        .args([
            "editor",
            "set-console",
            dir.to_str().unwrap(),
            "--error-pause",
            "on",
        ])
        .output()
        .expect("set error-pause");

    let out = wiimaker()
        .args([
            "editor",
            "play-step",
            dir.to_str().unwrap(),
            "--frames",
            "3",
            "--json",
        ])
        .env("WIIMAKER_AUDIO", "0")
        .env("WIIMAKER_PLAY_PROBE_FAIL", "1")
        .output()
        .expect("play-step");
    assert!(
        out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["backend"], "plugin");
    assert_eq!(v["paused_on_error"], true);
    assert_eq!(v["frames"], 1);
    let err = v["error"].as_str().unwrap();
    assert!(
        err.contains("play plugin update failed"),
        "error: {err}"
    );
}

#[test]
fn play_step_fails_plugin_tick_when_error_pause_off() {
    let root = workspace_root();
    let dir = tmp_play_probe_game(&root);
    wiimaker()
        .args([
            "editor",
            "set-console",
            dir.to_str().unwrap(),
            "--error-pause",
            "off",
        ])
        .output()
        .expect("set error-pause off");

    let out = wiimaker()
        .args([
            "editor",
            "play-step",
            dir.to_str().unwrap(),
            "--frames",
            "2",
            "--json",
        ])
        .env("WIIMAKER_AUDIO", "0")
        .env("WIIMAKER_PLAY_PROBE_FAIL", "1")
        .output()
        .expect("play-step");
    assert!(
        !out.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("play plugin update failed") || err.contains("update failed"),
        "stderr: {err}"
    );
}

#[test]
fn play_step_rejects_invalid_frame_counts() {
    let root = workspace_root();
    let status = Command::new("bash")
        .arg(root.join("tools/ci-scaffold-hello-orb.sh"))
        .status()
        .expect("scaffold");
    assert!(status.success());

    for frames in ["0", "10001"] {
        let out = wiimaker()
            .args(["editor", "play-step", "hello-orb", "--frames", frames])
            .env("WIIMAKER_AUDIO", "0")
            .output()
            .expect("play-step");
        assert!(!out.status.success(), "frames={frames} should fail");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("frames") || err.contains("--frames"),
            "frames={frames} stderr: {err}"
        );
    }
}
