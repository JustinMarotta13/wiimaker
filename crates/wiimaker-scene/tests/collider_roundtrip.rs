//! CLI-shaped round-trip: add Collider → save → load → hydrate → overlaps / move_and_collide.

use std::path::PathBuf;

use wiimaker_core::math::Vec2;
use wiimaker_core::{move_and_collide, overlaps};
use wiimaker_scene::{
    add_component_collider, add_entity, entities_overlap, entity_overlaps, entity_triggers_entered,
    hydrate, load_scene, save_scene, MutateOpts, SceneColliderKind, TextureMap,
};

fn tmp() -> PathBuf {
    let dir = std::env::temp_dir().join("wiimaker-collider-roundtrip");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("walls.scene.json")
}

#[test]
fn json_file_roundtrip_add_overlap_move() {
    let mut scene = wiimaker_scene::Scene::new("walls");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            radius: Some(6.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Wall",
        &MutateOpts {
            x: Some(40.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_collider(
        &mut scene,
        "Player",
        SceneColliderKind::Aabb,
        [12.0, 12.0],
        6.0,
        true,
        false,
        0,
    )
    .unwrap();
    add_component_collider(
        &mut scene,
        "Wall",
        SceneColliderKind::Aabb,
        [16.0, 48.0],
        8.0,
        true,
        false,
        0,
    )
    .unwrap();

    let path = tmp();
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    let c = loaded
        .find_entity("Wall")
        .unwrap()
        .components
        .collider
        .as_ref()
        .unwrap();
    assert_eq!(c.kind, SceneColliderKind::Aabb);
    assert_eq!(c.size, [16.0, 48.0]);
    assert!(c.solid);

    assert!(!entities_overlap(&loaded, "Player", "Wall").unwrap());
    assert!(entity_overlaps(&loaded, "Player").unwrap().is_empty());

    let mut world = hydrate(&loaded, &TextureMap::new()).unwrap();
    let player = world.find_by_name("Player").unwrap();
    let wall = world.find_by_name("Wall").unwrap();
    assert!(!overlaps(&world, player, wall));
    let hit = move_and_collide(&mut world, player, Vec2::new(40.0, 0.0));
    assert_eq!(hit.hit, Some(wall));
    let x = world.transform(player).unwrap().translation.x;
    assert!(x > 20.0 && x < 26.1, "expected contact near 26, got {x}");
    assert!(!overlaps(&world, player, wall));
}

#[test]
fn trigger_roundtrip_pass_through_and_entered() {
    let mut scene = wiimaker_scene::Scene::new("dots");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Dot",
        &MutateOpts {
            x: Some(20.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_collider(
        &mut scene,
        "Player",
        SceneColliderKind::Aabb,
        [12.0, 12.0],
        6.0,
        true,
        false,
        0,
    )
    .unwrap();
    add_component_collider(
        &mut scene,
        "Dot",
        SceneColliderKind::Circle,
        [0.0, 0.0],
        8.0,
        false,
        true,
        0,
    )
    .unwrap();

    let path = tmp().with_file_name("dots.scene.json");
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    let c = loaded
        .find_entity("Dot")
        .unwrap()
        .components
        .collider
        .as_ref()
        .unwrap();
    assert!(c.trigger);
    assert_eq!(c.filter_tag, 0);

    let mut world = hydrate(&loaded, &TextureMap::new()).unwrap();
    let player = world.find_by_name("Player").unwrap();
    let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 0.0));
    assert!(hit.hit.is_none());

    let mut close = loaded.clone();
    close.entities[0].transform.translation[0] = 20.0;
    assert_eq!(
        entity_triggers_entered(&close, "Player").unwrap(),
        vec!["Dot".to_string()]
    );
}
