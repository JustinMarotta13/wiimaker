//! Animated tile palette + auto-tile: JSON roundtrip, hydrate, animate tick, doctor.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{write_anim_clip, AnimClipCatalog};
use wiimaker_core::draw::{DrawCmd, TextureId};
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
    // `{sprite}_{mask}` is not in the texture map, so those slots stay None and
    // `texture_for_mask` falls through to the ticking `vis.texture`.
    assert!(
        vis.auto_frames[AUTOTILE_E as usize].is_none(),
        "unresolved east variant must not freeze frame 0"
    );
    assert!(
        vis.auto_frames[AUTOTILE_W as usize].is_none(),
        "unresolved west variant must not freeze frame 0"
    );

    let mut draw_before = wiimaker_core::draw::DrawList::new();
    render_world(&world, &mut draw_before, wiimaker_core::Rgba8::BLACK);
    let before = sprite_textures(&draw_before);
    assert!(
        before.len() >= 2,
        "occupied water cells should draw, got {}",
        before.len()
    );
    assert!(
        before.iter().all(|id| *id == TextureId(0)),
        "frame 0 should sample water_a (TextureId 0), got {before:?}"
    );

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

    let mut draw_after = wiimaker_core::draw::DrawList::new();
    render_world(&world, &mut draw_after, wiimaker_core::Rgba8::BLACK);
    let after = sprite_textures(&draw_after);
    assert_eq!(after.len(), before.len());
    assert!(
        after.iter().all(|id| *id == TextureId(1)),
        "unresolved auto-tile masks must follow the ticking clip (water_b = TextureId 1), got {after:?}"
    );
    assert_ne!(
        before, after,
        "DrawSprite texture must change after animate_world when anim + auto_tile share a cell"
    );
}

/// Named `auto_sprites[mask]` that resolve stay pinned; other masks still tick.
#[test]
fn resolved_autotile_variant_stays_static_while_unresolved_ticks() {
    let dir = tmp_game("variant");
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
    let mut auto_sprites = vec![String::new(); 16];
    auto_sprites[AUTOTILE_E as usize] = "edge_e".into();
    tilemap_set_palette(
        &mut scene,
        "Maze",
        2,
        &TilePaletteOpts {
            sprite: Some("water_a".into()),
            anim: Some("water".into()),
            anim_fps: Some(10.0),
            auto_tile: Some("id".into()),
            auto_sprites: Some(auto_sprites),
            ..Default::default()
        },
    )
    .unwrap();
    tilemap_set_cell(&mut scene, "Maze", 1, 1, 2, false).unwrap();
    tilemap_set_cell(&mut scene, "Maze", 2, 1, 2, false).unwrap();

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let textures = TextureMap::from_names(["water_a", "water_b", "edge_e"]);
    let mut world = hydrate_with_catalogs(&scene, &textures, None, Some(&anims)).unwrap();
    let maze = world.find_by_name("Maze").unwrap();
    let vis = world
        .tilemap(maze)
        .unwrap()
        .palette
        .iter()
        .find(|v| v.id == 2)
        .unwrap();
    assert_eq!(
        vis.auto_frames[AUTOTILE_E as usize].map(|(t, _)| t),
        Some(TextureId(2)),
        "resolved east variant should pin edge_e"
    );
    assert!(
        vis.auto_frames[AUTOTILE_W as usize].is_none(),
        "west mask has no named variant"
    );
    assert_eq!(world.tilemap(maze).unwrap().autotile_mask(1, 1), AUTOTILE_E);
    assert_eq!(world.tilemap(maze).unwrap().autotile_mask(2, 1), AUTOTILE_W);

    let east = world.tilemap(maze).unwrap().cell_texture(1, 1).unwrap().0;
    let west = world.tilemap(maze).unwrap().cell_texture(2, 1).unwrap().0;
    assert_eq!(east, TextureId(2));
    assert_eq!(west, TextureId(0));

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
    let east = world.tilemap(maze).unwrap().cell_texture(1, 1).unwrap().0;
    let west = world.tilemap(maze).unwrap().cell_texture(2, 1).unwrap().0;
    assert_eq!(
        east,
        TextureId(2),
        "resolved auto-tile variant must not follow the clip clock"
    );
    assert_eq!(
        west,
        TextureId(1),
        "unresolved mask must sample the ticking vis.texture"
    );
}

fn sprite_textures(draw: &wiimaker_core::draw::DrawList) -> Vec<TextureId> {
    draw.cmds()
        .iter()
        .filter_map(|c| match c {
            DrawCmd::DrawSprite { texture, .. } => Some(*texture),
            _ => None,
        })
        .collect()
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
