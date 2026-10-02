//! Prefab variants: create, instantiate (base+overrides), apply/revert to variant asset.

use std::fs;
use std::path::PathBuf;

use wiimaker_scene::{
    add_entity, apply_prefab, create_prefab_variant, entity_to_prefab, instantiate_prefab,
    load_prefab, load_prefab_for_instance, normalize_prefab_source, refresh_variant_overrides,
    resolve_prefab, revert_prefab_instance, save_prefab, set_entity_transform,
    unpack_prefab_instance, variant_from_instance, MutateOpts, Scene,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-prefab-variants-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn create_variant_instantiate_applies_base_plus_overrides() {
    let dir = tmp_game("create-inst");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(40.0),
            y: Some(50.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    // Paint base red.
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
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();

    let (variant_path, mut variant) =
        create_prefab_variant(&dir, "ghost", "ghost_blue").unwrap();
    assert!(variant.is_variant());
    assert_eq!(
        variant.base.as_deref(),
        Some("assets/prefabs/ghost.prefab.json")
    );
    assert!(variant.overrides.is_empty());

    // Override color on the variant asset.
    variant.entity.components.disc.as_mut().unwrap().color = [0, 0, 255, 255];
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    assert!(
        variant.overrides.iter().any(|f| f == "Disc.color"),
        "{:?}",
        variant.overrides
    );
    save_prefab(&variant_path, &variant).unwrap();

    // Change base radius — non-overridden field should inherit on resolve.
    let mut base = load_prefab(&base_path).unwrap();
    base.entity.components.disc.as_mut().unwrap().radius = 14.0;
    save_prefab(&base_path, &base).unwrap();

    let raw = load_prefab(&variant_path).unwrap();
    let resolved = resolve_prefab(&dir, &raw).unwrap();
    assert!((resolved.entity.components.disc.as_ref().unwrap().radius - 14.0).abs() < 1e-4);
    assert_eq!(
        resolved.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );

    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_blue.prefab.json",
        Some(200.0),
        Some(210.0),
    );
    let e = scene.find_entity(&inst).unwrap();
    assert_eq!(
        e.prefab.as_deref(),
        Some("assets/prefabs/ghost_blue.prefab.json")
    );
    assert!((e.components.disc.as_ref().unwrap().radius - 14.0).abs() < 1e-4);
    assert_eq!(e.components.disc.as_ref().unwrap().color, [0, 0, 255, 255]);
}

#[test]
fn apply_revert_writes_variant_not_base() {
    let dir = tmp_game("apply-revert");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();

    let (variant_path, variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    let resolved = resolve_prefab(&dir, &variant).unwrap();
    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_v.prefab.json",
        Some(100.0),
        Some(110.0),
    );

    set_entity_transform(&mut scene, &inst, Some(300.0), Some(310.0)).unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 22.0;

    let mut pf = load_prefab(&variant_path).unwrap();
    apply_prefab(&mut scene, &inst, &mut pf, "ghost_v").unwrap();
    refresh_variant_overrides(&dir, &mut pf).unwrap();
    save_prefab(&variant_path, &pf).unwrap();

    let base_after = load_prefab(&base_path).unwrap();
    assert!((base_after.entity.transform.translation[0] - 10.0).abs() < 1e-4);
    assert!((base_after.entity.components.disc.as_ref().unwrap().radius - 8.0).abs() < 1e-4);

    let variant_after = load_prefab(&variant_path).unwrap();
    assert!(variant_after.is_variant());
    assert!((variant_after.entity.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!((variant_after.entity.components.disc.as_ref().unwrap().radius - 22.0).abs() < 1e-4);
    assert!(variant_after.overrides.iter().any(|f| f == "transform.position"));
    assert!(variant_after.overrides.iter().any(|f| f == "Disc.radius"));

    set_entity_transform(&mut scene, &inst, Some(1.0), Some(2.0)).unwrap();
    let resolved = load_prefab_for_instance(
        &dir,
        scene.find_entity(&inst).unwrap(),
    )
    .unwrap();
    revert_prefab_instance(&mut scene, &inst, &resolved).unwrap();
    let e = scene.find_entity(&inst).unwrap();
    assert!((e.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!((e.components.disc.as_ref().unwrap().radius - 22.0).abs() < 1e-4);
}

#[test]
fn variant_from_instance_and_old_prefab_compat() {
    let dir = tmp_game("from-inst");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(5.0),
            y: Some(6.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost.prefab.json"), &base).unwrap();

    let inst = instantiate_prefab(
        &mut scene,
        &base,
        "assets/prefabs/ghost.prefab.json",
        Some(50.0),
        Some(60.0),
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
        .color = [0, 255, 0, 255];

    let variant = variant_from_instance(&dir, &scene, &inst).unwrap();
    assert!(variant.is_variant());
    assert!(variant.overrides.iter().any(|f| f == "Disc.color")
        || variant.overrides.iter().any(|f| f == "transform.position"));
    let dest = dir.join("assets/prefabs/ghost_green.prefab.json");
    save_prefab(&dest, &variant).unwrap();

    // Old one-entity prefab still loads.
    let old = r#"{"name":"Plain","transform":{"translation":[0.0,0.0,0.0]},"components":{},"tag":0}"#;
    fs::write(dir.join("assets/prefabs/plain.prefab.json"), old).unwrap();
    let plain = load_prefab(&dir.join("assets/prefabs/plain.prefab.json")).unwrap();
    assert!(!plain.is_variant());
    assert!(plain.children.is_empty());

    unpack_prefab_instance(&mut scene, &inst).unwrap();
    assert!(scene.find_entity(&inst).unwrap().prefab.is_none());
}

#[test]
fn normalize_variant_link() {
    assert_eq!(
        normalize_prefab_source("ghost_blue"),
        "assets/prefabs/ghost_blue.prefab.json"
    );
}
