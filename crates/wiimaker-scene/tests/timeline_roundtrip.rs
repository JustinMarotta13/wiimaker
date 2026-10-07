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

fn tmp_game(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-timeline-scene-{}-{tag}",
        std::process::id()
    ));
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
                property: None,
                clips: vec![TimelineClip::activation(0.5, 4.0, true)],
            },
            TimelineTrack {
                name: "PlayerMove".into(),
                kind: TimelineTrackKind::Animation,
                binding: Some("Player".into()),
                property: None,
                clips: vec![TimelineClip::animation(0.0, 2.0, "chomp")],
            },
            TimelineTrack {
                name: "Stinger".into(),
                kind: TimelineTrackKind::Audio,
                binding: None,
                property: None,
                clips: vec![TimelineClip::audio(0.0, 0.5, "beep", 1.0)],
            },
            TimelineTrack {
                name: "CamSlide".into(),
                kind: TimelineTrackKind::Transform,
                binding: Some("MainCamera".into()),
                property: None,
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
    let dir = tmp_game("tick");
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
    assert!(world.director(director).unwrap().playing);
    world.set_timeline_time(director, 0.2);
    assert!(
        !world.is_active(ghost),
        "README GhostAppear: hidden before 0.5s after the playhead applies"
    );

    world.set_timeline_time(director, 0.15);
    assert!(
        !world.is_active(ghost),
        "still hidden at t=0.15 (before clip start)"
    );
    let anim = world.animation(player).unwrap();
    assert_eq!(anim.clip, "chomp");
    assert_eq!(anim.cells, vec!["a".to_string(), "b".to_string()]);
    assert!(anim.hold_time);
    assert_eq!(anim.frame, 1, "local 0.15s at 10 fps");
    world.set_timeline_time(director, 1.0);
    assert!(world.is_active(ghost), "shown inside GhostAppear clip");
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

/// README sample: `GhostAppear:Activation:IntroGhost:0.5-4:true` via scene hydrate.
#[test]
fn readme_ghost_appear_through_scene_hydrate() {
    let dir = tmp_game("ghost");
    let mut project = GameProject::new("ghost-appear");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let assets = dir.join("assets");
    let meta = TimelineMeta {
        duration: 4.0,
        tracks: vec![
            TimelineTrack {
                name: "GhostAppear".into(),
                kind: TimelineTrackKind::Activation,
                binding: Some("IntroGhost".into()),
                property: None,
                clips: vec![TimelineClip::activation(0.5, 4.0, true)],
            },
            TimelineTrack {
                name: "CamSlide".into(),
                kind: TimelineTrackKind::Transform,
                binding: Some("MainCamera".into()),
                property: None,
                clips: vec![TimelineClip::transform(
                    0.0,
                    2.0,
                    [320.0, 240.0],
                    [400.0, 240.0],
                )],
            },
        ],
    };
    write_timeline(&assets, "intro", meta).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Director", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "IntroGhost", &MutateOpts::default()).unwrap();
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
    add_component_playable_director(&mut scene, "Director", "intro", true, false).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        None,
        None,
        None,
        Some(&timelines),
    );
    let ghost = world.find_by_name("IntroGhost").unwrap();
    let director = world.find_by_name("Director").unwrap();
    world.apply_playing_timelines();
    assert!(
        !world.is_active(ghost),
        "play_on_awake at t=0: hidden before 0.5s (no World::set_active)"
    );
    world.set_timeline_time(director, 0.2);
    assert!(!world.is_active(ghost), "t=0.2 still before the clip");
    world.set_timeline_time(director, 0.5);
    assert!(world.is_active(ghost), "shown at clip start");
    world.set_timeline_time(director, 1.0);
    assert!(world.is_active(ghost), "shown inside the clip");
    world.stop_timeline(director);
    assert!(!world.is_active(ghost), "Stop returns to t=0, still hidden");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hydrate_signal_and_control_tracks() {
    let dir = tmp_game("signal-control");
    let mut project = GameProject::new("signal-control");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let assets = dir.join("assets");
    write_timeline(
        &assets,
        "sub",
        TimelineMeta {
            duration: 1.0,
            tracks: vec![TimelineTrack {
                name: "Cue".into(),
                kind: TimelineTrackKind::Signal,
                binding: Some("Child".into()),
                property: None,
                clips: vec![TimelineClip::signal(0.0, "SubStart", None)],
            }],
        },
    )
    .unwrap();
    write_timeline(
        &assets,
        "intro",
        TimelineMeta {
            duration: 3.0,
            tracks: vec![
                TimelineTrack {
                    name: "Cues".into(),
                    kind: TimelineTrackKind::Signal,
                    binding: Some("Player".into()),
                    property: None,
                    clips: vec![TimelineClip::signal(0.25, "IntroDone", Some("go"))],
                },
                TimelineTrack {
                    name: "Nest".into(),
                    kind: TimelineTrackKind::Control,
                    binding: Some("Child".into()),
                    property: None,
                    clips: vec![TimelineClip::control(0.5, 1.5)],
                },
            ],
        },
    )
    .unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Director", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "Child", &MutateOpts::default()).unwrap();
    add_component_playable_director(&mut scene, "Director", "intro", true, false).unwrap();
    add_component_playable_director(&mut scene, "Child", "sub", false, false).unwrap();

    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        None,
        None,
        None,
        Some(&timelines),
    );
    let director = world.find_by_name("Director").unwrap();
    let child = world.find_by_name("Child").unwrap();
    assert_eq!(world.director(director).unwrap().tracks.len(), 2);
    assert_eq!(
        world.director(director).unwrap().tracks[0].kind,
        wiimaker_core::TimelineTrackKind::Signal
    );
    assert_eq!(
        world.director(director).unwrap().tracks[1].kind,
        wiimaker_core::TimelineTrackKind::Control
    );

    world.tick_timelines(0.1);
    assert!(world.take_timeline_signals().is_empty());
    world.tick_timelines(0.2);
    let signals = world.take_timeline_signals();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].signal, "IntroDone");
    assert_eq!(signals[0].payload, "go");
    assert_eq!(signals[0].binding, "Player");
    assert!(!world.is_active(child));

    world.tick_timelines(0.3);
    assert!(world.is_active(child));
    let sub = world.director(child).unwrap();
    assert!(sub.playing);
    assert!(sub.time < 0.2, "nested local time {}", sub.time);
    let nested = world.take_timeline_signals();
    assert!(
        nested.iter().any(|s| s.signal == "SubStart"),
        "control clip starts the child director: {nested:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn legacy_from_to_file_matches_lerp_and_curves_override() {
    let dir = tmp_game("curves");
    let mut project = GameProject::new("curve-test");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let assets = dir.join("assets");
    let legacy = r#"{
        "duration": 4.0,
        "tracks": [{
            "name": "CamSlide",
            "kind": "Transform",
            "binding": "MainCamera",
            "clips": [{ "start": 0.0, "end": 2.0, "from": [320.0, 240.0], "to": [400.0, 240.0] }]
        }]
    }"#;
    fs::write(assets.join("intro.timeline.json"), legacy).unwrap();
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Director", &MutateOpts::default()).unwrap();
    add_entity(&mut scene, "MainCamera", &MutateOpts::default()).unwrap();
    add_component_playable_director(&mut scene, "Director", "intro", true, false).unwrap();
    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        None,
        None,
        None,
        Some(&timelines),
    );
    let director = world.find_by_name("Director").unwrap();
    let cam = world.find_by_name("MainCamera").unwrap();
    world.set_timeline_time(director, 1.0);
    assert!((world.transform(cam).unwrap().translation.x - 360.0).abs() < 1e-2);
    world.set_timeline_time(director, 3.0);
    assert!((world.transform(cam).unwrap().translation.x - 400.0).abs() < 1e-2);

    let curved = r#"{
        "duration": 4.0,
        "tracks": [{
            "name": "CamSlide",
            "kind": "Transform",
            "binding": "MainCamera",
            "clips": [{
                "start": 0.0,
                "end": 2.0,
                "from": [0.0, 0.0],
                "to": [0.0, 80.0],
                "curves": { "x": [
                    {"t": 0.0, "v": 0.0, "interp": "linear"},
                    {"t": 2.0, "v": 10.0, "interp": "linear"}
                ]}
            }]
        }]
    }"#;
    fs::write(assets.join("intro.timeline.json"), curved).unwrap();
    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_lenient_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        None,
        None,
        None,
        Some(&timelines),
    );
    let director = world.find_by_name("Director").unwrap();
    let cam = world.find_by_name("MainCamera").unwrap();
    world.set_timeline_time(director, 1.0);
    let xf = world.transform(cam).unwrap();
    assert!((xf.translation.x - 5.0).abs() < 1e-2, "curved x");
    assert!((xf.translation.y - 40.0).abs() < 1e-2, "y stays from→to");

    let bad = r#"{
        "duration": 2.0,
        "tracks": [{
            "name": "Spin",
            "kind": "Float",
            "binding": "MainCamera",
            "property": "Transform.foo",
            "clips": [{
                "start": 0.0,
                "end": 1.0,
                "curves": {
                    "value": [],
                    "x": [{"t": 5.0, "v": 1.0, "interp": "bezier"}]
                }
            }]
        }]
    }"#;
    fs::write(assets.join("bad.timeline.json"), bad).unwrap();
    let diag = diagnose(&dir, &project);
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("unknown float property")),
        "{msgs:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("is empty")), "{msgs:?}");
    assert!(
        msgs.iter().any(|m| m.contains("outside the clip")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("unknown interp")),
        "{msgs:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}
