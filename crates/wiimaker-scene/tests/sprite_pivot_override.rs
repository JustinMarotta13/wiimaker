//! Component-level Sprite pivot override: hydrate, animate, JSON, bake.

use wiimaker_assets::{ResolvedSprite, SpriteCatalog};
use wiimaker_core::draw::TextureId;
use wiimaker_scene::{
    add_component_sprite, add_entity, animate_world, bake_scene_wscn_with_catalog,
    hydrate_with_catalog, load_scene, save_scene, set_entity_sprite_pivot, MutateOpts, Scene,
    TextureMap, KIND_SPRITE, WSCN_MAGIC,
};

fn catalog_hero() -> SpriteCatalog {
    let mut cat = SpriteCatalog::empty();
    cat.insert(
        "hero_2",
        ResolvedSprite {
            sheet_texture: "sheet".into(),
            uv: [0.1, 0.2, 0.3, 0.4],
            pivot: [0.5, 0.5],
            pixel_size: [16.0, 16.0],
            is_cell: true,
        },
    );
    cat.insert(
        "hero_3",
        ResolvedSprite {
            sheet_texture: "sheet".into(),
            uv: [0.5, 0.2, 0.3, 0.4],
            pivot: [0.25, 0.75],
            pixel_size: [16.0, 16.0],
            is_cell: true,
        },
    );
    cat
}

fn textures() -> TextureMap {
    let mut t = TextureMap::new();
    t.insert("sheet", TextureId(0));
    t
}

#[test]
fn hydrate_uses_catalog_pivot_when_omitted() {
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut scene, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    let world = hydrate_with_catalog(&scene, &textures(), Some(&catalog_hero())).unwrap();
    let id = world.find_by_name("Hero").unwrap();
    let sp = world.sprite(id).unwrap();
    assert!((sp.pivot.x - 0.5).abs() < 1e-4);
    assert!((sp.pivot.y - 0.5).abs() < 1e-4);
    assert!(!sp.lock_pivot);
}

#[test]
fn hydrate_uses_component_pivot_override() {
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut scene, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    set_entity_sprite_pivot(&mut scene, "Hero", Some(0.0), Some(1.0), false).unwrap();
    let world = hydrate_with_catalog(&scene, &textures(), Some(&catalog_hero())).unwrap();
    let id = world.find_by_name("Hero").unwrap();
    let sp = world.sprite(id).unwrap();
    assert!((sp.pivot.x - 0.0).abs() < 1e-4);
    assert!((sp.pivot.y - 1.0).abs() < 1e-4);
    assert!(sp.lock_pivot);
}

#[test]
fn json_omits_pivot_unless_overridden() {
    let dir = std::env::temp_dir().join(format!("wiimaker-pivot-json-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut scene, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    let path = dir.join("main.scene.json");
    save_scene(&path, &scene).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("pivot"), "{text}");
    set_entity_sprite_pivot(&mut scene, "Hero", Some(0.0), Some(1.0), false).unwrap();
    save_scene(&path, &scene).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"pivot\""), "{text}");
    let loaded = load_scene(&path).unwrap();
    assert_eq!(
        loaded
            .find_entity("Hero")
            .unwrap()
            .components
            .sprite
            .as_ref()
            .unwrap()
            .pivot,
        Some([0.0, 1.0])
    );
}

#[test]
fn animate_preserves_override_and_swaps_catalog_when_unset() {
    let cat = catalog_hero();
    let tex = textures();

    let mut with_ov = Scene::new("main");
    add_entity(&mut with_ov, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut with_ov, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    set_entity_sprite_pivot(&mut with_ov, "Hero", Some(0.0), Some(1.0), false).unwrap();
    let mut world = hydrate_with_catalog(&with_ov, &tex, Some(&cat)).unwrap();
    let id = world.find_by_name("Hero").unwrap();
    world.set_animation(
        id,
        Some(wiimaker_core::world::Animation::new(
            "walk",
            vec!["hero_2".into(), "hero_3".into()],
            10.0,
            true,
        )),
    );
    animate_world(&mut world, &cat, &tex, 0.11);
    let sp = world.sprite(id).unwrap();
    assert!((sp.pivot.x - 0.0).abs() < 1e-4 && (sp.pivot.y - 1.0).abs() < 1e-4);
    assert_eq!(world.animation(id).unwrap().frame, 1);

    let mut no_ov = Scene::new("main");
    add_entity(&mut no_ov, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut no_ov, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    let mut world = hydrate_with_catalog(&no_ov, &tex, Some(&cat)).unwrap();
    let id = world.find_by_name("Hero").unwrap();
    world.set_animation(
        id,
        Some(wiimaker_core::world::Animation::new(
            "walk",
            vec!["hero_2".into(), "hero_3".into()],
            10.0,
            true,
        )),
    );
    animate_world(&mut world, &cat, &tex, 0.11);
    let sp = world.sprite(id).unwrap();
    assert!((sp.pivot.x - 0.25).abs() < 1e-4 && (sp.pivot.y - 0.75).abs() < 1e-4);
}

#[test]
fn bake_uses_effective_pivot_without_bumping_magic() {
    let mut scene = Scene::new("main");
    add_entity(&mut scene, "Hero", &MutateOpts::default()).unwrap();
    add_component_sprite(&mut scene, "Hero", "hero_2", [16.0, 16.0]).unwrap();
    let mut pack = wiimaker_assets::WPack::new();
    pack.textures.push(wiimaker_assets::PackedTexture {
        name: "sheet".into(),
        width: 8,
        height: 8,
        rgba16: vec![0; 128],
    });
    let cat = catalog_hero();
    let bytes = bake_scene_wscn_with_catalog(&scene, &pack, Some(&cat)).unwrap();
    assert_eq!(&bytes[0..8], WSCN_MAGIC);
    let catalog_pivot = read_sprite_pivot(&bytes);
    assert!((catalog_pivot[0] - 0.5).abs() < 1e-4);
    assert!((catalog_pivot[1] - 0.5).abs() < 1e-4);

    set_entity_sprite_pivot(&mut scene, "Hero", Some(0.0), Some(1.0), false).unwrap();
    let bytes = bake_scene_wscn_with_catalog(&scene, &pack, Some(&cat)).unwrap();
    assert_eq!(&bytes[0..8], WSCN_MAGIC);
    let ov = read_sprite_pivot(&bytes);
    assert!((ov[0] - 0.0).abs() < 1e-4);
    assert!((ov[1] - 1.0).abs() < 1e-4);
}

fn read_sprite_pivot(bytes: &[u8]) -> [f32; 2] {
    let mut i = 8 + 4 + 4;
    let nlen = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    i += 2 + nlen + 6 * 4;
    assert_eq!(bytes[i], KIND_SPRITE);
    i += 1 + 2 + 8 + 16;
    [
        f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()),
        f32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()),
    ]
}
