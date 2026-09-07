//! GridMover hydrate + tick (no diagonals, immediate reverse, snap).

use wiimaker_core::input::{Button, Input};
use wiimaker_core::Dir;
use wiimaker_scene::{
    add_component_grid_mover, add_entity, hydrate, MutateOpts, TextureMap,
};

fn pad(up: bool, down: bool, left: bool, right: bool) -> Input {
    let mut i = Input::new();
    i.set_down(Button::DPadUp, up);
    i.set_down(Button::DPadDown, down);
    i.set_down(Button::DPadLeft, left);
    i.set_down(Button::DPadRight, right);
    i
}

#[test]
fn hydrate_grid_mover_and_step() {
    let mut scene = wiimaker_scene::Scene::new("grid");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(30.0),
            y: Some(30.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_grid_mover(&mut scene, "Player", 20.0, 400.0).unwrap();

    let json = serde_json::to_string(&scene).unwrap();
    assert!(json.contains("GridMover"));
    let scene: wiimaker_scene::Scene = serde_json::from_str(&json).unwrap();

    let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
    let id = world.find_by_name("Player").unwrap();
    assert!((world.grid_mover(id).unwrap().cell - 20.0).abs() < 1e-4);

    world.step_grid_movers(&pad(true, false, false, true), 0.05);
    let t = world.transform(id).unwrap().translation;
    // Up+Right → horizontal only (Right). 400 u/s * 0.05s = 20 → next center 50.
    assert!((t.x - 50.0).abs() < 1e-3, "x={}", t.x);
    assert!((t.y - 30.0).abs() < 1e-3, "y={}", t.y);
    assert_eq!(world.grid_mover(id).unwrap().current_dir, Some(Dir::Right));
}

#[test]
fn reverse_off_center_after_hydrate() {
    let mut scene = wiimaker_scene::Scene::new("grid");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(30.0),
            y: Some(30.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_grid_mover(&mut scene, "Player", 20.0, 200.0).unwrap();
    let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
    let id = world.find_by_name("Player").unwrap();
    world.step_grid_movers(&pad(false, false, false, true), 0.05); // 10px right → x=40
    let x0 = world.transform(id).unwrap().translation.x;
    assert!(x0 > 30.0 && x0 < 50.0);
    world.step_grid_movers(&pad(false, false, true, false), 0.05);
    let x1 = world.transform(id).unwrap().translation.x;
    assert!(x1 < x0, "reverse should move left {x0} → {x1}");
    assert_eq!(world.grid_mover(id).unwrap().current_dir, Some(Dir::Left));
}
