//! Build the `templates/basic-game` scaffold (what `wiimaker new` creates) as a `cdylib`
//! and prove Play ticks its `App` through the plugin path, not the WASD fallback.
//!
//! The scaffold is written under `target/template-plugin-test/` as a standalone crate
//! (path deps on the workspace crates, its own empty `[workspace]`), built into the
//! shared target dir, then loaded with `LoadedPlugin`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use wiimaker_core::input::Input;
use wiimaker_play::{apply_pad_keys, dylib_path, LoadedPlugin, PadKeys};

const NAME: &str = "template-game";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace")
}

/// Same rule as `wiimaker_play::plugin::target_dir` (`CARGO_TARGET_DIR` or `<root>/target`).
fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
}

fn copy_tree(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dest.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// Scaffold the template as a standalone crate and return its game dir.
fn scaffold_template(root: &Path) -> PathBuf {
    let dir = target_dir(root).join("template-plugin-test").join(NAME);
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&root.join("templates/basic-game"), &dir);
    for rel in [
        "Cargo.toml",
        "src/main.rs",
        "src/lib.rs",
        "src/game.rs",
        "game.toml",
        "scenes/main.scene.json",
    ] {
        let path = dir.join(rel);
        if path.is_file() {
            let text = fs::read_to_string(&path).unwrap().replace("{{name}}", NAME);
            fs::write(&path, text).unwrap();
        }
    }
    fs::create_dir_all(dir.join("assets")).unwrap();

    // Detach from the root workspace: inline package fields, path deps, empty [workspace].
    let crates = root.join("crates");
    let mut manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    manifest = manifest
        .replace("version.workspace = true", "version = \"0.0.0\"")
        .replace("edition.workspace = true", "edition = \"2021\"")
        .replace("license.workspace = true", "license = \"MIT\"");
    for dep in [
        "wiimaker-core",
        "wiimaker-host",
        "wiimaker-scene",
        "wiimaker-assets",
        "wiimaker-play",
    ] {
        manifest = manifest.replace(
            &format!("{dep} = {{ workspace = true }}"),
            &format!(
                "{dep} = {{ path = {:?} }}",
                crates.join(dep).display().to_string()
            ),
        );
    }
    assert!(
        !manifest.contains("workspace = true"),
        "template Cargo.toml has a workspace dep this test does not map:\n{manifest}"
    );
    manifest.push_str("\n[workspace]\n");
    fs::write(dir.join("Cargo.toml"), manifest).unwrap();
    // Pin the same dependency versions as the workspace.
    fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();
    dir
}

#[test]
fn template_plugin_app_moves_player() {
    let root = workspace_root();
    let game_dir = scaffold_template(&root);

    let status = Command::new(env!("CARGO"))
        .args(["build", "--lib", "--quiet"])
        .current_dir(&game_dir)
        .env("CARGO_TARGET_DIR", target_dir(&root))
        .status()
        .expect("spawn cargo build template plugin");
    assert!(status.success(), "template-game cdylib failed to build");
    let dylib = dylib_path(&root, NAME);
    assert!(dylib.is_file(), "missing template plugin at {}", dylib.display());

    let mut plugin =
        LoadedPlugin::load(&dylib, &game_dir, None).expect("load template-game plugin");
    let player = plugin
        .world()
        .and_then(|w| w.find_by_name("Player"))
        .expect("template scene has Player");
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
        "template App must move Player (plugin path), {x0} → {x1}"
    );
}
