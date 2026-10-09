//! Blend trees end to end: controller asset → hydrate → live Float params → clip selection,
//! plus doctor warnings for broken trees.

use std::fs;
use std::path::PathBuf;

use wiimaker_assets::{
    write_anim_clip, write_animator_controller, AnimClipCatalog, AnimatorControllerCatalog,
    AnimatorControllerMeta, BlendDimension, BlendMotion, BlendTreeMeta, ControllerParam,
    ControllerParamType, ControllerState, SpriteCatalog,
};
use wiimaker_scene::{
    add_component_animator, add_entity, animate_world, diagnose, hydrate_with_all_catalogs,
    load_scene, save_project, save_scene, GameProject, MutateOpts, Scene, TextureMap,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiimaker-blend-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

fn write_clips(assets: &std::path::Path, names: &[&str]) {
    for n in names {
        write_anim_clip(
            assets,
            n,
            vec![format!("{n}_0"), format!("{n}_1")],
            10.0,
            true,
        )
        .unwrap();
    }
}

fn motion_1d(clip: &str, t: f32) -> BlendMotion {
    BlendMotion {
        clip: clip.into(),
        threshold: Some(t),
        position: None,
    }
}

fn motion_2d(clip: &str, x: f32, y: f32) -> BlendMotion {
    BlendMotion {
        clip: clip.into(),
        threshold: None,
        position: Some([x, y]),
    }
}

fn floats(names: &[&str]) -> Vec<ControllerParam> {
    names
        .iter()
        .map(|n| ControllerParam {
            name: (*n).into(),
            kind: ControllerParamType::Float,
            default: Some(serde_json::json!(0.0)),
        })
        .collect()
}

fn blend_state(name: &str, tree: BlendTreeMeta) -> ControllerState {
    ControllerState {
        clip: String::new(),
        blend_tree: Some(tree),
        ..ControllerState::plain(name, "")
    }
}

fn locomotion_controller() -> AnimatorControllerMeta {
    AnimatorControllerMeta {
        default_state: "Locomotion".into(),
        parameters: floats(&["Speed", "Heading"]),
        states: vec![
            blend_state(
                "Locomotion",
                BlendTreeMeta {
                    dimension: BlendDimension::OneD,
                    params: vec!["Speed".into()],
                    motions: vec![motion_1d("idle", 0.0), motion_1d("walk", 1.0)],
                },
            ),
            blend_state(
                "Aim",
                BlendTreeMeta {
                    dimension: BlendDimension::TwoD,
                    params: vec!["Speed".into(), "Heading".into()],
                    motions: vec![
                        motion_2d("idle", 0.0, 0.0),
                        motion_2d("walk", 1.0, 0.0),
                        motion_2d("up", 0.0, 1.0),
                    ],
                },
            ),
        ],
        transitions: vec![],
        weight: 1.0,
        layers: Vec::new(),
    }
}

fn hydrate_player(dir: &std::path::Path, controller: &str) -> wiimaker_core::World {
    let assets = dir.join("assets");
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    add_component_animator(&mut scene, "Player", controller).unwrap();
    let path = dir.join("scenes/main.scene.json");
    save_scene(&path, &scene).unwrap();
    let loaded = load_scene(&path).unwrap();
    let anims = AnimClipCatalog::load_dir(&assets).unwrap();
    let controllers = AnimatorControllerCatalog::load_dir(&assets).unwrap();
    hydrate_with_all_catalogs(
        &loaded,
        &TextureMap::new(),
        None,
        Some(&anims),
        Some(&controllers),
    )
    .unwrap()
}

fn tick(world: &mut wiimaker_core::World) {
    animate_world(world, &SpriteCatalog::empty(), &TextureMap::new(), 0.0);
}

fn assert_simplex(weights: &[f32]) {
    let sum: f32 = weights.iter().sum();
    assert!((sum - 1.0).abs() < 1e-4, "weights {weights:?} sum to {sum}");
    assert!(
        weights.iter().all(|w| *w >= 0.0),
        "negative weight in {weights:?}"
    );
}

#[test]
fn blend_1d_drives_clip_from_float_param() {
    let dir = tmp_game("1d");
    let assets = dir.join("assets");
    write_clips(&assets, &["idle", "walk"]);
    write_animator_controller(&assets, "player", locomotion_controller()).unwrap();
    let mut world = hydrate_player(&dir, "player");
    let id = world.find_by_name("Player").unwrap();
    assert_eq!(world.animator(id).unwrap().state, "Locomotion");
    assert!(world.animator(id).unwrap().current_blend().is_some());

    assert!(world.set_animator_float(id, "Speed", 1.0));
    tick(&mut world);
    assert_eq!(world.animation(id).unwrap().clip, "walk");

    assert!(world.set_animator_float(id, "Speed", 0.0));
    tick(&mut world);
    assert_eq!(world.animation(id).unwrap().clip, "idle");
}

#[test]
fn blend_1d_weights_sum_to_one_and_clamp_out_of_range() {
    let dir = tmp_game("1d-clamp");
    let assets = dir.join("assets");
    write_clips(&assets, &["idle", "walk"]);
    write_animator_controller(&assets, "player", locomotion_controller()).unwrap();
    let mut world = hydrate_player(&dir, "player");
    let id = world.find_by_name("Player").unwrap();

    for speed in [-5.0, -0.25, 0.0, 0.3, 0.5, 0.9, 1.0, 1.5, 40.0] {
        world.set_animator_float(id, "Speed", speed);
        tick(&mut world);
        let tree = world.animator(id).unwrap().current_blend().unwrap();
        assert_simplex(&tree.weights);
    }

    world.set_animator_float(id, "Speed", 40.0);
    tick(&mut world);
    let tree = world.animator(id).unwrap().current_blend().unwrap();
    assert_eq!(tree.weights, vec![0.0, 1.0]);
    assert_eq!(world.animation(id).unwrap().clip, "walk");

    world.set_animator_float(id, "Speed", -40.0);
    tick(&mut world);
    let tree = world.animator(id).unwrap().current_blend().unwrap();
    assert_eq!(tree.weights, vec![1.0, 0.0]);
    assert_eq!(world.animation(id).unwrap().clip, "idle");
}

#[test]
fn blend_missing_param_reads_as_zero() {
    let dir = tmp_game("missing");
    let assets = dir.join("assets");
    write_clips(&assets, &["idle", "walk"]);
    let mut controller = locomotion_controller();
    controller.parameters.retain(|p| p.name != "Speed");
    controller.states[0].blend_tree.as_mut().unwrap().params = vec!["Speed".into()];
    write_animator_controller(&assets, "player", controller).unwrap();
    let mut world = hydrate_player(&dir, "player");
    let id = world.find_by_name("Player").unwrap();
    tick(&mut world);
    let tree = world.animator(id).unwrap().current_blend().unwrap();
    assert_simplex(&tree.weights);
    assert_eq!(world.animation(id).unwrap().clip, "idle");
}

#[test]
fn blend_2d_selects_motion_by_direction() {
    let dir = tmp_game("2d");
    let assets = dir.join("assets");
    write_clips(&assets, &["idle", "walk", "up"]);
    let mut controller = locomotion_controller();
    controller.default_state = "Aim".into();
    write_animator_controller(&assets, "player", controller).unwrap();
    let mut world = hydrate_player(&dir, "player");
    let id = world.find_by_name("Player").unwrap();
    assert_eq!(world.animator(id).unwrap().state, "Aim");

    world.set_animator_float(id, "Speed", 1.0);
    world.set_animator_float(id, "Heading", 0.0);
    tick(&mut world);
    assert_eq!(world.animation(id).unwrap().clip, "walk");

    world.set_animator_float(id, "Speed", 0.0);
    world.set_animator_float(id, "Heading", 1.0);
    tick(&mut world);
    assert_eq!(world.animation(id).unwrap().clip, "up");

    world.set_animator_float(id, "Speed", 0.3);
    world.set_animator_float(id, "Heading", -0.2);
    for _ in 0..3 {
        tick(&mut world);
        let tree = world.animator(id).unwrap().current_blend().unwrap();
        assert_simplex(&tree.weights);
    }
}

#[test]
fn doctor_flags_broken_blend_trees() {
    let dir = tmp_game("doctor");
    let assets = dir.join("assets");
    write_clips(&assets, &["idle", "walk"]);

    let controller = AnimatorControllerMeta {
        default_state: "Empty".into(),
        parameters: floats(&["Speed", "Heading"]),
        states: vec![
            blend_state(
                "Empty",
                BlendTreeMeta {
                    dimension: BlendDimension::OneD,
                    params: vec!["Speed".into()],
                    motions: vec![],
                },
            ),
            blend_state(
                "Unknown",
                BlendTreeMeta {
                    dimension: BlendDimension::OneD,
                    params: vec!["Nope".into()],
                    motions: vec![motion_1d("idle", 0.0)],
                },
            ),
            blend_state(
                "MissingClip",
                BlendTreeMeta {
                    dimension: BlendDimension::OneD,
                    params: vec!["Speed".into()],
                    motions: vec![motion_1d("ghost", 0.0)],
                },
            ),
            blend_state(
                "OutOfRange",
                BlendTreeMeta {
                    dimension: BlendDimension::TwoD,
                    params: vec!["Speed".into(), "Heading".into()],
                    motions: vec![motion_2d("idle", 0.0, 0.0), motion_2d("walk", 2.5, 0.0)],
                },
            ),
            blend_state(
                "Stacked",
                BlendTreeMeta {
                    dimension: BlendDimension::TwoD,
                    params: vec!["Speed".into(), "Heading".into()],
                    motions: vec![motion_2d("idle", 0.5, 0.5), motion_2d("walk", 0.5, 0.5)],
                },
            ),
        ],
        transitions: vec![],
        weight: 1.0,
        layers: Vec::new(),
    };
    write_animator_controller(&assets, "player", controller).unwrap();

    let project = GameProject::new("probe");
    save_project(&dir, &project).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Player", &MutateOpts::default()).unwrap();
    save_scene(&dir.join("scenes/main.scene.json"), &scene).unwrap();

    let diag = diagnose(&dir, &project);
    let msgs: Vec<String> = diag.issues.iter().map(|i| i.message.clone()).collect();
    let has = |needle: &str| msgs.iter().any(|m| m.contains(needle));
    assert!(has("blend state 'Empty' has no motions"), "{msgs:?}");
    assert!(has("param 'Nope' is not declared"), "{msgs:?}");
    assert!(has("motion clip 'ghost' missing"), "{msgs:?}");
    assert!(has("position (2.5, 0) is outside -1..1"), "{msgs:?}");
    assert!(has("share a position"), "{msgs:?}");
}
