//! CLI-shaped round-trip: fixture scene JSON → mutate → save → load → hydrate queries.

use std::path::PathBuf;

use wiimaker_core::{tile_solid, tile_solid_world, world_to_cell};
use wiimaker_scene::{
    hydrate, load_scene, save_scene, tilemap_fill, tilemap_get_cell, tilemap_set_cell,
    tilemap_stamp_ascii, TextureMap,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maze.scene.json")
}

#[test]
fn json_file_roundtrip_set_fill_stamp_get() {
    let src = load_scene(&fixture()).expect("load fixture");
    let dir = std::env::temp_dir().join("wiimaker-tilemap-roundtrip");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("maze.scene.json");
    save_scene(&path, &src).unwrap();

    let mut scene = load_scene(&path).unwrap();
    // fill walls, carve a corridor — same verbs as `wiimaker tilemap fill|stamp|set|get`
    tilemap_fill(&mut scene, "Maze", 0, 0, 5, 3, 1, true).unwrap();
    tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "#####\n#...#\n#####").unwrap();
    tilemap_set_cell(&mut scene, "Maze", 2, 1, 0, false).unwrap();
    save_scene(&path, &scene).unwrap();

    let loaded = load_scene(&path).unwrap();
    assert_eq!(tilemap_get_cell(&loaded, "Maze", 0, 0).unwrap(), (1, true));
    assert_eq!(tilemap_get_cell(&loaded, "Maze", 1, 1).unwrap(), (0, false));
    assert_eq!(tilemap_get_cell(&loaded, "Maze", 2, 1).unwrap(), (0, false));
    assert_eq!(tilemap_get_cell(&loaded, "Maze", 4, 1).unwrap(), (1, true));

    let world = hydrate(&loaded, &TextureMap::new()).unwrap();
    assert_eq!(world_to_cell(&world, 24.0, 24.0), Some((1, 1)));
    assert!(!tile_solid(&world, 1, 1));
    assert!(!tile_solid(&world, 2, 1));
    assert!(tile_solid(&world, 0, 1));
    assert!(tile_solid(&world, 2, 0));
    // walker at corridor center cannot step into the north wall
    assert!(!tile_solid_world(&world, 24.0, 24.0));
    assert!(tile_solid_world(&world, 24.0, 8.0));
}

#[test]
fn walker_blocked_by_solid_cells() {
    let mut scene = load_scene(&fixture()).unwrap();
    tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "#####\n#...#\n#####").unwrap();
    let world = hydrate(&scene, &TextureMap::new()).unwrap();
    // start in (1,1); try each cardinal
    let open = [(2, 1), (3, 1)];
    let blocked = [(1, 0), (1, 2), (0, 1), (4, 1)];
    for (x, y) in open {
        assert!(!tile_solid(&world, x, y), "expected open {x},{y}");
    }
    for (x, y) in blocked {
        assert!(tile_solid(&world, x, y), "expected solid {x},{y}");
    }
}
