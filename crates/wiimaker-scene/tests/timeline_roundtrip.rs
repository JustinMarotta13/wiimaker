//! Timeline asset roundtrip + PlayableDirector tick (activation, transform, anim, audio).

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_anim_clip, write_beep_wav, write_timeline, AnimClipCatalog, TimelineCatalog,
    TimelineClip, TimelineMeta, TimelineTrack, TimelineTrackKind,
};
use wiimaker_scene::{
    add_component_animation, add_component_playable_director, add_entity, animate_world, diagnose,
    hydrate_lenient_with_all_catalogs, save_project, save_scene, GameProject, MutateOpts,
    TextureMap,
};

fn tmp_game() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-timeline-scene-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn intro() -> TimelineMeta {
    TimelineMeta {
        duration: 4.0,
        tracks: vec![
            TimelineTrack {
                name: "GhostAppear".into(),
                kind: TimelineTrackKind::Activation,
                binding: Some("IntroGhost".into()),
                clips: vec![TimelineClip::activation(0.5, 4.0, true)],
            },
            TimelineTrack {
                name: "PlayerMove".into(),
                kind: TimelineTrackKind::Animation,
                binding: Some("Player".into()),
                clips: vec![TimelineClip::animation(0.0, 2.0, "chomp")],
            },
            TimelineTrack {
                name: "Stinger".into(),
                kind: TimelineTrackKind::Audio,
                binding: None,
                clips: vec![TimelineClip::audio(0.0, 0.5, "beep", 1.0)],
            },
            TimelineTrack {
                name: "CamSlide".into(),
                kind: TimelineTrackKind::Transform,
                binding: Some("MainCamera".into()),
                clips: vec![TimelineClip::transform(
                    0.0,
                    2.0,
                    [320.0, 240.0],
                    [400.0, 240.0],
                )],
            },
        ],
    }
}

#[test]
fn timeline_asset_and_director_tick() {
    let dir = tmp_game();
    let mut project = GameProject::new("timeline-test");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let assets = dir.join("assets");
    write_anim_clip(&assets, "chomp", vec!["a".into(), "b".into()], 10.0, true).unwrap();
    write_anim_clip(&assets, "idle", vec!["a".into()], 10.0, true).unwrap();
    write_beep_wav(assets.join("beep.wav"), 0.1).unwrap();
    let (path, _) = write_timeline(&assets, "intro", intro()).unwrap();
    let loaded = TimelineMeta::load(&path).unwrap();
    assert_eq!(loaded.tracks.len(), 4);

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Director", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "IntroGhost", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_entity(
        &mut scene,
        "MainCamera",
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            ..MutateOpts::default()
        },
    )
    .unwrap();
    add_component_animation(&mut scene, "Player", "idle", None, true).unwrap();
    add_component_playable_director(&mut scene, "Director", "intro", true, false).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        Some(&anims),
        None,
        None,
        Some(&timelines),
    );
    let ghost = world.find_by_name("IntroGhost").unwrap();
    let player = world.find_by_name("Player").unwrap();
    let cam = world.find_by_name("MainCamera").unwrap();
    let director = world.find_by_name("Director").unwrap();
    assert!(
        world.is_active(ghost),
        "authored active until the playhead applies"
    );
    assert!(world.director(director).unwrap().playing);

    world.set_timeline_time(director, 0.0);
    assert!(!world.is_active(ghost), "outside the activation clip");
    world.set_timeline_time(director, 0.15);
    let anim = world.animation(player).unwrap();
    assert_eq!(anim.clip, "chomp");
    assert_eq!(anim.cells, vec!["a".to_string(), "b".to_string()]);
    assert!(anim.hold_time);
    assert_eq!(anim.frame, 1, "local 0.15s at 10 fps");
    world.set_timeline_time(director, 1.0);
    assert!(world.is_active(ghost));
    let xf = world.transform(cam).unwrap();
    assert!((xf.translation.x - 360.0).abs() < 1e-2);
    assert!((xf.translation.y - 240.0).abs() < 1e-2);
    assert!(world.transform(cam).unwrap().translation.z.abs() < 1e-4);

    // Fresh hydrate so the stinger fires once from t = 0.
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        Some(&anims),
        None,
        None,
        Some(&timelines),
    );
    let director = world.find_by_name("Director").unwrap();
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        0.05,
    );
    let shots = world.drain_oneshots();
    assert_eq!(shots.len(), 1);
    assert_eq!(shots[0].clip, "beep");
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        0.05,
    );
    assert!(
        world.drain_oneshots().is_empty(),
        "audio fires once per playthrough"
    );
    assert!(world.director(director).unwrap().time > 0.05);

    world.set_timeline_time(director, 3.0);
    let cam = world.find_by_name("MainCamera").unwrap();
    assert!((world.transform(cam).unwrap().translation.x - 400.0).abs() < 1e-2);
    let player = world.find_by_name("Player").unwrap();
    assert_eq!(
        world.animation(player).unwrap().clip,
        "idle",
        "outside the anim clip"
    );

    let diag = diagnose(&dir, &project);
    assert!(
        diag.issues.iter().all(|i| !i.message.contains("intro")),
        "doctor should accept the intro timeline: {:?}",
        diag.issues
    );
    let _ = fs::remove_dir_all(&dir);
}
