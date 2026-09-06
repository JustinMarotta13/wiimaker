//! Camera + Follow hydrate / render / follow tick.

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::DrawCmd;
use wiimaker_core::math::Vec2;
use wiimaker_scene::{
    add_component_camera, add_component_follow, add_entity, hydrate, render_world, MutateOpts,
    TextureMap,
};

#[test]
fn hydrate_follow_and_snap_centers_target() {
    let mut scene = wiimaker_scene::Scene::new("cam");
    add_entity(
        &mut scene,
        "Player",
        &MutateOpts {
            x: Some(500.0),
            y: Some(240.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Main Camera",
        &MutateOpts {
            x: Some(320.0),
            y: Some(240.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_camera(&mut scene, "Main Camera", true).unwrap();
    add_component_follow(&mut scene, "Main Camera", "Player", Some(1.0)).unwrap();

    let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
    let cam = world.find_by_name("Main Camera").unwrap();
    assert!(world.camera(cam).unwrap().active);
    assert_eq!(world.follow(cam).unwrap().target, "Player");

    world.follow_cameras();
    let t = world.transform(cam).unwrap().translation;
    assert_eq!(t.x, 500.0);
    assert_eq!(t.y, 240.0);

    let mut draw = wiimaker_core::draw::DrawList::new();
    render_world(&world, &mut draw, Rgba8::BLACK);
    assert!(draw
        .cmds()
        .iter()
        .any(|c| matches!(c, DrawCmd::SetCamera { .. })));
    let disc = draw.cmds().iter().find_map(|c| match c {
        DrawCmd::DrawDisc { center, .. } => Some(*center),
        _ => None,
    });
    // Player at 500,240; camera centered there → offset x = 180; screen x = 320
    assert_eq!(disc, Some(Vec2::new(320.0, 240.0)));
}
