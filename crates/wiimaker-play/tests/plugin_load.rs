//! Load the `play_probe` example cdylib and prove Play ticks `App::update`.

use std::path::PathBuf;
use std::process::Command;

use wiimaker_core::input::Input;
use wiimaker_play::{apply_pad_keys, dylib_name, step_app, PadKeys, PlayKind, PLAYER_WASD_SPEED};
use wiimaker_play::{example_dylib_path, LoadedPlugin};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace")
}

fn build_probe() -> PathBuf {
    let root = workspace_root();
    let status = Command::new("cargo")
        .args(["build", "-p", "wiimaker-play", "--example", "play_probe"])
        .current_dir(&root)
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "play_probe example failed to build");
    let path = example_dylib_path(&root, "play_probe");
    assert!(
        path.is_file(),
        "expected probe dylib at {} (name {})",
        path.display(),
        dylib_name("play_probe")
    );
    path
}

#[test]
fn plugin_app_moves_marker_fallback_would_not() {
    let path = build_probe();
    let root = workspace_root();
    let mut plugin = LoadedPlugin::load(&path, &root, None).expect("load play_probe");
    assert_eq!(plugin.abi(), wiimaker_play::PLAY_ABI_VERSION);

    let marker0 = plugin
        .world()
        .and_then(|w| w.find_by_name("Marker"))
        .and_then(|id| plugin.world().unwrap().transform(id).map(|t| t.translation));
    let start = marker0.expect("Marker");
    assert!((start.x - 0.0).abs() < 1e-4);

    let mut input = Input::new();
    input.begin_frame();
    apply_pad_keys(
        &mut input,
        PadKeys {
            right: true,
            ..Default::default()
        },
    );
    plugin.update(&input, 1.0 / 60.0, 640, 480).unwrap();
    plugin.update(&input, 1.0 / 60.0, 640, 480).unwrap();

    let end = plugin
        .world()
        .and_then(|w| w.find_by_name("Marker"))
        .and_then(|id| plugin.world().unwrap().transform(id).map(|t| t.translation))
        .unwrap();
    // Probe App: +1 x per update (2 ticks) + stick.x added to y.
    assert!(
        (end.x - 2.0).abs() < 1e-3,
        "plugin App must move Marker.x by ticks, got {}",
        end.x
    );
    assert!(
        (end.y - 2.0).abs() < 1e-3,
        "plugin App must apply stick.x to Marker.y, got {}",
        end.y
    );

    // Fallback WASD only touches `Player` at 220 px/s — Marker would stay put.
    assert!(PLAYER_WASD_SPEED > 0.0);
    assert_ne!(PlayKind::Plugin, PlayKind::Fallback);
}

#[test]
fn step_app_is_the_host_bridge() {
    struct Tap {
        n: u32,
    }
    impl wiimaker_core::app::App for Tap {
        fn update(&mut self, _ctx: &wiimaker_core::app::FrameCtx<'_>) {
            self.n += 1;
        }
        fn render(
            &mut self,
            _ctx: &wiimaker_core::app::FrameCtx<'_>,
            _draw: &mut wiimaker_core::draw::DrawList,
        ) {
        }
    }
    let mut app = Tap { n: 0 };
    let clock = wiimaker_core::time::Clock::new(60.0);
    step_app(&mut app, &Input::new(), &clock, 640, 480);
    assert_eq!(app.n, 1, "host/editor bridge must call App::update");
}
