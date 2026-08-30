//! Collider scene mutations + overlap queries (CLI/editor twin).

use anyhow::{bail, Result};
use wiimaker_core::{overlapping, overlaps, triggers_entered};

use crate::hydrate::{hydrate_lenient, TextureMap};
use crate::scene::{Scene, SceneCollider, SceneColliderKind};

fn find_mut<'a>(scene: &'a mut Scene, name: &str) -> Result<&'a mut crate::scene::EntityData> {
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))
}

pub fn add_component_collider(
    scene: &mut Scene,
    name: &str,
    kind: SceneColliderKind,
    size: [f32; 2],
    radius: f32,
    solid: bool,
    trigger: bool,
    filter_tag: u32,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    let mut c = match kind {
        SceneColliderKind::Aabb => SceneCollider::aabb(size[0], size[1]),
        SceneColliderKind::Circle => SceneCollider::circle(radius),
    };
    c.solid = solid;
    c.trigger = trigger;
    c.filter_tag = filter_tag;
    ent.components.collider = Some(c);
    Ok(())
}

pub fn remove_component_collider(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.collider.is_none() {
        bail!("entity '{name}' has no Collider");
    }
    ent.components.collider = None;
    Ok(())
}

/// Pairwise overlap after hydrating world-space transforms (parented).
pub fn entities_overlap(scene: &Scene, a: &str, b: &str) -> Result<bool> {
    let world = hydrate_lenient(scene, &TextureMap::new());
    let aid = world
        .find_by_name(a)
        .ok_or_else(|| anyhow::anyhow!("entity '{a}' not found"))?;
    let bid = world
        .find_by_name(b)
        .ok_or_else(|| anyhow::anyhow!("entity '{b}' not found"))?;
    Ok(overlaps(&world, aid, bid))
}

/// Names of entities whose colliders overlap `name`.
pub fn entity_overlaps(scene: &Scene, name: &str) -> Result<Vec<String>> {
    let world = hydrate_lenient(scene, &TextureMap::new());
    let id = world
        .find_by_name(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    Ok(overlapping(&world, id)
        .into_iter()
        .filter_map(|oid| world.name(oid).map(|s| s.to_string()))
        .collect())
}

/// Names of trigger overlaps for `name` (see [`triggers_entered`]).
pub fn entity_triggers_entered(scene: &Scene, name: &str) -> Result<Vec<String>> {
    let world = hydrate_lenient(scene, &TextureMap::new());
    let id = world
        .find_by_name(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    Ok(triggers_entered(&world, id)
        .into_iter()
        .filter_map(|oid| world.name(oid).map(|s| s.to_string()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hydrate::{hydrate, TextureMap};
    use crate::mutate::{add_entity, MutateOpts};
    use crate::scene::Scene;
    use wiimaker_core::math::Vec2;
    use wiimaker_core::move_and_collide;

    fn scene_with_walls() -> Scene {
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Player",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                radius: Some(6.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_entity(
            &mut scene,
            "Wall",
            &MutateOpts {
                x: Some(40.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_collider(
            &mut scene,
            "Player",
            SceneColliderKind::Aabb,
            [12.0, 12.0],
            6.0,
            true,
            false,
            0,
        )
        .unwrap();
        add_component_collider(
            &mut scene,
            "Wall",
            SceneColliderKind::Aabb,
            [16.0, 48.0],
            8.0,
            true,
            false,
            0,
        )
        .unwrap();
        scene
    }

    #[test]
    fn add_remove_collider() {
        let mut scene = Scene::new("t");
        add_entity(&mut scene, "A", &MutateOpts::default()).unwrap();
        add_component_collider(
            &mut scene,
            "A",
            SceneColliderKind::Aabb,
            [32.0, 16.0],
            8.0,
            true,
            false,
            0,
        )
        .unwrap();
        let c = scene
            .find_entity("A")
            .unwrap()
            .components
            .collider
            .as_ref()
            .unwrap();
        assert_eq!(c.kind, SceneColliderKind::Aabb);
        assert_eq!(c.size, [32.0, 16.0]);
        assert!(c.solid);
        remove_component_collider(&mut scene, "A").unwrap();
        assert!(scene
            .find_entity("A")
            .unwrap()
            .components
            .collider
            .is_none());
        assert!(remove_component_collider(&mut scene, "A").is_err());
    }

    #[test]
    fn json_roundtrip_preserves_collider() {
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Wall",
            &MutateOpts {
                x: Some(10.0),
                y: Some(20.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_collider(
            &mut scene,
            "Wall",
            SceneColliderKind::Circle,
            [0.0, 0.0],
            14.0,
            false,
            false,
            0,
        )
        .unwrap();
        {
            let c = scene
                .entities
                .iter_mut()
                .find(|e| e.name == "Wall")
                .unwrap()
                .components
                .collider
                .as_mut()
                .unwrap();
            c.offset = [2.0, -1.0];
        }
        let text = serde_json::to_string_pretty(&scene).unwrap();
        assert!(text.contains("\"Collider\""));
        assert!(text.contains("\"Circle\""));
        let loaded: Scene = serde_json::from_str(&text).unwrap();
        let c = loaded
            .find_entity("Wall")
            .unwrap()
            .components
            .collider
            .as_ref()
            .unwrap();
        assert_eq!(c.kind, SceneColliderKind::Circle);
        assert_eq!(c.radius, 14.0);
        assert!(!c.solid);
        assert_eq!(c.offset, [2.0, -1.0]);
    }

    #[test]
    fn hydrate_overlap_query() {
        let scene = scene_with_walls();
        assert!(!entities_overlap(&scene, "Player", "Wall").unwrap());
        let mut close = scene.clone();
        close.entities[0].transform.translation[0] = 30.0;
        assert!(entities_overlap(&close, "Player", "Wall").unwrap());
        let names = entity_overlaps(&close, "Player").unwrap();
        assert_eq!(names, vec!["Wall".to_string()]);
    }

    #[test]
    fn hydrate_then_move_and_collide() {
        let scene = scene_with_walls();
        let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
        let player = world.find_by_name("Player").unwrap();
        let wall = world.find_by_name("Wall").unwrap();
        let hit = move_and_collide(&mut world, player, Vec2::new(40.0, 0.0));
        assert_eq!(hit.hit, Some(wall));
        let x = world.transform(player).unwrap().translation.x;
        // Player half 6, wall half 8, wall at 40 → contact ~26
        assert!(x > 20.0 && x < 26.1, "got {x}");
    }

    #[test]
    fn json_roundtrip_preserves_trigger_and_filter() {
        let mut scene = Scene::new("t");
        add_entity(&mut scene, "Dot", &MutateOpts::default()).unwrap();
        add_component_collider(
            &mut scene,
            "Dot",
            SceneColliderKind::Circle,
            [0.0, 0.0],
            8.0,
            false,
            true,
            7,
        )
        .unwrap();
        let text = serde_json::to_string_pretty(&scene).unwrap();
        assert!(text.contains("\"trigger\": true"));
        assert!(text.contains("\"filter_tag\": 7"));
        let loaded: Scene = serde_json::from_str(&text).unwrap();
        let c = loaded
            .find_entity("Dot")
            .unwrap()
            .components
            .collider
            .as_ref()
            .unwrap();
        assert!(c.trigger);
        assert_eq!(c.filter_tag, 7);
        assert!(!c.solid);
    }

    #[test]
    fn triggers_entered_and_pass_through() {
        let mut scene = Scene::new("t");
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
        add_entity(
            &mut scene,
            "Dot",
            &MutateOpts {
                x: Some(20.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_collider(
            &mut scene,
            "Player",
            SceneColliderKind::Aabb,
            [12.0, 12.0],
            6.0,
            true,
            false,
            0,
        )
        .unwrap();
        add_component_collider(
            &mut scene,
            "Dot",
            SceneColliderKind::Aabb,
            [12.0, 12.0],
            6.0,
            true,
            true,
            0,
        )
        .unwrap();
        // Move player onto Dot via hydrate + move_and_collide — trigger must not block.
        let mut world = hydrate(&scene, &TextureMap::new()).unwrap();
        let player = world.find_by_name("Player").unwrap();
        let hit = move_and_collide(&mut world, player, Vec2::new(20.0, 0.0));
        assert!(hit.hit.is_none());
        assert!((world.transform(player).unwrap().translation.x - 20.0).abs() < 1e-3);

        scene.entities[0].transform.translation[0] = 20.0;
        let names = entity_triggers_entered(&scene, "Player").unwrap();
        assert_eq!(names, vec!["Dot".to_string()]);
        assert!(entity_overlaps(&scene, "Player").unwrap().contains(&"Dot".to_string()));
    }
}
