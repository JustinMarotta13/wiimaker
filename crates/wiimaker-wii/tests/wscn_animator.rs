//! Bake → parse WSCN0003 Animator / PlayableDirector (additive tail).

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_anim_clip, write_animator_controller, write_timeline, AnimClipCatalog,
    AnimatorControllerCatalog, AnimatorControllerMeta, ControllerCondition, ControllerParam,
    ControllerParamType, ControllerState, ControllerTransition, ResolvedSprite, SpriteCatalog,
    TimelineCatalog, TimelineClip, TimelineMeta, TimelineTrack, TimelineTrackKind, WPack,
};
use wiimaker_scene::{
    add_component_animator, add_component_playable_director, add_component_sprite, add_entity,
    bake_scene_wscn, bake_scene_wscn_with_all_catalogs, MutateOpts, Scene,
};
use wiimaker_wii::parse_wscn;

fn tmp(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-wscn-anim-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    dir
}

fn player_controller() -> AnimatorControllerMeta {
    AnimatorControllerMeta {
        default_state: "Idle".into(),
        parameters: vec![ControllerParam {
            name: "Speed".into(),
            kind: ControllerParamType::Float,
            default: None,
        }],
        states: vec![
            ControllerState::plain("Idle", "idle"),
            ControllerState::plain("Walk", "walk"),
        ],
        transitions: vec![
            ControllerTransition {
                from: "Idle".into(),
                to: "Walk".into(),
                conditions: vec![ControllerCondition {
                    param: "Speed".into(),
                    greater: Some(0.5),
                    ..Default::default()
                }],
                has_exit_time: false,
            },
            ControllerTransition {
                from: "Walk".into(),
                to: "Idle".into(),
                conditions: vec![ControllerCondition {
                    param: "Speed".into(),
                    less: Some(0.5),
                    ..Default::default()
                }],
                has_exit_time: false,
            },
        ],
        weight: 1.0,
        layers: Vec::new(),
        graph: Default::default(),
    }
}

fn intro_timeline() -> TimelineMeta {
    TimelineMeta {
        duration: 2.0,
        tracks: vec![
            TimelineTrack {
                name: "GhostAppear".into(),
                kind: TimelineTrackKind::Activation,
                binding: Some("Ghost".into()),
                property: None,
                clips: vec![TimelineClip::activation(0.5, 2.0, true)],
            },
            TimelineTrack {
                name: "CamSlide".into(),
                kind: TimelineTrackKind::Transform,
                binding: Some("Ghost".into()),
                property: None,
                clips: vec![TimelineClip::transform(
                    0.0,
                    1.0,
                    [10.0, 20.0],
                    [40.0, 20.0],
                )],
            },
        ],
    }
}

fn dummy_pack(name: &str) -> WPack {
    let mut pack = WPack::new();
    pack.textures.push(wiimaker_assets::PackedTexture {
        name: name.into(),
        width: 32,
        height: 32,
        rgba16: vec![0u8; 32 * 32 * 2],
    });
    pack
}

#[test]
fn parse_old_wscn_without_anim_tables() {
    let mut scene = Scene::new("t");
    add_entity(
        &mut scene,
        "Orb",
        &MutateOpts {
            x: Some(8.0),
            y: Some(9.0),
            ..Default::default()
        },
    )
    .unwrap();
    wiimaker_scene::add_component_disc(&mut scene, "Orb", 4.0, [1, 2, 3, 255]).unwrap();
    let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
    let loaded = parse_wscn(&bytes).expect("old tail");
    assert!(loaded.cells.is_empty());
    let id = loaded.world.find_by_name("Orb").unwrap();
    assert!(loaded.world.animator(id).is_none());
    assert!(loaded.world.director(id).is_none());
    assert!(loaded.world.disc(id).is_some());
}

#[test]
fn bake_parse_animator_and_director() {
    let dir = tmp("roundtrip");
    let assets = dir.join("assets");
    write_anim_clip(
        &assets,
        "idle",
        vec!["idle_0".into(), "idle_1".into()],
        10.0,
        true,
    )
    .unwrap();
    write_anim_clip(&assets, "walk", vec!["walk_0".into()], 10.0, true).unwrap();
    write_animator_controller(&assets, "player", player_controller()).unwrap();
    write_timeline(&assets, "intro", intro_timeline()).unwrap();

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let controllers = AnimatorControllerCatalog::load_dir(&assets).unwrap();
    let timelines = TimelineCatalog::load_dir(&assets).unwrap();
    let pack = dummy_pack("sheet");
    let mut cat = SpriteCatalog::empty();
    cat.insert(
        "idle_0",
        ResolvedSprite {
            sheet_texture: "sheet".into(),
            uv: [0.0, 0.0, 0.5, 1.0],
            pivot: [0.5, 0.5],
            pixel_size: [16.0, 16.0],
            is_cell: true,
        },
    );

    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Hero",
        &MutateOpts {
            x: Some(32.0),
            y: Some(48.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_sprite(&mut scene, "Hero", "sheet", [16.0, 16.0]).unwrap();
    add_component_animator(&mut scene, "Hero", "player").unwrap();
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            ..Default::default()
        },
    )
    .unwrap();
    wiimaker_scene::add_component_disc(&mut scene, "Ghost", 8.0, [255, 0, 0, 255]).unwrap();
    add_entity(
        &mut scene,
        "Cut",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_playable_director(&mut scene, "Cut", "intro", true, false).unwrap();

    let bytes = bake_scene_wscn_with_all_catalogs(
        &scene,
        &pack,
        Some(&cat),
        Some(&anims),
        Some(&controllers),
        Some(&timelines),
    )
    .unwrap();
    assert_eq!(&bytes[0..8], b"WSCN0003");

    let loaded = parse_wscn(&bytes).expect("parse");
    assert!(!loaded.cells.is_empty());
    assert!(loaded.cells.iter().any(|c| c.name == "idle_0"));

    let hero = loaded.world.find_by_name("Hero").unwrap();
    let a = loaded.world.animator(hero).expect("animator");
    assert_eq!(a.controller, "player");
    assert_eq!(a.state, "Idle");
    assert_eq!(a.parameters.len(), 1);
    assert_eq!(a.states.len(), 2);
    let anim = loaded.world.animation(hero).expect("driven animation");
    assert_eq!(anim.clip, "idle");
    assert_eq!(anim.cells, vec!["idle_0", "idle_1"]);

    let cut = loaded.world.find_by_name("Cut").unwrap();
    let d = loaded.world.director(cut).expect("director");
    assert_eq!(d.timeline, "intro");
    assert!(d.play_on_awake);
    assert!(!d.loop_);
    assert_eq!(d.tracks.len(), 2);
    assert!((d.duration - 2.0).abs() < 1e-4);

    let mut world = loaded.world;
    assert!(world.set_animator_float(hero, "Speed", 1.0));
    world.tick_animators(0.0);
    assert_eq!(world.animator(hero).unwrap().state, "Walk");
    assert_eq!(world.animation(hero).unwrap().clip, "walk");
}

#[test]
fn animator_only_entity_is_kind_none() {
    let mut scene = Scene::new("t");
    add_entity(
        &mut scene,
        "Brain",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_animator(&mut scene, "Brain", "player").unwrap();
    let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
    // name "Brain" (5) + pose 24 + kind
    let mut i = 8 + 4 + 4;
    let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    i += 2 + nlen + 24;
    assert_eq!(bytes[i], 0);
    let loaded = parse_wscn(&bytes).expect("parse");
    let id = loaded.world.find_by_name("Brain").unwrap();
    assert!(loaded.world.animator(id).is_some());
}
