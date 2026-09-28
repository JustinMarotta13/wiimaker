//! Wii Rust staticlib — WSCN → World + `render_world` + GX FFI.
//!
//! Linked by `runtime/wii/Makefile` when
//! `target/powerpc-unknown-eabi/release/libwiimaker_wii.a` exists
//! (`USE_STUB=0`). Embeds still come from objcopy (`assets_wpack.o` /
//! `scene_wscn.o`); this crate `extern`s `_binary_*` like `stub_game.c`.
//!
//! Host builds keep `std` (default) for unit tests. Cross builds:
//! `cargo build -p wiimaker-wii --target powerpc-unknown-eabi --release --no-default-features`
//! (see `tools/wii-rustlib.sh`).

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
mod wii_alloc;

pub mod abi;
pub mod draw;
pub mod ffi;
pub mod input_map;
pub mod play;
pub mod wscn;

pub use ffi::WiimakerInput;
pub use input_map::{wiimaker_input_snapshot, wiimaker_input_to_core};
pub use play::GameState;
pub use wscn::{
    parse_wscn, LoadedScene, ParseError, AUDIO_NO_CLIP, KIND_AUDIO, KIND_DISC, KIND_SPRITE,
    KIND_TEXT, KIND_TILEMAP,
};

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::{install_test_backend, take_test_log, GxCall, WiimakerInput};
    use wiimaker_assets::WPack;
    use wiimaker_core::draw::{DrawCmd, DrawList, TextureId};
    use wiimaker_core::render::render_world;
    use wiimaker_core::wiimote_map;
    use wiimaker_core::Rgba8;
    use wiimaker_scene::{
        add_component_audio_source, add_component_disc, add_component_sprite, add_component_text,
        add_component_tilemap, add_entity, bake_scene_wscn, tilemap_stamp_ascii, MutateOpts, Scene,
        SceneTextAlign,
    };

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

    fn fixture_scene() -> (Scene, WPack) {
        let mut scene = Scene::new("t");
        scene.clear_color = [10, 20, 30, 255];
        add_entity(
            &mut scene,
            "Hero",
            &MutateOpts {
                x: Some(100.0),
                y: Some(200.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_sprite(&mut scene, "Hero", "sheet", [16.0, 24.0]).unwrap();
        add_entity(
            &mut scene,
            "Orb",
            &MutateOpts {
                x: Some(50.0),
                y: Some(60.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_disc(&mut scene, "Orb", 12.0, [255, 200, 80, 255]).unwrap();
        add_entity(
            &mut scene,
            "Hud",
            &MutateOpts {
                x: Some(8.0),
                y: Some(8.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_text(
            &mut scene,
            "Hud",
            "Hi",
            8.0,
            [255, 255, 255, 255],
            SceneTextAlign::Left,
        )
        .unwrap();
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_tilemap(&mut scene, "Maze", 3, 1, 16.0).unwrap();
        tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "###").unwrap();
        (scene, dummy_pack("sheet"))
    }

    #[test]
    fn parse_bake_roundtrip_sprite_disc_text() {
        let (scene, pack) = fixture_scene();
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        let loaded = parse_wscn(&bytes).expect("parse");
        assert_eq!(loaded.clear, [10, 20, 30, 255]);
        let hero = loaded.world.find_by_name("Hero").expect("Hero");
        let xf = loaded.world.transform(hero).unwrap();
        assert!((xf.translation.x - 100.0).abs() < 1e-4);
        let sp = loaded.world.sprite(hero).expect("sprite");
        assert_eq!(sp.texture, TextureId(0));
        assert!((sp.size.x - 16.0).abs() < 1e-4);
        let orb = loaded.world.find_by_name("Orb").expect("Orb");
        let d = loaded.world.disc(orb).expect("disc");
        assert!((d.radius - 12.0).abs() < 1e-4);
        let hud = loaded.world.find_by_name("Hud").expect("Hud");
        assert_eq!(loaded.world.text(hud).unwrap().string, "Hi");
        let maze = loaded.world.find_by_name("Maze").expect("Maze");
        let tm = loaded.world.tilemap(maze).expect("tilemap");
        assert_eq!(tm.get(0, 0), 1);
        assert_eq!(tm.get(1, 0), 1);
        assert_eq!(tm.get(2, 0), 1);
    }

    #[test]
    fn wscn_world_render_world_emits_ir() {
        let (scene, pack) = fixture_scene();
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        let loaded = parse_wscn(&bytes).expect("parse");
        let mut draw = DrawList::new();
        render_world(&loaded.world, &mut draw, loaded.clear_rgba());

        let mut saw_clear = false;
        let mut saw_sprite = false;
        let mut saw_disc = false;
        let mut saw_text = false;
        let mut tile_cells = 0usize;
        for cmd in draw.cmds() {
            match cmd {
                DrawCmd::Clear { color } => {
                    saw_clear = true;
                    assert_eq!(*color, Rgba8::new(10, 20, 30, 255));
                }
                DrawCmd::DrawSprite { texture, dest, .. } => {
                    if texture.is_untextured() {
                        tile_cells += 1;
                        assert!((dest.w - 16.0).abs() < 1e-4);
                    } else {
                        saw_sprite = true;
                        assert_eq!(*texture, TextureId(0));
                        // pivot 0.5, size 16×24 at (100, 200) → dest (92, 188)
                        assert!((dest.x - 92.0).abs() < 1e-3);
                        assert!((dest.y - 188.0).abs() < 1e-3);
                    }
                }
                DrawCmd::DrawDisc {
                    center, radius, ..
                } => {
                    saw_disc = true;
                    assert!((center.x - 50.0).abs() < 1e-3);
                    assert!((center.y - 60.0).abs() < 1e-3);
                    assert!((radius - 12.0).abs() < 1e-3);
                }
                DrawCmd::DrawText { pos, text, size, .. } => {
                    saw_text = true;
                    assert_eq!(text, "Hi");
                    assert!((pos.x - 8.0).abs() < 1e-3);
                    assert!((pos.y - 8.0).abs() < 1e-3);
                    assert!((size - 8.0).abs() < 1e-3);
                }
                _ => {}
            }
        }
        assert!(saw_clear);
        assert!(saw_sprite);
        assert!(saw_disc);
        assert!(saw_text);
        assert_eq!(tile_cells, 3);
    }

    #[test]
    fn draw_flush_emits_gx_calls() {
        install_test_backend();
        let mut scene = Scene::new("t");
        scene.clear_color = [1, 2, 3, 255];
        add_entity(
            &mut scene,
            "Player",
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_disc(&mut scene, "Player", 36.0, [72, 210, 160, 255]).unwrap();
        add_entity(
            &mut scene,
            "Label",
            &MutateOpts {
                x: Some(10.0),
                y: Some(10.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_text(
            &mut scene,
            "Label",
            "OK",
            8.0,
            [255, 255, 255, 255],
            SceneTextAlign::Left,
        )
        .unwrap();

        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        let mut g = GameState::init_from_bytes(&[0u8; 4], &bytes, 640, 480).unwrap();
        let raw = WiimakerInput::default();
        let _ = g.frame(&raw, 1.0 / 60.0);
        let log = take_test_log();

        assert!(log.iter().any(|c| matches!(
            c,
            GxCall::SetClear {
                r: 1,
                g: 2,
                b: 3,
                a: 255
            }
        )));
        assert!(log
            .iter()
            .any(|c| matches!(c, GxCall::DrawDisc { radius, .. } if (*radius - 36.0).abs() < 0.1)));
        assert!(log
            .iter()
            .any(|c| matches!(c, GxCall::DrawText { text, .. } if text == "OK")));
        assert!(log.iter().any(|c| matches!(c, GxCall::TexLoad { .. })));
        assert!(log.iter().any(|c| matches!(c, GxCall::AudioLoad { .. })));
    }

    #[test]
    fn input_map_drives_player_motion() {
        install_test_backend();
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Player",
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_disc(&mut scene, "Player", 36.0, [72, 210, 160, 255]).unwrap();
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        let mut g = GameState::init_from_bytes(&[], &bytes, 640, 480).unwrap();
        let pid = g.player.expect("Player");
        let x0 = g.world.transform(pid).unwrap().translation.x;
        let raw = WiimakerInput {
            main_x: 1.0,
            main_y: 0.0,
            ..Default::default()
        };
        let _ = g.frame(&raw, 0.1);
        let _ = take_test_log();
        let x1 = g.world.transform(pid).unwrap().translation.x;
        assert!(x1 > x0);
        assert!(g.input.main.x > 0.5);
    }

    #[test]
    fn start_button_requests_quit() {
        install_test_backend();
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "X",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        let bytes = bake_scene_wscn(&scene, &WPack::new()).unwrap();
        let mut g = GameState::init_from_bytes(&[], &bytes, 640, 480).unwrap();
        let raw = WiimakerInput {
            buttons: wiimote_map::BTN_START,
            ..Default::default()
        };
        assert!(g.frame(&raw, 0.016));
        let _ = take_test_log();
    }

    #[test]
    fn play_on_awake_queues_asnd() {
        install_test_backend();
        let mut pack = WPack::new();
        pack.audio.push(wiimaker_assets::PackedAudio {
            name: "beep".into(),
            sample_rate: 8000,
            channels: 1,
            pcm: vec![0i16; 32],
        });
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Sfx",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_audio_source(&mut scene, "Sfx", "beep", 0.5, true).unwrap();
        let bytes = bake_scene_wscn(&scene, &pack).unwrap();
        let _g = GameState::init_from_bytes(&pack_bytes_unused(), &bytes, 640, 480).unwrap();
        let log = take_test_log();
        assert!(log
            .iter()
            .any(|c| matches!(c, GxCall::AudioQueue { clip_id: 0, volume } if (*volume - 0.5).abs() < 1e-4)));
        assert!(log.iter().any(|c| matches!(c, GxCall::AudioFlush)));
    }

    fn pack_bytes_unused() -> &'static [u8] {
        &[0u8; 4]
    }
}
