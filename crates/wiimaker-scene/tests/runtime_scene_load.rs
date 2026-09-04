//! Runtime scene switch: Build Settings list + load_scene_into_world replaces World.

use std::fs;
use std::path::PathBuf;

use wiimaker_core::world::World;
use wiimaker_scene::{
    add_build_scene, add_entity, diagnose, list_build_scenes, load_project, load_scene_into_world,
    save_project, save_scene, set_default_scene, GameProject, MutateOpts, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-runtime-scene-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn write_named_scene(dir: &PathBuf, stem: &str, entity: &str) {
    let mut scene = wiimaker_scene::Scene::new(stem);
    add_entity(
        &mut scene,
        entity,
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            ..Default::default()
        },
    )
    .unwrap();
    save_scene(
        &dir.join(format!("scenes/{stem}.scene.json")),
        &scene,
    )
    .unwrap();
}

#[test]
fn load_scene_into_world_replaces_entities() {
    let dir = tmp_game("hydrate");
    write_named_scene(&dir, "menu", "Title");
    write_named_scene(&dir, "main", "Player");
    write_named_scene(&dir, "win", "WinLabel");

    let mut project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();

    add_build_scene(&dir, "menu").unwrap();
    add_build_scene(&dir, "main").unwrap();
    add_build_scene(&dir, "win").unwrap();
    set_default_scene(&dir, "menu").unwrap();
    project = load_project(&dir).unwrap();

    assert_eq!(
        list_build_scenes(&dir).unwrap(),
        vec![
            PathBuf::from("scenes/menu.scene.json"),
            PathBuf::from("scenes/main.scene.json"),
            PathBuf::from("scenes/win.scene.json"),
        ]
    );
    assert_eq!(project.default_scene, "scenes/menu.scene.json");

    let textures = TextureMap::new();
    let mut world = World::new();
    let clear_a = load_scene_into_world(
        &mut world,
        &dir,
        &project,
        "menu",
        &textures,
        None,
        None,
    )
    .unwrap();
    assert_eq!(clear_a.a, 255);
    assert!(world.find_by_name("Title").is_some());
    assert!(world.find_by_name("Player").is_none());
    assert_eq!(world.iter_entities().count(), 1);

    let _clear_b = load_scene_into_world(
        &mut world,
        &dir,
        &project,
        "scenes/main.scene.json",
        &textures,
        None,
        None,
    )
    .unwrap();
    assert!(world.find_by_name("Title").is_none());
    assert!(world.find_by_name("Player").is_some());
    assert_eq!(world.iter_entities().count(), 1);

    let _clear_c = load_scene_into_world(&mut world, &dir, &project, "", &textures, None, None)
        .unwrap();
    assert!(world.find_by_name("Title").is_some());
}

#[test]
fn doctor_warns_default_not_in_build_list_and_missing_file() {
    let dir = tmp_game("doctor");
    write_named_scene(&dir, "main", "Player");
    write_named_scene(&dir, "menu", "Title");

    let mut project = GameProject::new("probe");
    project.default_scene = "scenes/main.scene.json".into();
    project.scenes = vec![
        "scenes/menu.scene.json".into(),
        "scenes/gone.scene.json".into(),
    ];
    save_project(&dir, &project).unwrap();

    let diag = diagnose(&dir, &project);
    assert!(diag.ok, "build-list issues are warnings, not errors");
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("default_scene") && m.contains("build list")),
        "expected default-not-in-list warning, got {msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("build scene missing") && m.contains("gone.scene.json")),
        "expected missing build scene warning, got {msgs:?}"
    );
}
