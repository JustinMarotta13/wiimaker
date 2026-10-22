//! Scene-view visibility and pickability.
//!
//! Stored in [`crate::prefs::SceneVisibilityPrefs`] (`.wiimaker/prefs.toml`),
//! not in the scene file. Hidden entities are omitted from the Scene-view
//! draw only. Game view, editor Play, host play, and Wii bake keep calling
//! [`wiimaker_core::render_world`] on the untouched world.
//!
//! Unpickable entities (and hidden ones, while the toolbar toggle is on) are
//! skipped by Scene-view point pick and box pick. Hierarchy selection is
//! unaffected.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{bail, Result};
use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::World;

use crate::prefs::{normalize_project_rel, SceneVisibilityPrefs, SceneVisibilityScene};
use crate::scene::Scene;

/// `On` = visible or pickable. `Off` = hidden or unpickable. `Mixed` = the
/// entity and its descendants disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState {
    On,
    Off,
    Mixed,
}

/// Scene path relative to the game directory (`scenes/main.scene.json`).
pub fn scene_visibility_key(game_dir: &Path, scene_path: &Path) -> String {
    match scene_path.strip_prefix(game_dir) {
        Ok(rel) => normalize_project_rel(&rel.to_string_lossy()),
        Err(_) => {
            normalize_project_rel(&scene_path.file_name().unwrap_or_default().to_string_lossy())
        }
    }
}

fn branch_names(scene: &Scene, name: &str, recursive: bool) -> Vec<String> {
    let mut out = vec![name.to_string()];
    if !recursive {
        return out;
    }
    let mut i = 0;
    while i < out.len() {
        let current = out[i].clone();
        for child in scene.child_names(&current) {
            if !out.iter().any(|n| n == &child) {
                out.push(child);
            }
        }
        i += 1;
    }
    out
}

fn listed(list: &[String], name: &str) -> bool {
    list.iter().any(|n| n == name)
}

fn set_listed(list: &mut Vec<String>, name: &str, on: bool) {
    if on {
        if !listed(list, name) {
            list.push(name.to_string());
        }
    } else {
        list.retain(|n| n != name);
    }
}

fn entry_mut<'a>(vis: &'a mut SceneVisibilityPrefs, key: &str) -> &'a mut SceneVisibilityScene {
    if let Some(i) = vis.scenes.iter().position(|s| s.scene == key) {
        return &mut vis.scenes[i];
    }
    vis.scenes.push(SceneVisibilityScene {
        scene: key.to_string(),
        hidden: Vec::new(),
        unpickable: Vec::new(),
    });
    vis.scenes.last_mut().expect("just pushed")
}

fn finish_entry(vis: &mut SceneVisibilityPrefs, key: &str) {
    if let Some(i) = vis.scenes.iter().position(|s| s.scene == key) {
        vis.scenes[i].hidden.sort();
        vis.scenes[i].hidden.dedup();
        vis.scenes[i].unpickable.sort();
        vis.scenes[i].unpickable.dedup();
        if vis.scenes[i].hidden.is_empty() && vis.scenes[i].unpickable.is_empty() {
            vis.scenes.remove(i);
        }
    }
    vis.scenes.sort_by(|a, b| a.scene.cmp(&b.scene));
}

/// Set hidden and/or pickable on `entity`. `recursive` writes the same flags
/// onto descendants (Unity parent click). `recursive == false` is Alt-click.
///
/// `hidden`: `Some(true)` hides, `Some(false)` shows. `pickable`: `Some(true)`
/// makes the entity pickable. Returns the entity names that were written.
pub fn apply_entity_visibility(
    vis: &mut SceneVisibilityPrefs,
    scene_key: &str,
    scene: &Scene,
    entity: &str,
    hidden: Option<bool>,
    pickable: Option<bool>,
    recursive: bool,
) -> Result<Vec<String>> {
    if hidden.is_none() && pickable.is_none() {
        bail!("set-visibility needs --hidden and/or --pickable");
    }
    if scene.find_entity(entity).is_none() {
        bail!("entity '{entity}' not found");
    }
    let names = branch_names(scene, entity, recursive);
    {
        let entry = entry_mut(vis, scene_key);
        for name in &names {
            if let Some(h) = hidden {
                set_listed(&mut entry.hidden, name, h);
            }
            if let Some(p) = pickable {
                // Stored as the unpickable list. pickable on → drop the name.
                set_listed(&mut entry.unpickable, name, !p);
            }
        }
    }
    finish_entry(vis, scene_key);
    let mut affected = names;
    affected.sort();
    Ok(affected)
}

pub fn entity_is_hidden(vis: &SceneVisibilityPrefs, scene_key: &str, name: &str) -> bool {
    vis.scene(scene_key)
        .is_some_and(|e| listed(&e.hidden, name))
}

pub fn entity_is_unpickable(vis: &SceneVisibilityPrefs, scene_key: &str, name: &str) -> bool {
    vis.scene(scene_key)
        .is_some_and(|e| listed(&e.unpickable, name))
}

fn tri_for(scene: &Scene, name: &str, flagged: impl Fn(&str) -> bool) -> TriState {
    let names = branch_names(scene, name, true);
    let mut on = 0usize;
    let mut off = 0usize;
    for n in &names {
        if scene.find_entity(n).is_none() {
            continue;
        }
        if flagged(n) {
            off += 1;
        } else {
            on += 1;
        }
    }
    match (on, off) {
        (_, 0) => TriState::On,
        (0, _) => TriState::Off,
        _ => TriState::Mixed,
    }
}

/// Visible / hidden / mixed across `name` and its descendants.
pub fn visibility_tri(
    vis: &SceneVisibilityPrefs,
    scene_key: &str,
    scene: &Scene,
    name: &str,
) -> TriState {
    tri_for(scene, name, |n| entity_is_hidden(vis, scene_key, n))
}

/// Pickable / unpickable / mixed across `name` and its descendants.
pub fn pickability_tri(
    vis: &SceneVisibilityPrefs,
    scene_key: &str,
    scene: &Scene,
    name: &str,
) -> TriState {
    tri_for(scene, name, |n| entity_is_unpickable(vis, scene_key, n))
}

/// Names the Scene view should skip when drawing. Empty when the toolbar
/// toggle is off. Unknown names are ignored.
pub fn scene_view_hidden_names(
    vis: &SceneVisibilityPrefs,
    scene_key: &str,
    scene: &Scene,
) -> HashSet<String> {
    let mut out = HashSet::new();
    if !vis.enabled {
        return out;
    }
    let Some(entry) = vis.scene(scene_key) else {
        return out;
    };
    for name in &entry.hidden {
        if scene.find_entity(name).is_some() {
            out.insert(name.clone());
        }
    }
    out
}

/// Names Scene-view click and box pick must skip.
///
/// Unpickable always. Hidden only while the toolbar toggle is on (those
/// entities are not drawn, so they are not pickable either). Unknown names
/// are ignored.
pub fn scene_view_pick_blocklist(
    vis: &SceneVisibilityPrefs,
    scene_key: &str,
    scene: &Scene,
) -> HashSet<String> {
    let mut out = scene_view_hidden_names(vis, scene_key, scene);
    let Some(entry) = vis.scene(scene_key) else {
        return out;
    };
    for name in &entry.unpickable {
        if scene.find_entity(name).is_some() {
            out.insert(name.clone());
        }
    }
    out
}

/// Follow a rename so the prefs key stays on the same entity.
pub fn rename_scene_visibility(
    vis: &mut SceneVisibilityPrefs,
    scene_key: &str,
    old: &str,
    new: &str,
) {
    if old == new {
        return;
    }
    {
        let Some(entry) = vis.scene_mut(scene_key) else {
            return;
        };
        for list in [&mut entry.hidden, &mut entry.unpickable] {
            for name in list.iter_mut() {
                if name == old {
                    *name = new.to_string();
                }
            }
            list.sort();
            list.dedup();
        }
    }
    finish_entry(vis, scene_key);
}

/// Drop names removed from the scene (the entity and its former descendants).
pub fn forget_scene_visibility(vis: &mut SceneVisibilityPrefs, scene_key: &str, names: &[String]) {
    {
        let Some(entry) = vis.scene_mut(scene_key) else {
            return;
        };
        entry.hidden.retain(|n| !names.iter().any(|k| k == n));
        entry.unpickable.retain(|n| !names.iter().any(|k| k == n));
    }
    finish_entry(vis, scene_key);
}

/// Deactivate `hidden` entities on a Scene-view world clone.
///
/// [`crate::render_world_ex`] skips inactive entities. The caller's world is
/// not modified. Game view must not call this.
pub fn hide_entities_for_scene_view(world: &mut World, hidden: &HashSet<String>) {
    if hidden.is_empty() {
        return;
    }
    let ids: Vec<_> = world.iter_entities().collect();
    for id in ids {
        if world.name(id).is_some_and(|n| hidden.contains(n)) {
            world.set_active(id, false);
        }
    }
}

/// Scene-view draw list: clone `world`, deactivate `hidden`, render without
/// the camera offset (same as the editor Scene tab).
pub fn render_scene_view(world: &World, clear: Rgba8, hidden: &HashSet<String>) -> DrawList {
    let mut clone = world.clone();
    hide_entities_for_scene_view(&mut clone, hidden);
    let mut draw = DrawList::new();
    wiimaker_core::render_world_ex(&clone, &mut draw, clear, false);
    draw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::{load_editor_prefs, save_editor_prefs, EditorPrefs};
    use crate::scene::{EntityData, SceneComponents, SceneDisc, SceneSprite, SceneTransform};
    use wiimaker_core::color::Rgba8;
    use wiimaker_core::draw::DrawCmd;
    use wiimaker_core::world::{Disc, World};

    fn sprite(name: &str, parent: Option<&str>, x: f32, y: f32) -> EntityData {
        EntityData {
            name: name.into(),
            parent: parent.map(|p| p.to_string()),
            transform: SceneTransform::from_xy(x, y),
            components: SceneComponents {
                sprite: Some(SceneSprite {
                    texture: "t".into(),
                    size: [40.0, 40.0],
                    color: [255, 255, 255, 255],
                    z: 0.0,
                    sorting_layer: String::new(),
                    enabled: true,
                    pivot: None,
                }),
                ..Default::default()
            },
            tag: 0,
            prefab: None,
        }
    }

    fn family() -> (Scene, &'static str) {
        let mut scene = Scene::new("main");
        scene.entities.push(sprite("Parent", None, 100.0, 100.0));
        scene
            .entities
            .push(sprite("Child", Some("Parent"), 180.0, 100.0));
        scene.entities.push(sprite("Other", None, 300.0, 100.0));
        (scene, "scenes/main.scene.json")
    }

    fn world_with(names: &[(&str, f32, f32)]) -> World {
        let mut world = World::new();
        for (name, x, y) in names {
            let id = world.spawn_named(*name, wiimaker_core::world::Transform::from_xy(*x, *y));
            world.set_disc(id, Some(Disc::new(10.0, Rgba8::rgb(200, 40, 40))));
        }
        world
    }

    fn disc_centers(draw: &DrawList) -> Vec<[f32; 2]> {
        draw.cmds()
            .iter()
            .filter_map(|c| match c {
                DrawCmd::DrawDisc { center, .. } => Some([center.x, center.y]),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn recursive_parent_hide_and_isolate() {
        let (scene, key) = family();
        let mut vis = SceneVisibilityPrefs::default();
        let affected =
            apply_entity_visibility(&mut vis, key, &scene, "Parent", Some(true), None, true)
                .unwrap();
        assert_eq!(affected, vec!["Child".to_string(), "Parent".to_string()]);
        let hidden = scene_view_hidden_names(&vis, key, &scene);
        assert!(hidden.contains("Parent") && hidden.contains("Child"));
        assert!(!hidden.contains("Other"));
        assert_eq!(visibility_tri(&vis, key, &scene, "Parent"), TriState::Off);

        apply_entity_visibility(&mut vis, key, &scene, "Child", Some(false), None, false).unwrap();
        assert!(!entity_is_hidden(&vis, key, "Child"));
        assert!(entity_is_hidden(&vis, key, "Parent"));
        assert_eq!(visibility_tri(&vis, key, &scene, "Parent"), TriState::Mixed);
    }

    #[test]
    fn hidden_entity_skipped_by_scene_draw_filter() {
        let (mut scene, key) = family();
        scene.entities[0].components.sprite = None;
        scene.entities[0].components.disc = Some(SceneDisc {
            radius: 10.0,
            color: [200, 40, 40, 255],
            z: 0.0,
            sorting_layer: String::new(),
            enabled: true,
        });
        let mut vis = SceneVisibilityPrefs::default();
        apply_entity_visibility(&mut vis, key, &scene, "Parent", Some(true), None, false).unwrap();
        let world = world_with(&[("Parent", 40.0, 50.0), ("Other", 80.0, 50.0)]);
        let hidden = scene_view_hidden_names(&vis, key, &scene);
        let filtered = render_scene_view(&world, Rgba8::BLACK, &hidden);
        let centers = disc_centers(&filtered);
        assert_eq!(centers.len(), 1);
        assert!((centers[0][0] - 80.0).abs() < 1e-3);

        let mut full = DrawList::new();
        wiimaker_core::render_world_ex(&world, &mut full, Rgba8::BLACK, false);
        assert_eq!(
            disc_centers(&full).len(),
            2,
            "source world stays fully drawn"
        );
        assert!(world.is_active(world.find_by_name("Parent").unwrap()));
    }

    #[test]
    fn global_toggle_off_shows_everything() {
        let (scene, key) = family();
        let mut vis = SceneVisibilityPrefs::default();
        apply_entity_visibility(&mut vis, key, &scene, "Parent", Some(true), None, true).unwrap();
        vis.enabled = false;
        let hidden = scene_view_hidden_names(&vis, key, &scene);
        assert!(hidden.is_empty());
        let world = world_with(&[("Parent", 10.0, 10.0), ("Child", 30.0, 10.0)]);
        let draw = render_scene_view(&world, Rgba8::BLACK, &hidden);
        assert_eq!(disc_centers(&draw).len(), 2);
        assert!(vis.scene(key).unwrap().hidden.contains(&"Parent".into()));
    }

    #[test]
    fn unpickable_skipped_by_point_and_box_pick() {
        let (scene, key) = family();
        let mut vis = SceneVisibilityPrefs::default();
        apply_entity_visibility(&mut vis, key, &scene, "Parent", None, Some(false), true).unwrap();
        assert!(entity_is_unpickable(&vis, key, "Parent"));
        assert!(entity_is_unpickable(&vis, key, "Child"));
        assert!(!entity_is_unpickable(&vis, key, "Other"));
        let skip = scene_view_pick_blocklist(&vis, key, &scene);
        assert_eq!(
            crate::pick::pick_entity_at_filtered(&scene, 100.0, 100.0, None, &[], Some(&skip))
                .as_deref(),
            None
        );
        assert_eq!(
            crate::pick::pick_entity_at_filtered(&scene, 300.0, 100.0, None, &[], Some(&skip))
                .as_deref(),
            Some("Other")
        );
        let boxed =
            crate::pick::pick_entities_in_rect(&scene, [0.0, 0.0, 400.0, 400.0], None, Some(&skip));
        assert_eq!(boxed, vec!["Other".to_string()]);
        assert!(scene_view_hidden_names(&vis, key, &scene).is_empty());
    }

    #[test]
    fn hidden_is_not_scene_pickable_until_toggle_off() {
        let (scene, key) = family();
        let mut vis = SceneVisibilityPrefs::default();
        apply_entity_visibility(&mut vis, key, &scene, "Other", Some(true), None, false).unwrap();
        let skip = scene_view_pick_blocklist(&vis, key, &scene);
        assert!(skip.contains("Other"));
        vis.enabled = false;
        let skip = scene_view_pick_blocklist(&vis, key, &scene);
        assert!(!skip.contains("Other"));
    }

    #[test]
    fn unknown_names_are_ignored_and_rename_forgets() {
        let (scene, key) = family();
        let mut vis = SceneVisibilityPrefs::default();
        vis.scenes.push(SceneVisibilityScene {
            scene: key.into(),
            hidden: vec!["Nope".into(), "Other".into()],
            unpickable: vec!["Gone".into()],
        });
        let hidden = scene_view_hidden_names(&vis, key, &scene);
        assert_eq!(hidden.len(), 1);
        assert!(hidden.contains("Other"));
        assert!(scene_view_pick_blocklist(&vis, key, &scene).contains("Other"));
        assert!(!scene_view_pick_blocklist(&vis, key, &scene).contains("Gone"));

        rename_scene_visibility(&mut vis, key, "Other", "Renamed");
        assert!(entity_is_hidden(&vis, key, "Renamed"));
        assert!(!entity_is_hidden(&vis, key, "Other"));

        forget_scene_visibility(&mut vis, key, &["Renamed".into(), "Nope".into()]);
        let left = vis.scene(key).unwrap();
        assert!(left.hidden.is_empty());
        assert_eq!(left.unpickable, vec!["Gone".to_string()]);
        forget_scene_visibility(&mut vis, key, &["Gone".into()]);
        assert!(vis.scene(key).is_none());
    }

    #[test]
    fn prefs_roundtrip_and_old_file_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "wiimaker-vis-prefs-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".wiimaker")).unwrap();
        std::fs::write(
            dir.join(".wiimaker/prefs.toml"),
            "[game_view]\nscale = 1.0\n",
        )
        .unwrap();
        let old = load_editor_prefs(&dir).unwrap();
        assert!(old.scene_visibility.enabled);
        assert!(old.scene_visibility.scenes.is_empty());

        let (scene, key) = family();
        let mut prefs = EditorPrefs::default();
        apply_entity_visibility(
            &mut prefs.scene_visibility,
            key,
            &scene,
            "Parent",
            Some(true),
            Some(false),
            true,
        )
        .unwrap();
        prefs.scene_visibility.enabled = false;
        save_editor_prefs(&dir, &prefs).unwrap();
        let loaded = load_editor_prefs(&dir).unwrap();
        assert!(!loaded.scene_visibility.enabled);
        let entry = loaded.scene_visibility.scene(key).unwrap();
        assert_eq!(
            entry.hidden,
            vec!["Child".to_string(), "Parent".to_string()]
        );
        assert_eq!(
            entry.unpickable,
            vec!["Child".to_string(), "Parent".to_string()]
        );
        let text = std::fs::read_to_string(dir.join(".wiimaker/prefs.toml")).unwrap();
        assert!(text.contains("[scene_visibility]"), "{text}");
        assert!(text.contains("scenes/main.scene.json"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
