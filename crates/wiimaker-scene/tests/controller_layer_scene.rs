//! Controller layer rename/remove remaps scene Animator weight overrides.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_animator_controller, AnimatorControllerMeta, ControllerLayer, ControllerState,
    BASE_LAYER_NAME,
};
use wiimaker_scene::{
    add_component_animator, add_entity, diagnose, load_prefab, load_scene,
    remove_controller_layer_for_game, rename_controller_layer_for_game, save_prefab, save_project,
    save_scene, set_entity_animator_layer_weight, EntityData, GameProject, MutateOpts, Prefab,
    SceneAnimator,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-ctrl-layer-{}-{}",
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
        parameters: Vec::new(),
        states: vec![ControllerState::plain("Idle", "idle")],
        transitions: Vec::new(),
        weight: 1.0,
        layers: vec![ControllerLayer {
            name: "Power".into(),
            weight: 1.0,
            default_state: "Swing".into(),
            states: vec![ControllerState::plain("Swing", "swing")],
            transitions: Vec::new(),
            graph: Default::default(),
        }],
        graph: Default::default(),
    }
}

#[test]
fn rename_and_remove_remap_scene_layer_weight_overrides() {
    let dir = tmp_game("remap");
    let mut project = GameProject::new("ctrl-layer");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "hero", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "hero", "player").unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "Power", 0.4).unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "base", 0.9).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let anim = scene
        .find_entity("hero")
        .unwrap()
        .components
        .animator
        .as_ref()
        .unwrap();
    assert_eq!(
        anim.layers
            .iter()
            .filter(|l| l.name.eq_ignore_ascii_case(BASE_LAYER_NAME))
            .count(),
        1
    );
    assert!(anim
        .layers
        .iter()
        .any(|l| l.name == BASE_LAYER_NAME && (l.weight - 0.9).abs() < 1e-6));
    assert!(!anim.layers.iter().any(|l| l.name == "base"));

    rename_controller_layer_for_game(&dir, "player", "Power", "Upper", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded
        .find_entity("hero")
        .unwrap()
        .components
        .animator
        .as_ref()
        .unwrap();
    assert!(anim
        .layers
        .iter()
        .any(|l| l.name == "Upper" && (l.weight - 0.4).abs() < 1e-6));
    assert!(!anim.layers.iter().any(|l| l.name == "Power"));
    assert!(anim.layers.iter().any(|l| l.name == BASE_LAYER_NAME));

    let diag = diagnose(&dir, &project);
    let msgs: Vec<String> = diag.issues.iter().map(|i| i.message.clone()).collect();
    assert!(
        !msgs.iter().any(|m| m.contains("does not match any layer")),
        "{msgs:?}"
    );

    remove_controller_layer_for_game(&dir, "player", "Upper", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded
        .find_entity("hero")
        .unwrap()
        .components
        .animator
        .as_ref()
        .unwrap();
    assert!(!anim
        .layers
        .iter()
        .any(|l| l.name == "Upper" || l.name == "Power"));
    assert!(anim
        .layers
        .iter()
        .any(|l| l.name == BASE_LAYER_NAME && (l.weight - 0.9).abs() < 1e-6));
}

#[test]
fn rename_remaps_nested_prefab_child_layer_override() {
    let dir = tmp_game("prefab-child");
    let project = GameProject::new("ctrl-layer-prefab");
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
    child
        .components
        .animator
        .as_mut()
        .unwrap()
        .set_layer_weight("Power", 0.2);

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

    rename_controller_layer_for_game(&dir, "player", "Power", "Upper", None).unwrap();
    let loaded = load_prefab(&prefab_path).unwrap();
    let arm = loaded.children.iter().find(|e| e.name == "Arm").unwrap();
    let anim = arm.components.animator.as_ref().unwrap();
    assert!(anim
        .layers
        .iter()
        .any(|l| l.name == "Upper" && (l.weight - 0.2).abs() < 1e-6));
    assert!(!anim.layers.iter().any(|l| l.name == "Power"));

    remove_controller_layer_for_game(&dir, "player", "Upper", None).unwrap();
    let loaded = load_prefab(&prefab_path).unwrap();
    let arm = loaded.children.iter().find(|e| e.name == "Arm").unwrap();
    let anim = arm.components.animator.as_ref().unwrap();
    assert!(anim.layers.is_empty());
}

#[test]
fn rename_trims_new_layer_name_in_scene_overrides() {
    let dir = tmp_game("trim-rename");
    let mut project = GameProject::new("ctrl-layer-trim");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "hero", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "hero", "player").unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "Power", 0.55).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    rename_controller_layer_for_game(&dir, "player", "Power", " Upper ", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded
        .find_entity("hero")
        .unwrap()
        .components
        .animator
        .as_ref()
        .unwrap();
    assert!(anim
        .layers
        .iter()
        .any(|l| l.name == "Upper" && (l.weight - 0.55).abs() < 1e-6));
    assert!(!anim.layers.iter().any(|l| l.name == "Power" || l.name == " Upper "));

    let meta = AnimatorControllerMeta::load(&dir.join("assets/player.controller.json")).unwrap();
    assert!(meta.layers.iter().any(|l| l.name == "Upper"));
}

#[test]
fn rename_keeps_renamed_layer_weight_when_target_override_exists() {
    let dir = tmp_game("merge-weight");
    let mut project = GameProject::new("ctrl-layer-merge");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "hero", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "hero", "player").unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "Power", 0.25).unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "Upper", 0.75).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    rename_controller_layer_for_game(&dir, "player", "Power", "Upper", None).unwrap();
    let reloaded = load_scene(&dir.join("scenes/main.scene.json")).unwrap();
    let anim = reloaded
        .find_entity("hero")
        .unwrap()
        .components
        .animator
        .as_ref()
        .unwrap();
    let upper = anim
        .layers
        .iter()
        .filter(|l| l.name == "Upper")
        .collect::<Vec<_>>();
    assert_eq!(upper.len(), 1);
    assert!((upper[0].weight - 0.25).abs() < 1e-6);
    assert!(!anim.layers.iter().any(|l| l.name == "Power"));
}

#[test]
fn doctor_still_warns_on_unknown_scene_layer_override() {
    let dir = tmp_game("doctor-stale");
    let mut project = GameProject::new("ctrl-layer-doctor");
    project.default_scene = "scenes/main.scene.json".into();
    save_project(&dir, &project).unwrap();
    write_animator_controller(&dir.join("assets"), "player", player_controller()).unwrap();

    let mut scene = wiimaker_scene::Scene::new("main");
    add_entity(&mut scene, "hero", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "hero", "player").unwrap();
    set_entity_animator_layer_weight(&mut scene, "hero", "Ghost", 0.5).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let diag = diagnose(&dir, &project);
    let msgs: Vec<String> = diag.issues.iter().map(|i| i.message.clone()).collect();
    assert!(
        msgs.iter().any(|m| {
            m.contains(
            "animator layer weight override 'Ghost' does not match any layer on controller 'player'"
        )
        }),
        "{msgs:?}"
    );
}
