//! Controller parameter rename/remove remaps scene Animator overrides.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_animator_controller, AnimatorControllerMeta, ControllerCondition, ControllerParam,
    ControllerParamType, ControllerState, ControllerTransition,
};
use wiimaker_scene::{
    add_component_animator, add_entity, load_prefab, load_scene, remove_controller_param_for_game,
    rename_controller_param_for_game, save_prefab, save_project, save_scene, set_entity_animator_bool,
    set_entity_animator_float, EntityData, GameProject, MutateOpts, Prefab, SceneAnimator,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-ctrl-param-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn player_controller() -> AnimatorControllerMeta {
    AnimatorControllerMeta {
        default_state: "Idle".into(),
        parameters: vec![
            ControllerParam {
                name: "Moving".into(),
                kind: ControllerParamType::Bool,
                default: Some(serde_json::json!(false)),
            },
            ControllerParam {
                name: "Speed".into(),
                kind: ControllerParamType::Float,
                default: Some(serde_json::json!(0.0)),
            },
        ],
        states: vec![
            ControllerState::plain("Idle", "idle"),
            ControllerState::plain("Walk", "walk"),
        ],
        transitions: vec![ControllerTransition {
            from: "Idle".into(),
            to: "Walk".into(),
            conditions: vec![ControllerCondition {
                param: "Moving".into(),
                equals: Some(serde_json::json!(true)),
                ..Default::default()
            }],
            has_exit_time: false,
        }],
        weight: 1.0,
        layers: Vec::new(),
        graph: Default::default(),
    }
}

#[test]
fn rename_and_remove_remap_scene_param_overrides() {
    let dir = tmp_game("remap");
    let mut project = GameProject::new("ctrl-param");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "hero", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "hero", "player").unwrap();
    set_entity_animator_bool(&mut scene, "hero", "Moving", true).unwrap();
    set_entity_animator_float(&mut scene, "hero", "Speed", 2.5).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    rename_controller_param_for_game(&dir, "player", "Moving", "Running", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded.find_entity("hero").unwrap().components.animator.as_ref().unwrap();
    assert!(anim.parameters.iter().any(|p| p.name == "Running" && p.bool_value == Some(true)));
    assert!(!anim.parameters.iter().any(|p| p.name == "Moving"));

    remove_controller_param_for_game(&dir, "player", "Running", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded.find_entity("hero").unwrap().components.animator.as_ref().unwrap();
    assert!(!anim.parameters.iter().any(|p| p.name == "Running"));
    assert!(anim
        .parameters
        .iter()
        .any(|p| p.name == "Speed" && p.float_value == Some(2.5)));
}

#[test]
fn rename_remaps_nested_prefab_child_animator_override() {
    let dir = tmp_game("prefab-child");
    let project = GameProject::new("ctrl-prefab");
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut child = EntityData {
        name: "Arm".into(),
        parent: Some("Body".into()),
        transform: Default::default(),
        components: Default::default(),
        tag: 0,
        prefab: None,
    };
    child.components.animator = Some(SceneAnimator::new("player"));
    child.components.animator.as_mut().unwrap().parameters.push(
        wiimaker_scene::SceneAnimatorParam {
            name: "Moving".into(),
            bool_value: Some(true),
            float_value: None,
        },
    );

    let prefab = Prefab {
        entity: EntityData {
            name: "Body".into(),
            parent: None,
            transform: Default::default(),
            components: Default::default(),
            tag: 0,
            prefab: None,
        },
        children: vec![child],
        base: None,
        overrides: Vec::new(),
    };
    let prefab_path = dir.join("assets/prefabs/body.prefab.json");
    save_prefab(&prefab_path, &prefab).unwrap();

    rename_controller_param_for_game(&dir, "player", "Moving", "Running", None).unwrap();
    let loaded = load_prefab(&prefab_path).unwrap();
    let arm = loaded.children.iter().find(|e| e.name == "Arm").unwrap();
    let anim = arm.components.animator.as_ref().unwrap();
    assert!(anim.parameters.iter().any(|p| p.name == "Running" && p.bool_value == Some(true)));
    assert!(!anim.parameters.iter().any(|p| p.name == "Moving"));
}
