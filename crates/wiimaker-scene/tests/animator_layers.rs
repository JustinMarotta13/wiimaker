//! Override layers: base walk, upper-body aim wins while weight > 0.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_anim_clip, write_animator_controller, AnimClipCatalog, AnimatorControllerCatalog,
    AnimatorControllerMeta, ControllerCondition, ControllerLayer, ControllerParam,
    ControllerParamType, ControllerState, ControllerTransition,
};
use wiimaker_scene::{
    add_component_animator, add_entity, diagnose, hydrate_with_all_catalogs, save_project,
    save_scene, set_entity_animator_layer_weight, GameProject, MutateOpts, Scene, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("wiimaker-layers-{}-{}", label, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn controller() -> AnimatorControllerMeta {
    AnimatorControllerMeta {
        default_state: "Walk".into(),
        parameters: vec![ControllerParam {
            name: "Aiming".into(),
            kind: ControllerParamType::Bool,
            default: Some(serde_json::json!(false)),
        }],
        states: vec![
            ControllerState::plain("Walk", "walk"),
            ControllerState::plain("Idle", "idle"),
        ],
        transitions: vec![ControllerTransition {
            from: "Walk".into(),
            to: "Idle".into(),
            conditions: vec![ControllerCondition {
                param: "Aiming".into(),
                equals: Some(serde_json::json!(false)),
                ..Default::default()
            }],
            has_exit_time: false,
        }],
        weight: 1.0,
        layers: vec![ControllerLayer {
            name: "UpperBody".into(),
            weight: 1.0,
            default_state: "Aim".into(),
            states: vec![
                ControllerState::plain("Aim", "aim"),
                ControllerState::plain("Empty", ""),
            ],
            transitions: vec![],
        }],
    }
}

fn world_for(dir: &std::path::Path) -> wiimaker_core::World {
    let assets = dir.join("assets");
    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let controllers = AnimatorControllerCatalog::load_dir(&assets).unwrap();
    let scene = wiimaker_scene::load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    hydrate_with_all_catalogs(
        &scene,
        &TextureMap::new(),
        None,
        Some(&anims),
        Some(&controllers),
    )
    .unwrap()
}

#[test]
fn override_aim_wins_and_zero_weight_falls_back() {
    let dir = tmp_game("play");
    let assets = dir.join("assets");
    for (name, cell) in [("walk", "w0"), ("idle", "i0"), ("aim", "a0")] {
        write_anim_clip(&assets, name, vec![cell.into()], 8.0, true).unwrap();
    }
    write_animator_controller(&assets, "player", controller()).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "Player", "player").unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let world = world_for(&dir);
    let id = world.find_by_name("Player").unwrap();
    assert_eq!(world.animator(id).unwrap().state, "Walk");
    assert_eq!(world.animator(id).unwrap().layer_state_name(1), Some("Aim"));
    assert_eq!(world.animation(id).unwrap().clip, "aim");
    assert_eq!(world.animator(id).unwrap().layer_count(), 2);

    set_entity_animator_layer_weight(&mut scene, "Player", "UpperBody", 0.0).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    let world = world_for(&dir);
    let id = world.find_by_name("Player").unwrap();
    assert_eq!(world.animator(id).unwrap().layer_weight(1), Some(0.0));
    assert_eq!(world.animation(id).unwrap().clip, "walk");
}

#[test]
fn doctor_warns_on_layer_problems() {
    let dir = tmp_game("doctor");
    let assets = dir.join("assets");
    write_anim_clip(&assets, "walk", vec!["w0".into()], 8.0, true).unwrap();
    let mut meta = controller();
    meta.states.retain(|s| s.name == "Walk");
    meta.default_state = "Walk".into();
    meta.transitions.clear();
    meta.layers.push(ControllerLayer {
        name: "UpperBody".into(),
        weight: 1.0,
        default_state: "Aim".into(),
        states: vec![ControllerState::plain("Aim", "aim")],
        transitions: vec![],
    });
    meta.layers.push(ControllerLayer {
        name: "Empty".into(),
        weight: 2.5,
        default_state: "Nope".into(),
        states: vec![],
        transitions: vec![ControllerTransition {
            from: "Nope".into(),
            to: "Nope".into(),
            conditions: vec![ControllerCondition {
                param: "Ghost".into(),
                equals: Some(serde_json::json!(true)),
                ..Default::default()
            }],
            has_exit_time: false,
        }],
    });
    // Unknown transition endpoints are a load error. Keep the empty layer valid
    // and put the bad default / missing clip / undeclared param on a layer that has states.
    meta.layers.pop();
    meta.layers.push(ControllerLayer {
        name: "Empty".into(),
        weight: 2.5,
        default_state: String::new(),
        states: vec![],
        transitions: vec![],
    });
    meta.layers.push(ControllerLayer {
        name: "Broken".into(),
        weight: 1.0,
        default_state: "Nope".into(),
        states: vec![ControllerState::plain("Aim", "missing")],
        transitions: vec![ControllerTransition {
            from: "Aim".into(),
            to: "Aim".into(),
            conditions: vec![ControllerCondition {
                param: "Ghost".into(),
                equals: Some(serde_json::json!(true)),
                ..Default::default()
            }],
            has_exit_time: false,
        }],
    });
    write_animator_controller(&assets, "player", meta).unwrap();
    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();
    let diag = diagnose(&dir, &project);
    let msgs: Vec<String> = diag.issues.iter().map(|i| i.message.clone()).collect();
    let has = |needle: &str| msgs.iter().any(|m| m.contains(needle));
    assert!(has("duplicate layer name 'UpperBody'"), "{msgs:?}");
    assert!(has("layer 'Empty' is empty"), "{msgs:?}");
    assert!(has("weight 2.5 is outside 0..1"), "{msgs:?}");
    assert!(has("default 'Nope' is not a state"), "{msgs:?}");
    assert!(has("clip 'missing' missing"), "{msgs:?}");
    assert!(has("transition param 'Ghost' is not declared"), "{msgs:?}");
}
