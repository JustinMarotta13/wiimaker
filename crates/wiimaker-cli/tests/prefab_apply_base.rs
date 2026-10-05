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
}

#[test]
fn apply_prefab_flag_rejects_non_override_and_positional_still_works() {
    let (dir, inst) = tmp_game();
    let game = dir.to_str().unwrap();

    let flagged = wiimaker()
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
        .expect("apply-prefab --prefab");
    assert!(
        flagged.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&flagged.stderr)
    );
    let base = wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    assert_eq!(
        base.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );

    let rejected = wiimaker()
        .args([
            "entity",
            "apply-prefab",
            game,
            "--prefab",
            "ghost_v",
            "--to-base",
            "--field",
            "Disc.radius",
        ])
        .output()
        .expect("non-override field");
    assert!(
        !rejected.status.success(),
        "non-override path must fail: {}",
        String::from_utf8_lossy(&rejected.stdout)
    );
    let err = String::from_utf8_lossy(&rejected.stderr);
    assert!(err.contains("not set"), "{err}");
    let base = wiimaker_scene::load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    let radius = base.entity.components.disc.as_ref().unwrap().radius;
    assert!((radius - 8.0).abs() < 1e-3, "{radius}");

    let positional = wiimaker()
        .args(["entity", "apply-prefab", game, "--name", &inst, "ghost_v"])
        .output()
        .expect("positional prefab");
    assert!(
        positional.status.success(),
        "positional PREFAB: {}",
        String::from_utf8_lossy(&positional.stderr)
    );
}

#[test]
fn apply_prefab_to_base_updates_sibling_instances() {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-cli-sib-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    let mut project = GameProject::new("cli-sib");
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
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost.prefab.json"), &base).unwrap();
    create_prefab_variant(&dir, "ghost", "blinky").unwrap();
    create_prefab_variant(&dir, "ghost", "pinky").unwrap();
    let blinky_raw =
        wiimaker_scene::load_prefab(&dir.join("assets/prefabs/blinky.prefab.json")).unwrap();
    let pinky_raw =
        wiimaker_scene::load_prefab(&dir.join("assets/prefabs/pinky.prefab.json")).unwrap();
    let blinky_res = resolve_prefab(&dir, &blinky_raw).unwrap();
    let pinky_res = resolve_prefab(&dir, &pinky_raw).unwrap();
    let blinky = instantiate_prefab(
        &mut scene,
        &blinky_res,
        "assets/prefabs/blinky.prefab.json",
        None,
        None,
    );
    let pinky = instantiate_prefab(
        &mut scene,
        &pinky_res,
        "assets/prefabs/pinky.prefab.json",
        Some(40.0),
        None,
    );
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    let game = dir.to_str().unwrap();

    let set = wiimaker()
        .args([
            "entity", "set", game, "--name", &blinky, "--sx", "1.25", "--sy", "1.25",
        ])
        .output()
        .expect("entity set");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );

    let apply = wiimaker()
        .args([
            "entity",
            "apply-prefab",
            game,
            "--name",
            &blinky,
            "--to-base",
            "--field",
            "transform.scale",
            "--json",
        ])
        .output()
        .expect("apply to base");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );

    let scene = wiimaker_scene::load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let pinky_ent = scene.entities.iter().find(|e| e.name == pinky).unwrap();
    assert!(
        (pinky_ent.transform.scale[0] - 1.25).abs() < 1e-3,
        "{:?}",
        pinky_ent.transform.scale
    );
    let status = wiimaker()
        .args(["entity", "prefab-status", game, "--name", &pinky, "--json"])
        .output()
        .expect("prefab-status");
    assert!(status.status.success());
    let status_out = String::from_utf8_lossy(&status.stdout);
    assert!(
        !status_out.contains("transform.scale"),
        "sibling must not gain an instance override: {status_out}"
    );
}
