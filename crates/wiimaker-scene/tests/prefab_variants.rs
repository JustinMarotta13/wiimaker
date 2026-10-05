//! Prefab variants: create, instantiate (base+overrides), apply/revert to variant asset.

use std::fs;
use std::path::PathBuf;

use wiimaker_scene::{
    add_entity, apply_instance_to_base, apply_prefab, apply_variant_override_to_base,
    create_prefab_variant, entity_to_prefab, instantiate_prefab, load_prefab,
    load_prefab_for_instance, normalize_prefab_source, prefab_base_chain, prefab_chain_status,
    prefab_variant_chrome, refresh_variant_overrides, resolve_prefab, revert_prefab_instance,
    revert_prefab_instance_fields, revert_variant_to_base, save_prefab, set_entity_parent,
    set_entity_transform, unpack_prefab_instance, variant_from_instance, ApplyBaseTarget,
    MutateOpts, Scene, UndoStack,
};

fn tmp_game(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wiimaker-prefab-variants-{}-{}",
        label,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets").join("prefabs")).unwrap();
    fs::create_dir_all(dir.join("scenes")).unwrap();
    dir
}

#[test]
fn create_variant_instantiate_applies_base_plus_overrides() {
    let dir = tmp_game("create-inst");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(40.0),
            y: Some(50.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    // Paint base red.
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];

    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();

    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_blue").unwrap();
    assert!(variant.is_variant());
    assert_eq!(
        variant.base.as_deref(),
        Some("assets/prefabs/ghost.prefab.json")
    );
    assert!(variant.overrides.is_empty());

    // Override color on the variant asset.
    variant.entity.components.disc.as_mut().unwrap().color = [0, 0, 255, 255];
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    assert!(
        variant.overrides.iter().any(|f| f == "Disc.color"),
        "{:?}",
        variant.overrides
    );
    save_prefab(&variant_path, &variant).unwrap();

    // Change base radius — non-overridden field should inherit on resolve.
    let mut base = load_prefab(&base_path).unwrap();
    base.entity.components.disc.as_mut().unwrap().radius = 14.0;
    save_prefab(&base_path, &base).unwrap();

    let raw = load_prefab(&variant_path).unwrap();
    let resolved = resolve_prefab(&dir, &raw).unwrap();
    assert!((resolved.entity.components.disc.as_ref().unwrap().radius - 14.0).abs() < 1e-4);
    assert_eq!(
        resolved.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );

    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_blue.prefab.json",
        Some(200.0),
        Some(210.0),
    );
    let e = scene.find_entity(&inst).unwrap();
    assert_eq!(
        e.prefab.as_deref(),
        Some("assets/prefabs/ghost_blue.prefab.json")
    );
    assert!((e.components.disc.as_ref().unwrap().radius - 14.0).abs() < 1e-4);
    assert_eq!(e.components.disc.as_ref().unwrap().color, [0, 0, 255, 255]);
}

#[test]
fn apply_revert_writes_variant_not_base() {
    let dir = tmp_game("apply-revert");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();

    let (variant_path, variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    let resolved = resolve_prefab(&dir, &variant).unwrap();
    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_v.prefab.json",
        Some(100.0),
        Some(110.0),
    );

    set_entity_transform(&mut scene, &inst, Some(300.0), Some(310.0)).unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 22.0;

    let mut pf = load_prefab(&variant_path).unwrap();
    apply_prefab(&mut scene, &inst, &mut pf, "ghost_v").unwrap();
    refresh_variant_overrides(&dir, &mut pf).unwrap();
    save_prefab(&variant_path, &pf).unwrap();

    let base_after = load_prefab(&base_path).unwrap();
    assert!((base_after.entity.transform.translation[0] - 10.0).abs() < 1e-4);
    assert!((base_after.entity.components.disc.as_ref().unwrap().radius - 8.0).abs() < 1e-4);

    let variant_after = load_prefab(&variant_path).unwrap();
    assert!(variant_after.is_variant());
    assert!((variant_after.entity.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!(
        (variant_after
            .entity
            .components
            .disc
            .as_ref()
            .unwrap()
            .radius
            - 22.0)
            .abs()
            < 1e-4
    );
    assert!(variant_after
        .overrides
        .iter()
        .any(|f| f == "transform.position"));
    assert!(variant_after.overrides.iter().any(|f| f == "Disc.radius"));

    set_entity_transform(&mut scene, &inst, Some(1.0), Some(2.0)).unwrap();
    let resolved = load_prefab_for_instance(&dir, scene.find_entity(&inst).unwrap()).unwrap();
    revert_prefab_instance(&mut scene, &inst, &resolved).unwrap();
    let e = scene.find_entity(&inst).unwrap();
    assert!((e.transform.translation[0] - 300.0).abs() < 1e-4);
    assert!((e.components.disc.as_ref().unwrap().radius - 22.0).abs() < 1e-4);
}

#[test]
fn variant_from_instance_and_old_prefab_compat() {
    let dir = tmp_game("from-inst");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(5.0),
            y: Some(6.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost.prefab.json"), &base).unwrap();

    let inst = instantiate_prefab(
        &mut scene,
        &base,
        "assets/prefabs/ghost.prefab.json",
        Some(50.0),
        Some(60.0),
    );
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [0, 255, 0, 255];

    let variant = variant_from_instance(&dir, &scene, &inst).unwrap();
    assert!(variant.is_variant());
    assert!(
        variant.overrides.iter().any(|f| f == "Disc.color")
            || variant.overrides.iter().any(|f| f == "transform.position")
    );
    let dest = dir.join("assets/prefabs/ghost_green.prefab.json");
    save_prefab(&dest, &variant).unwrap();

    // Old one-entity prefab still loads.
    let old =
        r#"{"name":"Plain","transform":{"translation":[0.0,0.0,0.0]},"components":{},"tag":0}"#;
    fs::write(dir.join("assets/prefabs/plain.prefab.json"), old).unwrap();
    let plain = load_prefab(&dir.join("assets/prefabs/plain.prefab.json")).unwrap();
    assert!(!plain.is_variant());
    assert!(plain.children.is_empty());

    unpack_prefab_instance(&mut scene, &inst).unwrap();
    assert!(scene.find_entity(&inst).unwrap().prefab.is_none());
}

#[test]
fn apply_field_to_base_removes_override_and_siblings_inherit() {
    let dir = tmp_game("apply-base");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            x: Some(10.0),
            y: Some(20.0),
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();

    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_blue").unwrap();
    variant.entity.components.disc.as_mut().unwrap().color = [0, 0, 255, 255];
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    save_prefab(&variant_path, &variant).unwrap();

    // Sibling with an empty override list still stores the base snapshot.
    let (sib_path, sib) = create_prefab_variant(&dir, "ghost", "ghost_sib").unwrap();
    assert!(sib.overrides.is_empty());

    let report = apply_variant_override_to_base(
        &dir,
        "ghost_blue",
        &["Disc.color".into()],
        ApplyBaseTarget::Immediate,
    )
    .unwrap();
    assert_eq!(report.applied, vec!["Disc.color".to_string()]);
    assert!(
        !report.variant_overrides.iter().any(|p| p == "Disc.color"),
        "{:?}",
        report.variant_overrides
    );

    let base_after = load_prefab(&base_path).unwrap();
    assert_eq!(
        base_after.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );
    let variant_after = load_prefab(&variant_path).unwrap();
    assert!(variant_after.is_variant());
    assert!(!variant_after.overrides.iter().any(|p| p == "Disc.color"));
    let resolved = resolve_prefab(&dir, &variant_after).unwrap();
    assert_eq!(
        resolved.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );

    let sib_after = load_prefab(&sib_path).unwrap();
    let sib_resolved = resolve_prefab(&dir, &sib_after).unwrap();
    assert_eq!(
        sib_resolved.entity.components.disc.as_ref().unwrap().color,
        [0, 0, 255, 255]
    );
}

#[test]
fn apply_nested_child_field_to_base() {
    let dir = tmp_game("apply-nested");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    add_entity(
        &mut scene,
        "Eye",
        &MutateOpts {
            radius: Some(4.0),
            ..Default::default()
        },
    )
    .unwrap();
    set_entity_parent(&mut scene, "Eye", Some("Ghost")).unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost.prefab.json"), &base).unwrap();

    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    variant
        .children
        .iter_mut()
        .find(|c| c.name == "Eye")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 11.0;
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    assert!(
        variant.overrides.iter().any(|p| p == "Eye/Disc.radius"),
        "{:?}",
        variant.overrides
    );
    save_prefab(&variant_path, &variant).unwrap();

    apply_variant_override_to_base(
        &dir,
        "assets/prefabs/ghost_v.prefab.json",
        &["Eye/Disc.radius".into()],
        ApplyBaseTarget::Immediate,
    )
    .unwrap();

    let base_after = load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    let eye = base_after
        .children
        .iter()
        .find(|c| c.name == "Eye")
        .unwrap();
    assert!((eye.components.disc.as_ref().unwrap().radius - 11.0).abs() < 1e-4);
    let variant_after = load_prefab(&variant_path).unwrap();
    assert!(!variant_after
        .overrides
        .iter()
        .any(|p| p == "Eye/Disc.radius"));
    let resolved = resolve_prefab(&dir, &variant_after).unwrap();
    let eye = resolved.children.iter().find(|c| c.name == "Eye").unwrap();
    assert!((eye.components.disc.as_ref().unwrap().radius - 11.0).abs() < 1e-4);
}

#[test]
fn apply_to_root_through_variant_chain() {
    let dir = tmp_game("apply-root");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];
    save_prefab(
        &dir.join("assets/prefabs/ghost.prefab.json"),
        &entity_to_prefab(&scene, "Ghost").unwrap(),
    )
    .unwrap();
    let (_mid_path, mut mid) = create_prefab_variant(&dir, "ghost", "ghost_mid").unwrap();
    mid.entity.tag = 3;
    refresh_variant_overrides(&dir, &mut mid).unwrap();
    save_prefab(&dir.join("assets/prefabs/ghost_mid.prefab.json"), &mid).unwrap();
    let (leaf_path, mut leaf) = create_prefab_variant(&dir, "ghost_mid", "ghost_leaf").unwrap();
    leaf.entity.components.disc.as_mut().unwrap().color = [0, 255, 0, 255];
    refresh_variant_overrides(&dir, &mut leaf).unwrap();
    assert!(leaf.overrides.iter().any(|p| p == "Disc.color"));
    save_prefab(&leaf_path, &leaf).unwrap();

    let chain = prefab_base_chain(&dir, "ghost_leaf").unwrap();
    assert_eq!(chain.len(), 3);

    apply_variant_override_to_base(
        &dir,
        "ghost_leaf",
        &["Disc.color".into()],
        ApplyBaseTarget::Immediate,
    )
    .unwrap();
    let mid_after = load_prefab(&dir.join("assets/prefabs/ghost_mid.prefab.json")).unwrap();
    assert!(mid_after.overrides.iter().any(|p| p == "Disc.color"));
    let root = load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    assert_eq!(
        root.entity.components.disc.as_ref().unwrap().color,
        [255, 0, 0, 255]
    );

    // Fresh chain for root apply.
    let dir = tmp_game("apply-root-2");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];
    save_prefab(
        &dir.join("assets/prefabs/ghost.prefab.json"),
        &entity_to_prefab(&scene, "Ghost").unwrap(),
    )
    .unwrap();
    create_prefab_variant(&dir, "ghost", "ghost_mid").unwrap();
    let (leaf_path, mut leaf) = create_prefab_variant(&dir, "ghost_mid", "ghost_leaf").unwrap();
    leaf.entity.components.disc.as_mut().unwrap().color = [0, 255, 0, 255];
    refresh_variant_overrides(&dir, &mut leaf).unwrap();
    save_prefab(&leaf_path, &leaf).unwrap();

    apply_variant_override_to_base(
        &dir,
        "ghost_leaf",
        &["Disc.color".into()],
        ApplyBaseTarget::Root,
    )
    .unwrap();
    let root = load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    assert_eq!(
        root.entity.components.disc.as_ref().unwrap().color,
        [0, 255, 0, 255]
    );
    let mid = load_prefab(&dir.join("assets/prefabs/ghost_mid.prefab.json")).unwrap();
    assert!(!mid.overrides.iter().any(|p| p == "Disc.color"));
    let leaf = load_prefab(&leaf_path).unwrap();
    assert!(!leaf.overrides.iter().any(|p| p == "Disc.color"));
    let resolved = resolve_prefab(&dir, &leaf).unwrap();
    assert_eq!(
        resolved.entity.components.disc.as_ref().unwrap().color,
        [0, 255, 0, 255]
    );
    let status = prefab_chain_status(&dir, "ghost_leaf").unwrap();
    assert_eq!(status.chain.len(), 3);
    assert!(status.base.unwrap().ends_with("ghost_mid.prefab.json"));
    assert!(status.root.ends_with("ghost.prefab.json"));
}

#[test]
fn apply_instance_value_to_base_and_revert_field_undo() {
    let dir = tmp_game("apply-inst");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == "Ghost")
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [255, 0, 0, 255];
    save_prefab(
        &dir.join("assets/prefabs/ghost.prefab.json"),
        &entity_to_prefab(&scene, "Ghost").unwrap(),
    )
    .unwrap();
    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    variant.entity.components.disc.as_mut().unwrap().color = [0, 0, 255, 255];
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    save_prefab(&variant_path, &variant).unwrap();
    let resolved = resolve_prefab(&dir, &variant).unwrap();
    let inst = instantiate_prefab(
        &mut scene,
        &resolved,
        "assets/prefabs/ghost_v.prefab.json",
        None,
        None,
    );
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .color = [0, 255, 0, 255];

    apply_instance_to_base(
        &dir,
        &scene,
        &inst,
        &["Disc.color".into()],
        ApplyBaseTarget::Immediate,
    )
    .unwrap();
    let base = load_prefab(&dir.join("assets/prefabs/ghost.prefab.json")).unwrap();
    assert_eq!(
        base.entity.components.disc.as_ref().unwrap().color,
        [0, 255, 0, 255]
    );
    let variant = load_prefab(&variant_path).unwrap();
    assert!(!variant.overrides.iter().any(|p| p == "Disc.color"));

    // Instance still has green; revert the field back to the resolved asset (now green too).
    // Change instance off the asset, revert one field, undo the scene mutation.
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == inst)
        .unwrap()
        .components
        .disc
        .as_mut()
        .unwrap()
        .radius = 30.0;
    let before = scene.clone();
    let mut undo = UndoStack::with_default_depth();
    undo.push(&before);
    let resolved = load_prefab_for_instance(&dir, scene.find_entity(&inst).unwrap()).unwrap();
    revert_prefab_instance_fields(&mut scene, &inst, &resolved, &["Disc.radius".into()]).unwrap();
    assert!(
        (scene
            .find_entity(&inst)
            .unwrap()
            .components
            .disc
            .as_ref()
            .unwrap()
            .radius
            - 8.0)
            .abs()
            < 1e-4
    );
    assert!(undo.undo(&mut scene));
    assert!(
        (scene
            .find_entity(&inst)
            .unwrap()
            .components
            .disc
            .as_ref()
            .unwrap()
            .radius
            - 30.0)
            .abs()
            < 1e-4
    );
}

#[test]
fn revert_variant_asset_override_and_old_prefab_unchanged() {
    let dir = tmp_game("revert-asset");
    let mut scene = Scene::new("main");
    add_entity(
        &mut scene,
        "Ghost",
        &MutateOpts {
            radius: Some(8.0),
            ..Default::default()
        },
    )
    .unwrap();
    let base = entity_to_prefab(&scene, "Ghost").unwrap();
    let base_path = dir.join("assets/prefabs/ghost.prefab.json");
    save_prefab(&base_path, &base).unwrap();
    let (variant_path, mut variant) = create_prefab_variant(&dir, "ghost", "ghost_v").unwrap();
    variant.entity.components.disc.as_mut().unwrap().radius = 18.0;
    refresh_variant_overrides(&dir, &mut variant).unwrap();
    save_prefab(&variant_path, &variant).unwrap();

    let err = apply_variant_override_to_base(
        &dir,
        "ghost",
        &["Disc.radius".into()],
        ApplyBaseTarget::Immediate,
    )
    .unwrap_err();
    assert!(err.to_string().contains("not a variant"), "{err}");
    let plain = load_prefab(&base_path).unwrap();
    assert!(!plain.is_variant());
    assert!((plain.entity.components.disc.as_ref().unwrap().radius - 8.0).abs() < 1e-4);

    revert_variant_to_base(&dir, "ghost_v", &["Disc.radius".into()]).unwrap();
    let variant = load_prefab(&variant_path).unwrap();
    assert!(!variant.overrides.iter().any(|p| p == "Disc.radius"));
    let resolved = resolve_prefab(&dir, &variant).unwrap();
    assert!((resolved.entity.components.disc.as_ref().unwrap().radius - 8.0).abs() < 1e-4);
    let base_after = load_prefab(&base_path).unwrap();
    assert!((base_after.entity.components.disc.as_ref().unwrap().radius - 8.0).abs() < 1e-4);
}

#[test]
fn prefab_variant_chrome_labels() {
    let c = prefab_variant_chrome("ghost_v", "ghost", None);
    assert_eq!(c.open_base, "Open Base");
    assert_eq!(c.select_base, "Select Base");
    assert_eq!(c.apply_all_to_base, "Apply All to Base");
    assert_eq!(c.apply_to_base, "Apply to Base 'ghost'");
    assert_eq!(c.apply_to_variant, "Apply to Prefab Variant 'ghost_v'");
    assert_eq!(
        c.apply_all_to_variant,
        "Apply All to Prefab Variant 'ghost_v'"
    );
    assert_eq!(c.revert, "Revert");
    assert_eq!(c.revert_all, "Revert All");
    assert_eq!(c.revert_to_base, "Revert to Base");
    assert!(c.apply_to_root.is_none());
    let deep = prefab_variant_chrome("leaf", "mid", Some("root"));
    assert_eq!(deep.apply_to_root.as_deref(), Some("Apply to Root 'root'"));
    assert_eq!(
        deep.apply_all_to_root.as_deref(),
        Some("Apply All to Root 'root'")
    );
}

#[test]
fn normalize_variant_link() {
    assert_eq!(
        normalize_prefab_source("ghost_blue"),
        "assets/prefabs/ghost_blue.prefab.json"
    );
}
