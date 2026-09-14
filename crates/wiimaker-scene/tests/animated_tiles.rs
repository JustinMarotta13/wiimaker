//! Animated tile palette + auto-tile: JSON roundtrip, hydrate, animate tick, doctor.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{write_anim_clip, AnimClipCatalog};
use wiimaker_core::draw::DrawCmd;
use wiimaker_core::{AUTOTILE_E, AUTOTILE_N, AUTOTILE_S, AUTOTILE_W};
use wiimaker_scene::{
    add_component_tilemap, add_entity, animate_world, diagnose, hydrate_with_catalogs, load_scene,
    render_world, save_project, save_scene, tilemap_autotile_mask, tilemap_fill, tilemap_set_cell,
    tilemap_set_palette, GameProject, MutateOpts, TextureMap, TilePaletteOpts,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-animated-tiles-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn palette_json_roundtrip_hydrate_and_tick() {
    let dir = tmp_game("tick");
    let assets = dir.join("assets");
    write_anim_clip(
        &assets,
        "water",
        vec!["water_a".into(), "water_b".into()],
        10.0,
        true,
    )
    .unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(
        &mut scene,
        "Maze",
        &MutateOpts {
            x: Some(0.0),
            y: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_tilemap(&mut scene, "Maze", 3, 3, 16.0).unwrap();
    tilemap_set_palette(
        &mut scene,
        "Maze",
        2,
        &TilePaletteOpts {
            sprite: Some("water_a".into()),
            anim: Some("water".into()),
            anim_fps: Some(10.0),
            auto_tile: Some("id".into()),
            ..Default::default()
        },
    )
    .unwrap();
    tilemap_set_cell(&mut scene, "Maze", 1, 1, 2, false).unwrap();
    tilemap_set_cell(&mut scene, "Maze", 2, 1, 2, false).unwrap();

    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    let pal = loaded
        .find_entity("Maze")
        .unwrap()
        .components
        .tilemap
        .as_ref()
        .unwrap()
        .palette
        .iter()
        .find(|p| p.id == 2)
        .unwrap();
    assert_eq!(pal.anim.as_deref(), Some("water"));
    assert_eq!(pal.auto_tile, Some(wiimaker_scene::SceneAutoTile::Id));

    let (id, _, mask, rule) = tilemap_autotile_mask(&loaded, "Maze", 1, 1).unwrap();
    assert_eq!(id, 2);
    assert_eq!(rule, Some(wiimaker_scene::SceneAutoTile::Id));
    assert_eq!(mask, AUTOTILE_E);

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let textures = TextureMap::from_names(["water_a", "water_b"]);
    let mut world = hydrate_with_catalogs(&loaded, &textures, None, Some(&anims)).unwrap();
    let maze = world.find_by_name("Maze").unwrap();
    let vis = world
        .tilemap(maze)
        .unwrap()
        .palette
        .iter()
        .find(|v| v.id == 2)
        .unwrap();
    assert_eq!(vis.frames.len(), 2);
    assert_eq!(vis.fps, 10.0);
    assert_eq!(vis.frame, 0);
    assert_eq!(world.tilemap(maze).unwrap().autotile_mask(1, 1), AUTOTILE_E);

    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &textures,
        0.11,
    );
    let vis = world
        .tilemap(maze)
        .unwrap()
        .palette
        .iter()
        .find(|v| v.id == 2)
        .unwrap();
    assert_eq!(vis.frame, 1);

    let mut draw = wiimaker_core::draw::DrawList::new();
    render_world(&world, &mut draw, wiimaker_core::Rgba8::BLACK);
    let sprites = draw
        .cmds()
        .iter()
        .filter(|c| matches!(c, DrawCmd::DrawSprite { .. }))
        .count();
    assert!(
        sprites >= 2,
        "occupied water cells should draw, got {sprites}"
    );
}

#[test]
fn solid_autotile_corner_is_closed() {
    let mut scene = wiimaker_scene::Scene::new("maze");
    add_entity(&mut scene, "Maze", &MutateOpts::default()).unwrap();
    add_component_tilemap(&mut scene, "Maze", 3, 3, 16.0).unwrap();
    tilemap_set_palette(
        &mut scene,
        "Maze",
        1,
        &TilePaletteOpts {
            auto_tile: Some("solid".into()),
            ..Default::default()
        },
    )
    .unwrap();
    tilemap_fill(&mut scene, "Maze", 0, 0, 3, 3, 1, true).unwrap();
    let mask = tilemap_autotile_mask(&scene, "Maze", 0, 0).unwrap().2;
    assert_eq!(
        mask,
        AUTOTILE_N | AUTOTILE_E | AUTOTILE_S | AUTOTILE_W,
        "corner OOB + solid neighbors should be 15"
    );
}

#[test]
fn doctor_warns_missing_tile_anim() {
    let dir = tmp_game("doctor");
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Maze", &MutateOpts::default()).unwrap();
    add_component_tilemap(&mut scene, "Maze", 2, 2, 16.0).unwrap();
    tilemap_set_palette(
        &mut scene,
        "Maze",
        1,
        &TilePaletteOpts {
            anim: Some("no-such-clip".into()),
            ..Default::default()
        },
    )
    .unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let diag = diagnose(&dir, &project);
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("no-such-clip") && m.contains("palette")),
        "expected missing tile anim warning, got {msgs:?}"
    );
}
