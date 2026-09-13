//! When `games/hello-orb` has been built (`--lib`), prove Play ticks its `App`.
//! Skips if the dylib is absent (CI without a local hello-orb checkout).

use std::path::PathBuf;

use wiimaker_core::input::Input;
use wiimaker_play::{apply_pad_keys, dylib_path, LoadedPlugin, PadKeys};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace")
}

#[test]
fn hello_orb_plugin_app_moves_player() {
    let root = workspace_root();
    let game_dir = root.join("games/hello-orb");
    let dylib = dylib_path(&root, "hello-orb");
    if !dylib.is_file() || !game_dir.join("game.toml").is_file() {
        eprintln!("skip: hello-orb plugin not built ({})", dylib.display());
        return;
    }

    let mut plugin = LoadedPlugin::load(&dylib, &game_dir, None).expect("load hello-orb plugin");
    let player = plugin
        .world()
        .and_then(|w| w.find_by_name("Player"))
        .expect("template hello-orb scene has Player");
    let x0 = plugin
        .world()
        .unwrap()
        .transform(player)
        .unwrap()
        .translation
        .x;

    let mut input = Input::new();
    input.begin_frame();
    apply_pad_keys(
        &mut input,
        PadKeys {
            right: true,
            ..Default::default()
        },
    );
    // ~0.05s at 60 Hz ≈ 3 App::update calls inside a session; here one step each.
    for _ in 0..3 {
        plugin.update(&input, 1.0 / 60.0, 640, 480).unwrap();
    }

    let x1 = plugin
        .world()
        .and_then(|w| w.transform(player))
        .unwrap()
        .translation
        .x;
    assert!(
        x1 > x0 + 1.0,
        "hello-orb App must move Player (plugin path), {x0} → {x1}"
    );
}
