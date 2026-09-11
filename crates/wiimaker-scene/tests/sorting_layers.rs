//! Sorting layers: game.toml list, scene assignment, hydrate, render, doctor.

use std::fs;
use std::path::PathBuf;

use wiimaker_core::draw::{DrawCmd, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::{Disc, Sprite, Transform};
use wiimaker_scene::{
    add_component_disc, add_component_sprite, add_entity, add_sorting_layer, diagnose, hydrate,
    list_sorting_layers, load_project, load_scene, move_sorting_layer, remove_sorting_layer,
    rename_sorting_layer, render_world, save_project, save_scene, set_entity_sorting, GameProject,
    MutateOpts, Scene, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-sorting-layers-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn draw_kinds(cmds: &[DrawCmd]) -> Vec<&'static str> {
    cmds.iter()
        .filter_map(|c| match c {
            DrawCmd::DrawSprite { .. } => Some("sprite"),
            DrawCmd::DrawDisc { .. } => Some("disc"),
            _ => None,
        })
        .collect()
}

#[test]
fn game_toml_defaults_then_roundtrip() {
    let dir = tmp_game("toml");
    let mut project = GameProject::new("sort-demo");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    let text = fs::read_to_string(dir.join("game.toml")).unwrap();
    assert!(
        !text.contains("sorting_layers"),
        "empty list should omit the key: {text}"
    );
    let listed = list_sorting_layers(&dir).unwrap();
    assert_eq!(listed, vec!["Background", "Default", "Foreground"]);

    add_sorting_layer(&dir, "UI", None).unwrap();
    let listed = list_sorting_layers(&dir).unwrap();
    assert_eq!(listed, vec!["Background", "Default", "Foreground", "UI"]);
    let text = fs::read_to_string(dir.join("game.toml")).unwrap();
    assert!(text.contains("sorting_layers"));
    assert!(text.contains("UI"));
}

#[test]
fn hydrate_save_unknown_is_default_and_render_order() {
    let dir = tmp_game("hydrate");
    let mut project = GameProject::new("sort-h");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();

    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "back",
        &MutateOpts {
            x: Some(10.0),
            y: Some(10.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "front",
        &MutateOpts {
            x: Some(40.0),
            y: Some(10.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_sprite(&mut scene, "front", "tex", [16.0, 16.0]).unwrap();
    set_entity_sorting(&mut scene, "back", Some("Background"), Some(99.0)).unwrap();
    set_entity_sorting(&mut scene, "front", Some("Foreground"), Some(0.0)).unwrap();

    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("Background"));
    assert!(text.contains("Foreground"));
    assert!(!text.contains("\"sorting_layer\": \"Default\""));

    let loaded = load_scene(&path).unwrap();
    let textures = TextureMap::from_names(["tex"]);
    let world = hydrate(&loaded, &textures).unwrap();
    let back = world.find_by_name("back").unwrap();
    let front = world.find_by_name("front").unwrap();
    assert_eq!(
        world.disc(back).unwrap().sorting_layer,
        world.sorting_layer_index("Background")
    );
    assert_eq!(
        world.sprite(front).unwrap().sorting_layer,
        world.sorting_layer_index("Foreground")
    );

    let mut draw = wiimaker_core::DrawList::new();
    render_world(&world, &mut draw, wiimaker_core::Rgba8::BLACK);
    assert_eq!(draw_kinds(draw.cmds()), vec!["disc", "sprite"]);
}

#[test]
fn old_scene_without_layers_still_draws() {
    let mut world = wiimaker_core::World::new();
    let t = world.spawn_named("tile", Transform::from_xy(0.0, 0.0));
    let mut tm = wiimaker_core::Tilemap::new(1, 1, 8.0);
    tm.set(0, 0, 1, false);
    world.set_tilemap(t, Some(tm));
    let s = world.spawn_named("spr", Transform::from_xy(20.0, 0.0));
    world.set_sprite(s, Some(Sprite::new(TextureId(0), Vec2::new(8.0, 8.0))));
    let d = world.spawn_named("orb", Transform::from_xy(40.0, 0.0));
    world.set_disc(d, Some(Disc::new(4.0, wiimaker_core::Rgba8::WHITE)));

    let mut draw = wiimaker_core::DrawList::new();
    render_world(&world, &mut draw, wiimaker_core::Rgba8::BLACK);
    // Default layers + default z: tilemap z=-1 first, then sprite, then disc (stable).
    assert_eq!(draw_kinds(draw.cmds()), vec!["sprite", "sprite", "disc"]);
}

#[test]
fn rename_and_remove_remap_scene() {
    let dir = tmp_game("remap");
    let mut project = GameProject::new("sort-r");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    add_sorting_layer(&dir, "UI", None).unwrap();

    let mut scene = Scene::new("main");
    add_entity(&mut scene, "hud", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut scene, "hud", "tex", [8.0, 8.0]).unwrap();
    set_entity_sorting(&mut scene, "hud", Some("UI"), Some(1.0)).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    rename_sorting_layer(&dir, "UI", "HUD", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let sp = reloaded
        .find_entity("hud")
        .unwrap()
        .components
        .sprite
        .as_ref()
        .unwrap();
    assert_eq!(sp.sorting_layer, "HUD");

    remove_sorting_layer(&dir, "HUD", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let sp = reloaded
        .find_entity("hud")
        .unwrap()
        .components
        .sprite
        .as_ref()
        .unwrap();
    assert!(sp.sorting_layer.is_empty());
    let layers = list_sorting_layers(&dir).unwrap();
    assert!(!layers.iter().any(|n| n == "HUD"));
    assert!(remove_sorting_layer(&dir, "Default", None).is_err());
}

#[test]
fn move_layer_changes_order() {
    let dir = tmp_game("move");
    let project = GameProject::new("sort-m");
    save_project(&dir, &project).unwrap();
    add_sorting_layer(&dir, "UI", None).unwrap();
    move_sorting_layer(&dir, "UI", 0).unwrap();
    let layers = list_sorting_layers(&dir).unwrap();
    assert_eq!(layers[0], "UI");
}

#[test]
fn doctor_warns_unknown_layer() {
    let dir = tmp_game("doctor");
    let mut project = GameProject::new("sort-d");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();

    let mut scene = Scene::new("main");
    add_entity(&mut scene, "orb", &MutateOpts::default()).unwrap();
    add_component_disc(&mut scene, "orb", 8.0, [72, 210, 160, 255]).unwrap();
    set_entity_sorting(&mut scene, "orb", Some("NopeLayer"), None).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let diag = diagnose(&dir, &load_project(&dir).unwrap());
    assert!(
        diag.issues
            .iter()
            .any(|m| m.message.contains("NopeLayer") && m.message.contains("sorting layer")),
        "{:?}",
        diag.issues
    );
}
