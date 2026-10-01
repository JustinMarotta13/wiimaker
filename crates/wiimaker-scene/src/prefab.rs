//! Prefab instance links + override detection (Unity analogue, nested prefabs).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use crate::scene::{
    display_sorting_layer, EntityData, Prefab, Scene, SceneAudioSource, SceneCamera, SceneCollider,
    SceneComponents, SceneDisc, SceneGridMover, SceneSprite, SceneText, SceneTilemap,
    SceneTransform,
};

const EPS: f32 = 1e-4;

/// Normalize a stem or path to a stable game-relative `*.prefab.json`.
///
/// `"ghost"` → `assets/prefabs/ghost.prefab.json`  
/// `"assets/prefabs/ghost.prefab.json"` is kept (slashes normalized).
pub fn normalize_prefab_source(source: &str) -> String {
    let s = source.replace('\\', "/");
    let s = s.trim().trim_start_matches("./");
    if s.is_empty() {
        return String::new();
    }
    if s.ends_with(".prefab.json") {
        if s.contains('/') {
            return s.to_string();
        }
        return format!("assets/prefabs/{s}");
    }
    format!("assets/prefabs/{s}.prefab.json")
}

/// Display stem (`ghost` from `assets/prefabs/ghost.prefab.json`).
pub fn prefab_stem(source: &str) -> String {
    let s = source.replace('\\', "/");
    let name = s.rsplit('/').next().unwrap_or(source);
    name.strip_suffix(".prefab.json")
        .unwrap_or(name)
        .to_string()
}

/// Resolve a stem or relative path to an existing prefab file under `game_dir`.
pub fn resolve_prefab_asset(game_dir: &Path, source: &str) -> Result<PathBuf> {
    let rel = normalize_prefab_source(source);
    let candidates = [
        game_dir.join(&rel),
        game_dir.join(source),
        game_dir.join("assets").join("prefabs").join(source),
    ];
    for p in &candidates {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    bail!(
        "prefab not found: {source} (tried {})",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Load the prefab asset linked on `entity`.
pub fn load_prefab_for_instance(game_dir: &Path, entity: &EntityData) -> Result<Prefab> {
    let src = entity
        .prefab
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("entity '{}' is not a prefab instance", entity.name))?;
    crate::scene::load_prefab(&resolve_prefab_asset(game_dir, src)?)
}

/// Overridden property paths vs the prefab asset (`transform.position`, `Disc.radius`, …).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PrefabOverrides {
    pub fields: Vec<String>,
}

impl PrefabOverrides {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.fields.iter().any(|f| f == key)
    }

    /// Override key for a root field (`key`) or nested child (`ChildName/key`).
    pub fn path(child: Option<&str>, key: &str) -> String {
        match child {
            Some(c) if !c.is_empty() => format!("{c}/{key}"),
            _ => key.to_string(),
        }
    }

    /// True if `key` (optionally under prefab-local `child`) is overridden.
    pub fn has(&self, child: Option<&str>, key: &str) -> bool {
        self.contains(&Self::path(child, key))
    }

    /// True if the component is added/removed or any of its fields differ.
    pub fn component(&self, name: &str) -> bool {
        self.component_at(None, name)
    }

    /// Component override on the root or under prefab-local `child` (`Eye/Disc.radius`).
    pub fn component_at(&self, child: Option<&str>, name: &str) -> bool {
        let exact = Self::path(child, name);
        let prefix = format!("{exact}.");
        self.fields
            .iter()
            .any(|f| f == &exact || f.starts_with(&prefix))
    }
}

/// Compare an instance to a loaded prefab entity (name / parent / prefab link ignored).
pub fn prefab_overrides(instance: &EntityData, prefab: &EntityData) -> PrefabOverrides {
    let mut fields = Vec::new();
    push_transform(&mut fields, &instance.transform, &prefab.transform);
    if instance.tag != prefab.tag {
        fields.push("tag".into());
    }
    push_components(&mut fields, &instance.components, &prefab.components);
    PrefabOverrides { fields }
}

/// Map prefab-local entity names → instance entity names for a prefab instance root.
///
/// Matching prefers exact names, then `Name_N` unique suffixes, then first unpaired child.
pub fn match_prefab_instance(
    scene: &Scene,
    instance_root: &str,
    prefab: &Prefab,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(prefab.entity.name.clone(), instance_root.to_string());
    let mut remaining: Vec<&EntityData> = prefab.children.iter().collect();
    let mut safety = 0usize;
    while !remaining.is_empty() && safety < prefab.children.len().saturating_add(8) {
        safety += 1;
        let before = remaining.len();
        remaining.retain(|child| {
            let prefab_parent = child
                .parent
                .as_deref()
                .unwrap_or(prefab.entity.name.as_str());
            let Some(inst_parent) = map.get(prefab_parent).cloned() else {
                return true;
            };
            let candidates = scene.child_names(&inst_parent);
            if let Some(inst_name) = pick_matching_child(&candidates, &child.name, &map) {
                map.insert(child.name.clone(), inst_name);
                false
            } else {
                true
            }
        });
        if remaining.len() == before {
            break;
        }
    }
    map
}

fn pick_matching_child(
    candidates: &[String],
    prefab_name: &str,
    map: &HashMap<String, String>,
) -> Option<String> {
    let used: HashSet<&String> = map.values().collect();
    if let Some(c) = candidates
        .iter()
        .find(|c| c.as_str() == prefab_name && !used.contains(c))
    {
        return Some(c.clone());
    }
    let prefix = format!("{prefab_name}_");
    if let Some(c) = candidates.iter().find(|c| {
        !used.contains(c)
            && c.starts_with(&prefix)
            && c[prefix.len()..].chars().all(|ch| ch.is_ascii_digit())
    }) {
        return Some(c.clone());
    }
    candidates.iter().find(|c| !used.contains(c)).cloned()
}

/// Collect descendant entity names under `root` (not including root), breadth-first.
pub fn collect_descendants(scene: &Scene, root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = scene.child_names(root);
    let mut i = 0usize;
    while i < stack.len() {
        let n = stack[i].clone();
        out.push(n.clone());
        for c in scene.child_names(&n) {
            if !stack.iter().any(|s| s == &c) {
                stack.push(c);
            }
        }
        i += 1;
    }
    out
}

/// Walk up from `name` and return the nearest ancestor (or self) with a prefab link.
pub fn find_prefab_instance_root<'a>(scene: &'a Scene, name: &str) -> Option<&'a EntityData> {
    let mut current = name.to_string();
    for _ in 0..=scene.entities.len() {
        let ent = scene.find_entity(&current)?;
        if ent.prefab.as_ref().is_some_and(|s| !s.is_empty()) {
            return Some(ent);
        }
        match &ent.parent {
            Some(p) => current = p.clone(),
            None => return None,
        }
    }
    None
}

/// Prefab-local name for an instance entity under a matched instance root, if any.
pub fn prefab_local_name(
    scene: &Scene,
    instance_root: &str,
    prefab: &Prefab,
    instance_entity: &str,
) -> Option<String> {
    let map = match_prefab_instance(scene, instance_root, prefab);
    map.into_iter()
        .find(|(_, inst)| inst == instance_entity)
        .map(|(local, _)| local)
}

/// Compare a prefab instance tree to the asset (root fields unprefixed; children as `Child/field`).
pub fn prefab_tree_overrides(
    scene: &Scene,
    instance_root: &str,
    prefab: &Prefab,
) -> PrefabOverrides {
    let Some(inst_root) = scene.find_entity(instance_root) else {
        return PrefabOverrides::default();
    };
    let map = match_prefab_instance(scene, instance_root, prefab);
    let mut fields = Vec::new();
    fields.extend(prefab_overrides(inst_root, &prefab.entity).fields);
    for child in &prefab.children {
        let Some(inst_name) = map.get(&child.name) else {
            // Missing nested child counts as a structural override marker.
            fields.push(format!("{}/<missing>", child.name));
            continue;
        };
        let Some(inst) = scene.find_entity(inst_name) else {
            fields.push(format!("{}/<missing>", child.name));
            continue;
        };
        for f in prefab_overrides(inst, child).fields {
            fields.push(format!("{}/{f}", child.name));
        }
    }
    PrefabOverrides { fields }
}

/// Topo-order prefab children so parents appear before their descendants.
pub fn topo_prefab_children(prefab: &Prefab) -> Vec<&EntityData> {
    let root = prefab.entity.name.as_str();
    let mut remaining: Vec<&EntityData> = prefab.children.iter().collect();
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut ready = HashSet::new();
    ready.insert(root.to_string());
    let mut safety = 0usize;
    while !remaining.is_empty() && safety < prefab.children.len().saturating_add(8) {
        safety += 1;
        let before = remaining.len();
        remaining.retain(|child| {
            let parent = child.parent.as_deref().unwrap_or(root);
            if ready.contains(parent) {
                ready.insert(child.name.clone());
                ordered.push(*child);
                false
            } else {
                true
            }
        });
        if remaining.len() == before {
            // Cycle / missing parent — append rest as-is.
            ordered.extend(remaining);
            break;
        }
    }
    ordered
}

fn push_transform(out: &mut Vec<String>, a: &SceneTransform, b: &SceneTransform) {
    if !near3(&a.translation, &b.translation) {
        out.push("transform.position".into());
    }
    if !near4(&a.rotation, &b.rotation) {
        out.push("transform.rotation".into());
    }
    if !near3(&a.scale, &b.scale) {
        out.push("transform.scale".into());
    }
}

fn push_components(out: &mut Vec<String>, a: &SceneComponents, b: &SceneComponents) {
    match (&a.sprite, &b.sprite) {
        (Some(i), Some(p)) => push_sprite(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Sprite".into()),
        (None, None) => {}
    }
    match (&a.disc, &b.disc) {
        (Some(i), Some(p)) => push_disc(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Disc".into()),
        (None, None) => {}
    }
    match (&a.camera, &b.camera) {
        (Some(i), Some(p)) => push_camera(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Camera".into()),
        (None, None) => {}
    }
    match (&a.tilemap, &b.tilemap) {
        (Some(i), Some(p)) => push_tilemap(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Tilemap".into()),
        (None, None) => {}
    }
    match (&a.collider, &b.collider) {
        (Some(i), Some(p)) => push_collider(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Collider".into()),
        (None, None) => {}
    }
    match (&a.animation, &b.animation) {
        (Some(i), Some(p)) => {
            if i.clip != p.clip {
                out.push("Animation.clip".into());
            }
            if i.fps != p.fps {
                out.push("Animation.fps".into());
            }
            if i.loop_ != p.loop_ {
                out.push("Animation.loop".into());
            }
            if i.enabled != p.enabled {
                out.push("Animation.enabled".into());
            }
        }
        (Some(_), None) | (None, Some(_)) => out.push("Animation".into()),
        (None, None) => {}
    }
    match (&a.grid_mover, &b.grid_mover) {
        (Some(i), Some(p)) => push_grid_mover(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("GridMover".into()),
        (None, None) => {}
    }
    match (&a.audio_source, &b.audio_source) {
        (Some(i), Some(p)) => push_audio(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("AudioSource".into()),
        (None, None) => {}
    }
    match (&a.text, &b.text) {
        (Some(i), Some(p)) => push_text(out, i, p),
        (Some(_), None) | (None, Some(_)) => out.push("Text".into()),
        (None, None) => {}
    }
}

fn push_sprite(out: &mut Vec<String>, a: &SceneSprite, b: &SceneSprite) {
    if a.texture != b.texture {
        out.push("Sprite.texture".into());
    }
    if !near2(&a.size, &b.size) {
        out.push("Sprite.size".into());
    }
    if a.color != b.color {
        out.push("Sprite.color".into());
    }
    if !near(a.z, b.z) {
        out.push("Sprite.z".into());
    }
    if display_sorting_layer(&a.sorting_layer) != display_sorting_layer(&b.sorting_layer) {
        out.push("Sprite.sorting_layer".into());
    }
    if a.enabled != b.enabled {
        out.push("Sprite.enabled".into());
    }
    match (&a.pivot, &b.pivot) {
        (None, None) => {}
        (Some(x), Some(y)) if near2(x, y) => {}
        _ => out.push("Sprite.pivot".into()),
    }
}

fn push_disc(out: &mut Vec<String>, a: &SceneDisc, b: &SceneDisc) {
    if !near(a.radius, b.radius) {
        out.push("Disc.radius".into());
    }
    if a.color != b.color {
        out.push("Disc.color".into());
    }
    if !near(a.z, b.z) {
        out.push("Disc.z".into());
    }
    if display_sorting_layer(&a.sorting_layer) != display_sorting_layer(&b.sorting_layer) {
        out.push("Disc.sorting_layer".into());
    }
    if a.enabled != b.enabled {
        out.push("Disc.enabled".into());
    }
}

fn push_camera(out: &mut Vec<String>, a: &SceneCamera, b: &SceneCamera) {
    if a.active != b.active {
        out.push("Camera.active".into());
    }
    if a.follow != b.follow {
        out.push("Camera.follow".into());
    }
    if !near(a.lerp, b.lerp) {
        out.push("Camera.lerp".into());
    }
}

fn push_tilemap(out: &mut Vec<String>, a: &SceneTilemap, b: &SceneTilemap) {
    if !near(a.cell, b.cell) {
        out.push("Tilemap.cell".into());
    }
    if !near2(&a.origin, &b.origin) {
        out.push("Tilemap.origin".into());
    }
    if a.width != b.width || a.height != b.height {
        out.push("Tilemap.size".into());
    }
    if a.cells != b.cells {
        out.push("Tilemap.cells".into());
    }
    if a.solid != b.solid {
        out.push("Tilemap.solid".into());
    }
    if a.palette.len() != b.palette.len()
        || a.palette.iter().zip(b.palette.iter()).any(|(x, y)| {
            x.id != y.id
                || x.sprite != y.sprite
                || x.color != y.color
                || x.anim != y.anim
                || x.anim_fps != y.anim_fps
                || x.auto_tile != y.auto_tile
                || x.auto_sprites != y.auto_sprites
        })
    {
        out.push("Tilemap.palette".into());
    }
    if !near(a.z, b.z) {
        out.push("Tilemap.z".into());
    }
    if display_sorting_layer(&a.sorting_layer) != display_sorting_layer(&b.sorting_layer) {
        out.push("Tilemap.sorting_layer".into());
    }
    if a.enabled != b.enabled {
        out.push("Tilemap.enabled".into());
    }
}

fn push_collider(out: &mut Vec<String>, a: &SceneCollider, b: &SceneCollider) {
    if a.kind != b.kind {
        out.push("Collider.kind".into());
    }
    if !near2(&a.size, &b.size) {
        out.push("Collider.size".into());
    }
    if !near(a.radius, b.radius) {
        out.push("Collider.radius".into());
    }
    if !near2(&a.offset, &b.offset) {
        out.push("Collider.offset".into());
    }
    if a.solid != b.solid {
        out.push("Collider.solid".into());
    }
    if a.trigger != b.trigger {
        out.push("Collider.trigger".into());
    }
    if a.filter_tag != b.filter_tag {
        out.push("Collider.filter_tag".into());
    }
    if a.enabled != b.enabled {
        out.push("Collider.enabled".into());
    }
}

fn push_grid_mover(out: &mut Vec<String>, a: &SceneGridMover, b: &SceneGridMover) {
    if !near(a.cell, b.cell) {
        out.push("GridMover.cell".into());
    }
    if !near(a.speed, b.speed) {
        out.push("GridMover.speed".into());
    }
    if a.queued_dir != b.queued_dir {
        out.push("GridMover.queued_dir".into());
    }
    if a.enabled != b.enabled {
        out.push("GridMover.enabled".into());
    }
}

fn push_audio(out: &mut Vec<String>, a: &SceneAudioSource, b: &SceneAudioSource) {
    if a.clip != b.clip {
        out.push("AudioSource.clip".into());
    }
    if !near(a.volume, b.volume) {
        out.push("AudioSource.volume".into());
    }
    if a.play_on_awake != b.play_on_awake {
        out.push("AudioSource.play_on_awake".into());
    }
    if a.enabled != b.enabled {
        out.push("AudioSource.enabled".into());
    }
}

fn push_text(out: &mut Vec<String>, a: &SceneText, b: &SceneText) {
    if a.text != b.text {
        out.push("Text.text".into());
    }
    if !near(a.size, b.size) {
        out.push("Text.size".into());
    }
    if a.color != b.color {
        out.push("Text.color".into());
    }
    if a.align != b.align {
        out.push("Text.align".into());
    }
    if !near(a.z, b.z) {
        out.push("Text.z".into());
    }
    if display_sorting_layer(&a.sorting_layer) != display_sorting_layer(&b.sorting_layer) {
        out.push("Text.sorting_layer".into());
    }
    if a.enabled != b.enabled {
        out.push("Text.enabled".into());
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= EPS
}

fn near2(a: &[f32], b: &[f32]) -> bool {
    a.len() >= 2 && b.len() >= 2 && near(a[0], b[0]) && near(a[1], b[1])
}

fn near3(a: &[f32; 3], b: &[f32; 3]) -> bool {
    near(a[0], b[0]) && near(a[1], b[1]) && near(a[2], b[2])
}

fn near4(a: &[f32; 4], b: &[f32; 4]) -> bool {
    near(a[0], b[0]) && near(a[1], b[1]) && near(a[2], b[2]) && near(a[3], b[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SceneDisc;

    #[test]
    fn normalize_stem_and_path() {
        assert_eq!(
            normalize_prefab_source("ghost"),
            "assets/prefabs/ghost.prefab.json"
        );
        assert_eq!(
            normalize_prefab_source("assets/prefabs/ghost.prefab.json"),
            "assets/prefabs/ghost.prefab.json"
        );
        assert_eq!(
            normalize_prefab_source("ghost.prefab.json"),
            "assets/prefabs/ghost.prefab.json"
        );
        assert_eq!(prefab_stem("assets/prefabs/ghost.prefab.json"), "ghost");
        assert_eq!(prefab_stem("ghost"), "ghost");
    }

    #[test]
    fn missing_prefab_field_is_not_an_instance() {
        let json = r#"{"name":"plain"}"#;
        let e: EntityData = serde_json::from_str(json).unwrap();
        assert!(e.prefab.is_none());
        let text = serde_json::to_string(&e).unwrap();
        assert!(!text.contains("prefab"), "{text}");
    }

    #[test]
    fn override_detects_transform_and_disc() {
        let mut prefab = EntityData {
            name: "Ghost".into(),
            parent: None,
            transform: SceneTransform::from_xy(10.0, 20.0),
            components: SceneComponents {
                disc: Some(SceneDisc {
                    radius: 8.0,
                    color: [255, 0, 0, 255],
                    z: 0.0,
                    sorting_layer: String::new(),
                    enabled: true,
                }),
                ..Default::default()
            },
            tag: 0,
            prefab: None,
        };
        let mut inst = prefab.clone();
        inst.name = "Ghost_1".into();
        inst.prefab = Some("assets/prefabs/ghost.prefab.json".into());
        assert!(prefab_overrides(&inst, &prefab).is_empty());

        inst.transform.translation[0] = 40.0;
        inst.components.disc.as_mut().unwrap().radius = 12.0;
        inst.tag = 3;
        let ov = prefab_overrides(&inst, &prefab);
        assert!(ov.contains("transform.position"), "{ov:?}");
        assert!(ov.contains("Disc.radius"), "{ov:?}");
        assert!(ov.contains("tag"), "{ov:?}");
        assert!(!ov.contains("transform.scale"));

        prefab.components.disc = None;
        let ov = prefab_overrides(&inst, &prefab);
        assert!(ov.component("Disc"));
    }

    #[test]
    fn override_detects_sprite_pivot() {
        let prefab = EntityData {
            name: "Orb".into(),
            parent: None,
            transform: SceneTransform::from_xy(0.0, 0.0),
            components: SceneComponents {
                sprite: Some(SceneSprite {
                    texture: "tex".into(),
                    size: [16.0, 16.0],
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
        };
        let mut inst = prefab.clone();
        assert!(prefab_overrides(&inst, &prefab).is_empty());
        inst.components.sprite.as_mut().unwrap().pivot = Some([0.0, 1.0]);
        let ov = prefab_overrides(&inst, &prefab);
        assert!(ov.contains("Sprite.pivot"), "{ov:?}");
    }

    #[test]
    fn old_single_entity_prefab_json_loads() {
        let json = r#"{"name":"Ghost","transform":{"translation":[1.0,2.0,0.0]},"components":{},"tag":0}"#;
        let pf: Prefab = serde_json::from_str(json).unwrap();
        assert_eq!(pf.entity.name, "Ghost");
        assert!(pf.children.is_empty());
        assert_eq!(pf.entity.transform.translation[0], 1.0);
    }

    #[test]
    fn nested_prefab_json_roundtrip() {
        let pf = Prefab {
            entity: EntityData {
                name: "Ghost".into(),
                parent: None,
                transform: SceneTransform::from_xy(10.0, 20.0),
                components: SceneComponents {
                    disc: Some(SceneDisc {
                        radius: 8.0,
                        color: [255, 0, 0, 255],
                        z: 0.0,
                        sorting_layer: String::new(),
                        enabled: true,
                    }),
                    ..Default::default()
                },
                tag: 0,
                prefab: None,
            },
            children: vec![EntityData {
                name: "Eye".into(),
                parent: Some("Ghost".into()),
                transform: SceneTransform::from_xy(4.0, -2.0),
                components: SceneComponents {
                    disc: Some(SceneDisc {
                        radius: 2.0,
                        color: [255, 255, 255, 255],
                        z: 1.0,
                        sorting_layer: String::new(),
                        enabled: true,
                    }),
                    ..Default::default()
                },
                tag: 0,
                prefab: None,
            }],
        };
        let text = serde_json::to_string(&pf).unwrap();
        assert!(text.contains("\"children\""), "{text}");
        let back: Prefab = serde_json::from_str(&text).unwrap();
        assert_eq!(back.children.len(), 1);
        assert_eq!(back.children[0].name, "Eye");
        assert_eq!(back.children[0].parent.as_deref(), Some("Ghost"));
    }

    #[test]
    fn tree_overrides_use_child_prefix() {
        let mut scene = Scene::new("t");
        scene.entities.push(EntityData {
            name: "Ghost_1".into(),
            parent: None,
            transform: SceneTransform::from_xy(0.0, 0.0),
            components: Default::default(),
            tag: 0,
            prefab: Some("assets/prefabs/ghost.prefab.json".into()),
        });
        scene.entities.push(EntityData {
            name: "Eye_1".into(),
            parent: Some("Ghost_1".into()),
            transform: SceneTransform::from_xy(9.0, 0.0),
            components: SceneComponents {
                disc: Some(SceneDisc {
                    radius: 3.0,
                    color: [255, 255, 255, 255],
                    z: 0.0,
                    sorting_layer: String::new(),
                    enabled: true,
                }),
                ..Default::default()
            },
            tag: 0,
            prefab: None,
        });
        let prefab = Prefab {
            entity: EntityData {
                name: "Ghost".into(),
                parent: None,
                transform: SceneTransform::from_xy(0.0, 0.0),
                components: Default::default(),
                tag: 0,
                prefab: None,
            },
            children: vec![EntityData {
                name: "Eye".into(),
                parent: Some("Ghost".into()),
                transform: SceneTransform::from_xy(4.0, 0.0),
                components: SceneComponents {
                    disc: Some(SceneDisc {
                        radius: 2.0,
                        color: [255, 255, 255, 255],
                        z: 0.0,
                        sorting_layer: String::new(),
                        enabled: true,
                    }),
                    ..Default::default()
                },
                tag: 0,
                prefab: None,
            }],
        };
        let ov = prefab_tree_overrides(&scene, "Ghost_1", &prefab);
        assert!(ov.has(Some("Eye"), "transform.position"), "{ov:?}");
        assert!(ov.has(Some("Eye"), "Disc.radius"), "{ov:?}");
        assert!(ov.component_at(Some("Eye"), "Disc"), "{ov:?}");
        assert!(!ov.contains("transform.position"));
    }
}
