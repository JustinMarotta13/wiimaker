//! Wii Rust staticlib — WSCN subset player calling C GX / ASND.
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
pub use wscn::{parse_wscn, Entity, LoadedScene, ParseError, KIND_AUDIO, KIND_DISC, KIND_SPRITE, KIND_TEXT, KIND_TILEMAP};

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
    use wiimaker_core::wiimote_map;
    use wiimaker_scene::{
        add_component_disc, add_component_sprite, add_component_text, add_entity,
        bake_scene_wscn, MutateOpts, Scene, SceneTextAlign,
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

    #[test]
    fn parse_bake_roundtrip_sprite_disc_text() {
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
        add_component_text(&mut scene, "Hud", "Hi", 8.0, [255, 255, 255, 255], SceneTextAlign::Left).unwrap();

        let bytes = bake_scene_wscn(&scene, &dummy_pack("sheet")).unwrap();
        let loaded = parse_wscn(&bytes).expect("parse");
        assert_eq!(loaded.clear, [10, 20, 30, 255]);
        assert_eq!(loaded.entities.len(), 3);
        assert_eq!(loaded.entities[0].kind, KIND_SPRITE);
        assert_eq!(loaded.entities[0].name, "Hero");
        assert!((loaded.entities[0].x - 100.0).abs() < 1e-4);
        assert_eq!(loaded.entities[1].kind, KIND_DISC);
        assert!((loaded.entities[1].radius - 12.0).abs() < 1e-4);
        assert_eq!(loaded.entities[2].kind, KIND_TEXT);
        assert_eq!(loaded.entities[2].text, "Hi");
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
        add_component_text(&mut scene, "Label", "OK", 8.0, [255, 255, 255, 255], SceneTextAlign::Left).unwrap();

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
        let x0 = g.entities[0].x;
        let raw = WiimakerInput {
            main_x: 1.0,
            main_y: 0.0,
            ..Default::default()
        };
        let _ = g.frame(&raw, 0.1);
        let _ = take_test_log();
        assert!(g.entities[0].x > x0);
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
}
