//! CLI twin: `entity apply-prefab --to-base`, `open-base`, `prefab-status --json`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use wiimaker_scene::{
    add_entity, create_prefab_variant, entity_to_prefab, instantiate_prefab,
    refresh_variant_overrides, resolve_prefab, save_prefab, save_project, save_scene, GameProject,
    MutateOpts, Scene,
};

fn tmp_game() -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-apply-base-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-prefab");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();

    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost.prefab.json"), &base).unwrap();
    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    variant.entity.components.disc.as_mut().unwrap().color = [0, 0, 255, 255];
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    save_prefab(&variant_path, &variant).unwrap();
    let resolved = resolve_prefab(&dir, &variant).unwrap();
    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_v.prefab.json",
        Some(40.0),
        Some(50.0),
    );
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 21.0;
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    (dir, inst)
}

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn apply_prefab_to_base_open_base_and_status_json() {
    let (dir, inst) = tmp_game();
    let game = dir.to_str().unwrap();

    let status = wiimaker()
        .args(["entity", "prefab-status", game, "--name", &inst, "--json"])
        .output()
        .expect("prefab-status");
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status_out = String::from_utf8_lossy(&status.stdout);
    assert!(status_out.contains("\"variant\":true"), "{status_out}");
    assert!(
        status_out.contains("assets/prefabs/ghost.prefab.json"),
        "{status_out}"
    );
    assert!(status_out.contains("\"chain\""), "{status_out}");
    assert!(
        status_out.contains("Disc.color"),
        "variant overrides: {status_out}"
    );
    assert!(
        status_out.contains("Disc.radius"),
        "instance overrides: {status_out}"
    );
    assert!(status_out.contains("variant_overrides"), "{status_out}");
    assert!(status_out.contains("base_overrides"), "{status_out}");

    let open = wiimaker()
        .args(["entity", "open-base", game, "--name", &inst, "--json"])
        .output()
        .expect("open-base");
    assert!(
        open.status.success(),
        "{}",
        String::from_utf8_lossy(&open.stderr)
    );
    let open_out = String::from_utf8_lossy(&open.stdout);
    assert!(
        open_out.contains("\"target\":\"assets/prefabs/ghost.prefab.json\"")
            || open_out.contains("\"target\": \"assets/prefabs/ghost.prefab.json\""),
        "{open_out}"
    );
    assert!(open_out.contains("ghost_v.prefab.json"), "{open_out}");

    let open_asset = wiimaker()
        .args(["entity", "open-base", game, "--prefab", "ghost_v", "--json"])
        .output()
        .expect("open-base asset");
    assert!(
        open_asset.status.success(),
        "{}",
        String::from_utf8_lossy(&open_asset.stderr)
    );

    let apply = wiimaker()
        .args([
            "entity",
            "apply-prefab",
            game,
            "--name",
            &inst,
            "--to-base",
            "--field",
            "Disc.radius",
            "--json",
        ])
        .output()
        .expect("apply-prefab --to-base");
    assert!(
        apply.status.success(),
        "apply: {}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let apply_out = String::from_utf8_lossy(&apply.stdout);
    assert!(apply_out.contains("Disc.radius"), "{apply_out}");
    assert!(apply_out.contains("ghost.prefab.json"), "{apply_out}");

    let base = wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    let radius = base.entity.components.disc.as_ref().unwrap().radius;
    assert!((radius - 21.0).abs() < 1e-3, "{radius}");

    let variant =
        wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost_v.prefab.json")).unwrap();
    assert!(
        variant.overrides.iter().all(|v| v != "Disc.radius"),
        "{:?}",
        variant.overrides
    );
    assert!(
        variant.overrides.iter().any(|v| v == "Disc.color"),
        "color override stays on the variant: {:?}",
        variant.overrides
    );

    let apply_asset = wiimaker()
        .args([
            "entity",
            "apply-prefab",
            game,
            "--prefab",
            "ghost_v",
            "--to-base",
            "--field",
            "Disc.color",
            "--json",
        ])
        .output()
        .expect("apply-prefab --prefab --to-base");
    assert!(
        apply_asset.status.success(),
        "apply --prefab: {}",
        String::from_utf8_lossy(&apply_asset.stderr)
    );
    let apply_asset_out = String::from_utf8_lossy(&apply_asset.stdout);
    assert!(apply_asset_out.contains("Disc.color"), "{apply_asset_out}");

    let base_color =
        wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    assert_eq!(
        base_color.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );
    let variant_after =
        wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost_v.prefab.json")).unwrap();
    assert!(
        variant_after.overrides.iter().all(|v| v != "Disc.color"),
        "{:?}",
        variant_after.overrides
    );
}
