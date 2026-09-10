//! AudioSource hydrate + doctor (host-first oneshots).

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::write_beep_wav;
use wiimaker_scene::{
    add_component_audio_source, add_entity, diagnose, hydrate, load_scene, save_project, save_scene,
    set_entity_audio_source, GameProject, MutateOpts, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-audio-roundtrip-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn json_file_roundtrip_add_set_hydrate() {
    let dir = tmp_game("hydrate");
    let assets = dir.join("assets");
    write_beep_wav(assets.join("beep.wav"), 0.05).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_audio_source(&mut scene, "Player", "beep", 0.75, true).unwrap();

    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("AudioSource"));
    assert!(text.contains("beep"));

    let mut edited = load_scene(&path).unwrap();
    set_entity_audio_source(&mut edited, "Player", Some("beep"), Some(0.25), Some(false)).unwrap();
    save_scene(&path, &edited).unwrap();
    let reloaded = load_scene(&path).unwrap();
    let a = reloaded
        .find_entity("Player")
        .unwrap()
        .components
        .audio_source
        .as_ref()
        .unwrap();
    assert_eq!(a.clip, "beep");
    assert!((a.volume - 0.25).abs() < 1e-4);
    assert!(!a.play_on_awake);

    let world = hydrate(&reloaded, &TextureMap::new()).unwrap();
    let id = world.find_by_name("Player").unwrap();
    let src = world.audio_source(id).unwrap();
    assert_eq!(src.clip, "beep");
    assert!(src.play_on_awake == false);
    assert!((src.volume - 0.25).abs() < 1e-4);

    let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
    world.queue_awake_audio();
    let q = world.drain_oneshots();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].clip, "beep");
}

#[test]
fn doctor_warns_missing_wav() {
    let dir = tmp_game("doctor");
    let mut project = GameProject::new("audio-doc");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_audio_source(&mut scene, "Player", "no-such-clip", 1.0, false).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let diag = diagnose(&dir, &project);
    assert!(
        diag.issues
            .iter()
            .any(|m| m.message.contains("no-such-clip") && m.message.contains(".wav")),
        "{:?}",
        diag.issues
    );
}
