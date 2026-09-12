//! Prefab instance links, apply-to-asset, revert, unpack, override detection.

use std::fs;
use std::path::PathBuf;

use wiimaker_scene::{
    add_entity, apply_prefab, entity_to_prefab, instantiate_prefab, load_prefab, load_scene,
    prefab_overrides, revert_prefab_instance, save_prefab, save_scene, set_entity_transform,
    unpack_prefab_instance, MutateOpts, Scene,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-prefab-overrides-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn prefab_instantiate_unpack_apply_revert_roundtrip() {
    let dir = tmp_game("roundtrip");
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

    let mut prefab = entity_to_prefab(&scene, "Ghost").unwrap();
    let asset = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&asset, &prefab).unwrap();

    let inst = instantiate_prefab(
        &mut scene,
        &prefab,
        "assets/prefabs/ghost.prefab.json",
        Some(120.0),
        Some(130.0),
    );
    let scene_path = dir.join("scenes/main.scene.json");
    save_scene(&scene_path, &scene).unwrap();

    let loaded = load_scene(&scene_path).unwrap();
    let e = loaded.find_entity(&inst).unwrap();
    assert_eq!(
        e.prefab.as_deref(),
        Some("assets/prefabs/ghost.prefab.json")
    );
    let ov = prefab_overrides(e, &prefab.entity);
    assert!(ov.contains("transform.position"), "{ov:?}");
    assert!(!ov.contains("Disc.radius"));

    unpack_prefab_instance(&mut scene, &inst).unwrap();
    assert!(scene.find_entity(&inst).unwrap().prefab.is_none());
    save_scene(&scene_path, &scene).unwrap();
    let loaded = load_scene(&scene_path).unwrap();
    assert!(loaded.find_entity(&inst).unwrap().prefab.is_none());
    assert_eq!(
        loaded.find_entity(&inst).unwrap().transform.translation[0],
        120.0
    );

    // Re-link via instantiate of a second copy, then apply + revert.
    let inst2 = instantiate_prefab(&mut scene, &prefab, "ghost", Some(200.0), Some(210.0));
    set_entity_transform(&mut scene, &inst2, Some(300.0), Some(310.0)).unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst2)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 22.0;

    apply_prefab(&mut scene, &inst2, &mut prefab, "ghost").unwrap();
    save_prefab(&asset, &prefab).unwrap();
    let from_disk = load_prefab(&asset).unwrap();
    assert!((from_disk.entity.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!((from_disk.entity.components.disc.as_ref().unwrap().radius - 22.0).abs() < 1e-4);

    set_entity_transform(&mut scene, &inst2, Some(1.0), Some(2.0)).unwrap();
    revert_prefab_instance(&mut scene, &inst2, &from_disk).unwrap();
    let e = scene.find_entity(&inst2).unwrap();
    assert!((e.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!((e.components.disc.as_ref().unwrap().radius - 22.0).abs() < 1e-4);
    assert_eq!(
        e.prefab.as_deref(),
        Some("assets/prefabs/ghost.prefab.json")
    );
}
