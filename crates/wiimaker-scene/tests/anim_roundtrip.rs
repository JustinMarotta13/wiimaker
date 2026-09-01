//! CLI-shaped round-trip: write clip, add Animation, save, load, hydrate, doctor.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{write_anim_clip, AnimClipCatalog};
use wiimaker_scene::{
    add_component_animation, add_entity, animate_world, diagnose, hydrate_with_catalogs,
    load_scene, save_project, save_scene, set_entity_anim, GameProject, MutateOpts, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-anim-roundtrip-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn json_file_roundtrip_add_set_hydrate() {
    let dir = tmp_game("hydrate");
    let assets = dir.join("assets");
    write_anim_clip(
        &assets,
        "chomp",
        vec!["cell_a".into(), "cell_b".into()],
        10.0,
        true,
    )
    .unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
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
    add_component_animation(&mut scene, "Player", "chomp", None, true).unwrap();

    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    let a = loaded
        .find_entity("Player")
        .unwrap()
        .components
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(a.clip, "chomp");
    assert!(a.loop_);
    assert!(a.fps.is_none());

    let mut edited = loaded;
    set_entity_anim(&mut edited, "Player", "chomp", Some(12.0), Some(false)).unwrap();
    save_scene(&path, &edited).unwrap();
    let reloaded = load_scene(&path).unwrap();
    let a = reloaded
        .find_entity("Player")
        .unwrap()
        .components
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(a.fps, Some(12.0));
    assert!(!a.loop_);

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let world = hydrate_with_catalogs(&reloaded, &TextureMap::new(), None, Some(&anims)).unwrap();
    let id = world.find_by_name("Player").unwrap();
    let anim = world.animation(id).unwrap();
    assert_eq!(anim.clip, "chomp");
    assert_eq!(anim.cells, vec!["cell_a", "cell_b"]);
    assert_eq!(anim.fps, 12.0);
    assert!(!anim.loop_);
    assert_eq!(anim.frame, 0);

    let mut world = world;
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        1.0,
    );
    assert_eq!(world.animation(id).unwrap().frame, 1);
}

#[test]
fn doctor_warns_missing_clip_cells() {
    let dir = tmp_game("cells");
    let assets = dir.join("assets");
    write_anim_clip(
        &assets,
        "chomp",
        vec!["missing_cell".into()],
        10.0,
        true,
    )
    .unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animation(&mut scene, "Player", "chomp", None, true).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let diag = diagnose(&dir, &project);
    assert!(diag.ok, "missing cells are warnings, not errors");
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("missing_cell") && m.contains("chomp")),
        "expected missing-cell warning, got {msgs:?}"
    );
}

#[test]
fn doctor_warns_missing_clip_file() {
    let dir = tmp_game("clip");
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animation(&mut scene, "Player", "no-such-clip", None, true).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let diag = diagnose(&dir, &project);
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("no-such-clip") && m.contains("anim.json")),
        "expected missing-clip warning, got {msgs:?}"
    );
}
