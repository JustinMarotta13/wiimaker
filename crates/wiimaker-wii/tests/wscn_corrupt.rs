//! Malformed WSCN tail: bounded reserve, truncation, non-finite floats.

use wiimaker_assets::WPack;
use wiimaker_scene::{
    add_component_audio_source, add_entity, bake_scene_wscn, bake_scene_wscn_with_all_catalogs,
    MutateOpts, Scene,
};
use wiimaker_wii::{parse_wscn, ParseError};

fn minimal_wscn_no_entities() -> Vec<u8> {
    let scene = Scene::new("t");
    let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
    assert_eq!(&bytes[0..8], b"WSCN0003");
    // Drop entity block: magic + clear + entity count 0.
    bytes[0..16].to_vec()
}

#[test]
fn corrupt_tail_huge_u16_count_does_not_reserve_megabytes() {
    let mut bytes = minimal_wscn_no_entities();
    bytes.extend_from_slice(&0u16.to_le_bytes()); // empty audio table
    // Animator tail: cell_n = 0xFFFF with no payload → Truncated on first read_str.
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    let err = parse_wscn(&bytes).unwrap_err();
    assert!(matches!(err, ParseError::Truncated));
}

#[test]
fn corrupt_tail_truncated_controller_table() {
    let mut bytes = minimal_wscn_no_entities();
    bytes.extend_from_slice(&0u16.to_le_bytes()); // empty audio table
    bytes.extend_from_slice(&0u16.to_le_bytes()); // cell_n
    bytes.extend_from_slice(&1u16.to_le_bytes()); // controller count
    bytes.push(0); // truncated mid-controller
    let err = parse_wscn(&bytes).unwrap_err();
    assert!(matches!(err, ParseError::Truncated));
}

#[test]
fn reject_non_finite_timeline_duration() {
    let mut scene = Scene::new("t");
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
    wiimaker_scene::add_component_playable_director(&mut scene, "Cut", "intro", false, false)
        .unwrap();

    let dir = std::env::temp_dir().join(format!(
        "wiimaker-wscn-nan-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    wiimaker_assets::write_timeline(
        &dir.join("assets"),
        "intro",
        wiimaker_assets::TimelineMeta {
            duration: 1.0,
            tracks: vec![],
        },
    )
    .unwrap();
    let timelines = wiimaker_assets::TimelineCatalog::load_dir(&dir.join("assets")).unwrap();
    let mut bytes = bake_scene_wscn_with_all_catalogs(
        &scene,
        &WPack::new(),
        None,
        None,
        None,
        Some(&timelines),
    )
    .unwrap();

    let name = b"intro";
    let name_pos = bytes
        .windows(name.len())
        .position(|w| w == name)
        .expect("timeline name in tail");
    let dur_off = name_pos + name.len();
    bytes[dur_off..dur_off + 4].copy_from_slice(&f32::to_bits(f32::NAN).to_le_bytes());

    let err = parse_wscn(&bytes).unwrap_err();
    assert!(matches!(err, ParseError::BadValue));
}

#[test]
fn parse_wscn0002_with_audio_entity() {
    let mut pack = WPack::new();
    pack.audio.push(wiimaker_assets::PackedAudio {
        name: "beep".into(),
        sample_rate: 8000,
        channels: 1,
        pcm: vec![0i16; 16],
    });
    let mut scene = Scene::new("t");
    add_entity(
        &mut scene,
        "Sfx",
        &MutateOpts {
            x: Some(1.0),
            y: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_audio_source(&mut scene, "Sfx", "beep", 0.75, false).unwrap();
    let mut bytes = bake_scene_wscn(&scene, &pack).unwrap();
    bytes[0..8].copy_from_slice(b"WSCN0002");
    let loaded = parse_wscn(&bytes).expect("WSCN0002 audio");
    let id = loaded.world.find_by_name("Sfx").unwrap();
    let audio = loaded.world.audio_source(id).expect("audio");
    assert_eq!(audio.clip, "0");
    assert!((audio.volume - 0.75).abs() < 1e-4);
    assert!(!audio.play_on_awake);
}
