//! Animator controller: write asset, attach component, hydrate default, Idle↔Walk.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_anim_clip, write_animator_controller, AnimClipCatalog, AnimatorControllerCatalog,
    AnimatorControllerMeta, ControllerCondition, ControllerParam, ControllerParamType,
    ControllerState, ControllerTransition,
};
use wiimaker_scene::{
    add_component_animator, add_entity, animate_world, diagnose, hydrate_with_all_catalogs,
    load_scene, save_project, save_scene, set_entity_animator_bool, GameProject, MutateOpts,
    TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-animator-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn player_controller() -> AnimatorControllerMeta {
    AnimatorControllerMeta {
        default_state: "Idle".into(),
        parameters: vec![ControllerParam {
            name: "Moving".into(),
            kind: ControllerParamType::Bool,
            default: Some(serde_json::json!(false)),
        }],
        states: vec![
            ControllerState {
                name: "Idle".into(),
                clip: "idle".into(),
                speed: 1.0,
            },
            ControllerState {
                name: "Walk".into(),
                clip: "walk".into(),
                speed: 1.0,
            },
        ],
        transitions: vec![
            ControllerTransition {
                from: "Idle".into(),
                to: "Walk".into(),
                conditions: vec![ControllerCondition {
                    param: "Moving".into(),
                    equals: Some(serde_json::json!(true)),
                    ..Default::default()
                }],
                has_exit_time: false,
            },
            ControllerTransition {
                from: "Walk".into(),
                to: "Idle".into(),
                conditions: vec![ControllerCondition {
                    param: "Moving".into(),
                    equals: Some(serde_json::json!(false)),
                    ..Default::default()
                }],
                has_exit_time: false,
            },
        ],
    }
}

#[test]
fn hydrate_default_state_and_idle_walk() {
    let dir = tmp_game("hydrate");
    let assets = dir.join("assets");
    write_anim_clip(
        &assets,
        "idle",
        vec!["idle_0".into(), "idle_1".into()],
        10.0,
        true,
    )
    .unwrap();
    write_anim_clip(
        &assets,
        "walk",
        vec!["walk_0".into(), "walk_1".into()],
        10.0,
        true,
    )
    .unwrap();
    write_animator_controller(&assets, "player", player_controller()).unwrap();

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
    add_component_animator(&mut scene, "Player", "player").unwrap();
    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    assert_eq!(
        loaded
            .find_entity("Player")
            .unwrap()
            .components
            .animator
            .as_ref()
            .unwrap()
            .controller,
        "player"
    );

    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let controllers = AnimatorControllerCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_with_all_catalogs(
        &loaded,
        &TextureMap::new(),
        None,
        Some(&anims),
        Some(&controllers),
    )
    .unwrap();
    let id = world.find_by_name("Player").unwrap();
    let a = world.animator(id).unwrap();
    assert_eq!(a.controller, "player");
    assert_eq!(a.state, "Idle");
    let anim = world.animation(id).unwrap();
    assert_eq!(anim.clip, "idle");
    assert_eq!(anim.cells, vec!["idle_0", "idle_1"]);

    assert!(world.set_animator_bool(id, "Moving", true));
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        0.0,
    );
    assert_eq!(world.animator(id).unwrap().state, "Walk");
    assert_eq!(world.animation(id).unwrap().clip, "walk");
    assert_eq!(world.animation(id).unwrap().cells[0], "walk_0");

    world.set_animator_bool(id, "Moving", false);
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        0.0,
    );
    assert_eq!(world.animator(id).unwrap().state, "Idle");
    assert_eq!(world.animation(id).unwrap().cells[0], "idle_0");
}

#[test]
fn scene_param_override_hydrates() {
    let dir = tmp_game("override");
    let assets = dir.join("assets");
    write_anim_clip(&assets, "idle", vec!["i".into()], 10.0, true).unwrap();
    write_anim_clip(&assets, "walk", vec!["w".into()], 10.0, true).unwrap();
    write_animator_controller(&assets, "player", player_controller()).unwrap();
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "Player", "player").unwrap();
    set_entity_animator_bool(&mut scene, "Player", "Moving", true).unwrap();
    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let controllers = AnimatorControllerCatalog::load_dir(&assets).unwrap();
    let mut world = hydrate_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        Some(&anims),
        Some(&controllers),
    )
    .unwrap();
    let id = world.find_by_name("Player").unwrap();
    assert_eq!(world.animator(id).unwrap().bool_value("Moving"), Some(true));
    animate_world(
        &mut world,
        &wiimaker_assets::SpriteCatalog::empty(),
        &TextureMap::new(),
        0.0,
    );
    assert_eq!(world.animator(id).unwrap().state, "Walk");
}

#[test]
fn doctor_warns_missing_controller() {
    let dir = tmp_game("doctor");
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "Player", "no-such").unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let diag = diagnose(&dir, &project);
    let msgs: Vec<_> = diag.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("no-such") && m.contains("controller.json")),
        "expected missing-controller warning, got {msgs:?}"
    );
}

#[test]
fn old_animation_only_scene_still_hydrates() {
    let dir = tmp_game("legacy");
    let assets = dir.join("assets");
    write_anim_clip(&assets, "chomp", vec!["a".into(), "b".into()], 10.0, true).unwrap();
    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    wiimaker_scene::add_component_animation(&mut scene, "Player", "chomp", None, true).unwrap();
    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let world =
        wiimaker_scene::hydrate_with_catalogs(&scene, &TextureMap::new(), None, Some(&anims))
            .unwrap();
    let id = world.find_by_name("Player").unwrap();
    assert!(world.animator(id).is_none());
    assert_eq!(world.animation(id).unwrap().clip, "chomp");
}
