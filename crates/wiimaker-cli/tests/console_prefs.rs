//! `editor set-console` + `editor prefs` console block.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{save_project, save_scene, GameProject, Scene};

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

fn tmp_game() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-console-prefs-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-console");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &Scene::new("main")).unwrap();
    dir
}

#[test]
fn old_prefs_default_clear_on_play_and_set_console_roundtrip() {
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
    let console = &v["prefs"]["console"];
    assert_eq!(console["clear_on_play"], true);
    assert_eq!(console["clear_on_build"], false);
    assert_eq!(console["collapse"], false);
    assert_eq!(console["error_pause"], false);

    let set = wiimaker()
        .args([
            "editor",
            "set-console",
            dir.to_str().unwrap(),
            "--clear-on-play",
            "off",
            "--clear-on-build",
            "on",
            "--collapse",
            "on",
            "--error-pause",
            "on",
            "--json",
        ])
        .output()
        .expect("set-console");
    assert!(
        set.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&set.stdout),
        String::from_utf8_lossy(&set.stderr)
    );
    let set_v: serde_json::Value = serde_json::from_slice(&set.stdout).unwrap();
    assert_eq!(set_v["ok"], true);
    assert_eq!(set_v["console"]["clear_on_play"], false);
    assert_eq!(set_v["console"]["clear_on_build"], true);
    assert_eq!(set_v["console"]["collapse"], true);
    assert_eq!(set_v["console"]["error_pause"], true);
    assert_eq!(set_v["prefs"]["console"]["error_pause"], true);

    let after = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap(), "--json"])
        .output()
        .expect("prefs again");
    assert!(after.status.success());
    let after_v: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    assert_eq!(after_v["prefs"]["console"]["clear_on_play"], false);
    assert_eq!(after_v["prefs"]["console"]["collapse"], true);

    let text = wiimaker()
        .args(["editor", "prefs", dir.to_str().unwrap()])
        .output()
        .expect("prefs text");
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(
        stdout.contains("clear-on-play=off") && stdout.contains("error-pause=on"),
        "{stdout}"
    );

    let bad = wiimaker()
        .args([
            "editor",
            "set-console",
            dir.to_str().unwrap(),
            "--collapse",
            "maybe",
        ])
        .output()
        .expect("bad flag");
    assert!(!bad.status.success());
}

#[test]
fn play_step_reports_paused_on_error_false_without_a_tick_error() {
    let dir = tmp_game();
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
            "1",
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
    assert_eq!(v["paused_on_error"], false);
    assert!(v["error"].is_null());
}
