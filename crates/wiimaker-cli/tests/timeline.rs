//! CLI twin: `asset timeline` + PlayableDirector + timeline-status.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{add_entity, save_project, save_scene, GameProject, MutateOpts, Scene};

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-timeline-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-timeline");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Director", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "IntroGhost", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "MainCamera", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    dir
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn timeline_asset_and_director_status_json() {
    let dir = tmp_game("status");
    let game = dir.to_str().unwrap();

    let wrote = wiimaker()
        .args([
            "asset",
            "timeline",
            game,
            "intro",
            "--duration",
            "4",
            "--track",
            "GhostAppear:Activation:IntroGhost:0.5-4:true",
            "--track",
            "PlayerMove:Animation:Player:0-2:chomp",
            "--track",
            "Stinger:Audio:-:0-0.5:beep:1",
            "--track",
            "CamSlide:Transform:MainCamera:0-2:320,240>400,240",
            "--json",
        ])
        .output()
        .expect("timeline");
    assert!(
        wrote.status.success(),
        "timeline failed: {}",
        String::from_utf8_lossy(&wrote.stderr)
    );
    let stdout = String::from_utf8_lossy(&wrote.stdout);
    assert!(stdout.contains("intro"), "{stdout}");

    let list = wiimaker()
        .args(["asset", "list-timelines", game, "--json"])
        .output()
        .expect("list");
    assert!(list.status.success());
    let listed = String::from_utf8_lossy(&list.stdout);
    assert!(listed.contains("intro"), "{listed}");

    let add = wiimaker()
        .args([
            "entity",
            "add-component",
            game,
            "--name",
            "Director",
            "PlayableDirector",
            "--timeline",
            "intro",
            "--play-on-awake",
            "true",
            "--loop",
            "false",
            "--json",
        ])
        .output()
        .expect("add");
    assert!(
        add.status.success(),
        "add-component: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let status = wiimaker()
        .args([
            "entity",
            "timeline-status",
            game,
            "--name",
            "Director",
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
    assert!(body.contains("intro"), "{body}");
    assert!(body.contains("\"playing\": true"), "{body}");

    let stopped = wiimaker()
        .args([
            "entity",
            "timeline-stop",
            game,
            "--name",
            "Director",
            "--json",
        ])
        .output()
        .expect("stop");
    assert!(
        stopped.status.success(),
        "stop: {}",
        String::from_utf8_lossy(&stopped.stderr)
    );
    let body = String::from_utf8_lossy(&stopped.stdout);
    assert!(body.contains("\"playing\": false"), "{body}");

    let set = wiimaker()
        .args([
            "entity", "set", game, "--name", "Director", "--loop", "true", "--json",
        ])
        .output()
        .expect("set");
    assert!(
        set.status.success(),
        "set: {}",
        String::from_utf8_lossy(&set.stderr)
    );
}

#[test]
fn director_play_on_awake_does_not_insert_audio_source() {
    let dir = tmp_game("poa");
    let game = dir.to_str().unwrap();

    let add = wiimaker()
        .args([
            "entity",
            "add-component",
            game,
            "--name",
            "Director",
            "PlayableDirector",
            "--timeline",
            "intro",
            "--json",
        ])
        .output()
        .expect("add");
    assert!(
        add.status.success(),
        "add-component: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let set = wiimaker()
        .args([
            "entity",
            "set",
            game,
            "--name",
            "Director",
            "--play-on-awake",
            "false",
            "--json",
        ])
        .output()
        .expect("set");
    assert!(
        set.status.success(),
        "entity set --play-on-awake: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let scene = wiimaker_scene::load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let ent = scene.find_entity("Director").expect("Director");
    let d = ent
        .components
        .playable_director
        .as_ref()
        .expect("PlayableDirector");
    assert!(!d.play_on_awake, "director play_on_awake should be false");
    assert!(
        ent.components.audio_source.is_none(),
        "director-only entity must not gain an AudioSource"
    );
}
