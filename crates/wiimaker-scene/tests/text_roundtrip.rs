//! Text / HUD hydrate + render_world emit (host-first bitmap font).

use std::fs;
use std::path::PathBuf;

use wiimaker_core::draw::DrawCmd;
use wiimaker_scene::{
    add_component_text, add_entity, diagnose, hydrate, load_scene, render_world, save_project,
    save_scene, set_entity_text, GameProject, MutateOpts, SceneTextAlign, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-text-hud-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn json_roundtrip_hydrate_emits_draw_text() {
    let dir = tmp_game("hydrate");
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(
        &mut scene,
        "Hud",
        &MutateOpts {
            x: Some(24.0),
            y: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_component_text(
        &mut scene,
        "Hud",
        "Score: 0",
        16.0,
        [255, 220, 64, 255],
        SceneTextAlign::Left,
    )
    .unwrap();

    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"Text\""));
    assert!(text.contains("Score: 0"));

    let mut edited = load_scene(&path).unwrap();
    set_entity_text(
        &mut edited,
        "Hud",
        Some("Score: 1"),
        Some(12.0),
        None,
        Some(SceneTextAlign::Center),
    )
    .unwrap();
    save_scene(&path, &edited).unwrap();
    let reloaded = load_scene(&path).unwrap();
    let t = reloaded
        .find_entity("Hud")
        .unwrap()
        .components
        .text
        .as_ref()
        .unwrap();
    assert_eq!(t.text, "Score: 1");
    assert!((t.size - 12.0).abs() < 1e-4);
    assert_eq!(t.align, SceneTextAlign::Center);

    let world = hydrate(&reloaded, &TextureMap::new()).unwrap();
    let id = world.find_by_name("Hud").unwrap();
    let rt = world.text(id).unwrap();
    assert_eq!(rt.string, "Score: 1");
    assert_eq!(rt.align, wiimaker_core::TextAlign::Center);

    let mut draw = wiimaker_core::DrawList::new();
    render_world(&world, &mut draw, wiimaker_core::Rgba8::BLACK);
    let hit = draw
        .cmds()
        .iter()
        .any(|c| matches!(c, DrawCmd::DrawText { text, .. } if text == "Score: 1"));
    assert!(hit, "expected DrawText, cmds={:?}", draw.cmds());
}

#[test]
fn doctor_warns_zero_text_size() {
    let dir = tmp_game("doctor");
    let mut project = GameProject::new("text-doc");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Hud", &MutateOpts::default()).unwrap();
    add_component_text(
        &mut scene,
        "Hud",
        "Hi",
        16.0,
        [255, 255, 255, 255],
        SceneTextAlign::Left,
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Hud")
        .unwrap()
        .components
        .text
        .as_mut()
        .unwrap()
        .size = 0.0;
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let diag = diagnose(&dir, &project);
    assert!(
        diag.issues
            .iter()
            .any(|m| m.message.contains("Text size must be > 0")),
        "{:?}",
        diag.issues
    );
}
