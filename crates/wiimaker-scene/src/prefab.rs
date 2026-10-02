//! Prefab instance links + override detection (Unity analogue, nested + variants).

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

/// Load the prefab asset linked on `entity`, resolving variants (base + overrides).
pub fn load_prefab_for_instance(game_dir: &Path, entity: &EntityData) -> Result<Prefab> {
    let src = entity
        .prefab
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("entity '{}' is not a prefab instance", entity.name))?;
    let raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, src)?)?;
    resolve_prefab(game_dir, &raw)
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

/// Diff two prefab assets (root fields unprefixed; children as `Child/field`).
pub fn prefab_asset_overrides(variant: &Prefab, base: &Prefab) -> PrefabOverrides {
    let mut fields = prefab_overrides(&variant.entity, &base.entity).fields;
    let base_children: HashMap<&str, &EntityData> = base
        .children
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();
    let mut seen = HashSet::new();
    for child in &variant.children {
        seen.insert(child.name.as_str());
        match base_children.get(child.name.as_str()) {
            Some(b) => {
                for f in prefab_overrides(child, b).fields {
                    fields.push(format!("{}/{f}", child.name));
                }
            }
            None => fields.push(format!("{}/<added>", child.name)),
        }
    }
    for b in &base.children {
        if !seen.contains(b.name.as_str()) {
            fields.push(format!("{}/<missing>", b.name));
        }
    }
    PrefabOverrides { fields }
}

/// Resolve a prefab for instantiate / override detect: ordinary assets pass through;
/// variants apply `overrides` on top of the recursively resolved base.
pub fn resolve_prefab(game_dir: &Path, prefab: &Prefab) -> Result<Prefab> {
    resolve_prefab_inner(game_dir, prefab, &mut Vec::new())
}

fn resolve_prefab_inner(
    game_dir: &Path,
    prefab: &Prefab,
    stack: &mut Vec<String>,
) -> Result<Prefab> {
    let Some(base_src) = prefab.base.as_deref().filter(|s| !s.is_empty()) else {
        return Ok(Prefab {
            entity: prefab.entity.clone(),
            children: prefab.children.clone(),
            base: None,
            overrides: Vec::new(),
        });
    };
    let link = normalize_prefab_source(base_src);
    if stack.iter().any(|s| s == &link) {
        bail!("prefab variant cycle involving {link}");
    }
    stack.push(link.clone());
    let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &link)?)?;
    let mut resolved = resolve_prefab_inner(game_dir, &base_raw, stack)?;
    stack.pop();

    // Keep variant root name (Unity allows renaming the variant root).
    let variant_root_name = prefab.entity.name.clone();
    let base_root_name = resolved.entity.name.clone();

    let paths = if prefab.overrides.is_empty() {
        // Hand-edited / legacy: treat full tree diff as overrides.
        prefab_asset_overrides(prefab, &resolved).fields
    } else {
        prefab.overrides.clone()
    };
    apply_override_paths(&mut resolved, prefab, &paths)?;

    if variant_root_name != base_root_name {
        // Remap child parent pointers that still name the base root.
        for child in &mut resolved.children {
            if child.parent.as_deref() == Some(base_root_name.as_str()) {
                child.parent = Some(variant_root_name.clone());
            }
        }
        resolved.entity.name = variant_root_name;
    }
    resolved.base = None;
    resolved.overrides.clear();
    Ok(resolved)
}

/// Recompute `overrides` for a variant asset vs its resolved base. No-op if not a variant.
pub fn refresh_variant_overrides(game_dir: &Path, prefab: &mut Prefab) -> Result<()> {
    let Some(base_src) = prefab.base.clone().filter(|s| !s.is_empty()) else {
        prefab.overrides.clear();
        return Ok(());
    };
    let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &base_src)?)?;
    let base = resolve_prefab(game_dir, &base_raw)?;
    // Compare using matching root names for clean paths.
    let mut cmp = prefab.clone();
    let base_name = base.entity.name.clone();
    if cmp.entity.name != base_name {
        let old = cmp.entity.name.clone();
        cmp.entity.name = base_name.clone();
        for child in &mut cmp.children {
            if child.parent.as_deref() == Some(old.as_str()) {
                child.parent = Some(base_name.clone());
            }
        }
    }
    prefab.overrides = prefab_asset_overrides(&cmp, &base).fields;
    Ok(())
}

/// Create a prefab variant asset from a base (resolved snapshot, empty overrides).
pub fn create_prefab_variant(
    game_dir: &Path,
    base_source: &str,
    as_name: &str,
) -> Result<(PathBuf, Prefab)> {
    let base_link = normalize_prefab_source(base_source);
    if base_link.is_empty() {
        bail!("base prefab source cannot be empty");
    }
    let stem = as_name.trim();
    if stem.is_empty() {
        bail!("variant name cannot be empty");
    }
    let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &base_link)?)?;
    let resolved = resolve_prefab(game_dir, &base_raw)?;
    let variant = Prefab {
        entity: resolved.entity,
        children: resolved.children,
        base: Some(base_link),
        overrides: Vec::new(),
    };
    let dest = game_dir
        .join("assets")
        .join("prefabs")
        .join(format!("{stem}.prefab.json"));
    crate::scene::save_prefab(&dest, &variant)?;
    Ok((dest, variant))
}

/// Build a variant Prefab from a scene instance (base = instance link; overrides vs base).
pub fn variant_from_instance(
    game_dir: &Path,
    scene: &crate::scene::Scene,
    instance_root: &str,
) -> Result<Prefab> {
    let ent = scene
        .find_entity(instance_root)
        .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' not found"))?;
    let base_src = ent
        .prefab
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("entity '{instance_root}' is not a prefab instance (need a base link)")
        })?;
    let base_link = normalize_prefab_source(base_src);
    let mut variant = crate::mutate::entity_to_prefab(scene, instance_root)?;
    variant.base = Some(base_link);
    refresh_variant_overrides(game_dir, &mut variant)?;
    Ok(variant)
}

fn apply_override_paths(dest: &mut Prefab, src: &Prefab, paths: &[String]) -> Result<()> {
    for path in paths {
        if path.ends_with("/<missing>") || path.ends_with("/<added>") {
            let child_name = path.rsplit_once('/').map(|(c, _)| c).unwrap_or(path);
            if path.ends_with("/<added>") {
                if let Some(child) = src.children.iter().find(|c| c.name == child_name) {
                    if !dest.children.iter().any(|c| c.name == child_name) {
                        let mut c = child.clone();
                        if c.parent.is_none() {
                            c.parent = Some(dest.entity.name.clone());
                        }
                        dest.children.push(c);
                    }
                }
            }
            // <missing>: leave base child (variant removed it) — drop from dest.
            if path.ends_with("/<missing>") {
                dest.children.retain(|c| c.name != child_name);
            }
            continue;
        }
        if let Some((child_name, field)) = path.split_once('/') {
            ensure_child_from_src(dest, src, child_name);
            let Some(dest_child) = dest.children.iter_mut().find(|c| c.name == child_name) else {
                continue;
            };
            let Some(src_child) = src.children.iter().find(|c| c.name == child_name) else {
                continue;
            };
            copy_entity_field(dest_child, src_child, field);
        } else {
            copy_entity_field(&mut dest.entity, &src.entity, path);
        }
    }
    Ok(())
}

fn ensure_child_from_src(dest: &mut Prefab, src: &Prefab, child_name: &str) {
    if dest.children.iter().any(|c| c.name == child_name) {
        return;
    }
    if let Some(child) = src.children.iter().find(|c| c.name == child_name) {
        let mut c = child.clone();
        if c.parent.is_none() {
            c.parent = Some(dest.entity.name.clone());
        }
        dest.children.push(c);
    }
}

fn copy_entity_field(dest: &mut EntityData, src: &EntityData, field: &str) {
    match field {
        "transform.position" => dest.transform.translation = src.transform.translation,
        "transform.rotation" => dest.transform.rotation = src.transform.rotation,
        "transform.scale" => dest.transform.scale = src.transform.scale,
        "tag" => dest.tag = src.tag,
        "Sprite" => dest.components.sprite = src.components.sprite.clone(),
        "Sprite.texture" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.texture = s.texture.clone();
            } else {
                dest.components.sprite = src.components.sprite.clone();
            }
        }
        "Sprite.size" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.size = s.size;
            }
        }
        "Sprite.color" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.color = s.color;
            }
        }
        "Sprite.z" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.z = s.z;
            }
        }
        "Sprite.sorting_layer" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.sorting_layer = s.sorting_layer.clone();
            }
        }
        "Sprite.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.enabled = s.enabled;
            }
        }
        "Sprite.pivot" => {
            if let (Some(d), Some(s)) = (&mut dest.components.sprite, &src.components.sprite) {
                d.pivot = s.pivot;
            }
        }
        "Disc" => dest.components.disc = src.components.disc.clone(),
        "Disc.radius" => {
            if let (Some(d), Some(s)) = (&mut dest.components.disc, &src.components.disc) {
                d.radius = s.radius;
            } else {
                dest.components.disc = src.components.disc.clone();
            }
        }
        "Disc.color" => {
            if let (Some(d), Some(s)) = (&mut dest.components.disc, &src.components.disc) {
                d.color = s.color;
            }
        }
        "Disc.z" => {
            if let (Some(d), Some(s)) = (&mut dest.components.disc, &src.components.disc) {
                d.z = s.z;
            }
        }
        "Disc.sorting_layer" => {
            if let (Some(d), Some(s)) = (&mut dest.components.disc, &src.components.disc) {
                d.sorting_layer = s.sorting_layer.clone();
            }
        }
        "Disc.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.disc, &src.components.disc) {
                d.enabled = s.enabled;
            }
        }
        "Camera" => dest.components.camera = src.components.camera.clone(),
        "Camera.active" => {
            if let (Some(d), Some(s)) = (&mut dest.components.camera, &src.components.camera) {
                d.active = s.active;
            }
        }
        "Camera.follow" => {
            if let (Some(d), Some(s)) = (&mut dest.components.camera, &src.components.camera) {
                d.follow = s.follow.clone();
            }
        }
        "Camera.lerp" => {
            if let (Some(d), Some(s)) = (&mut dest.components.camera, &src.components.camera) {
                d.lerp = s.lerp;
            }
        }
        "Tilemap" => dest.components.tilemap = src.components.tilemap.clone(),
        "Tilemap.cell" | "Tilemap.origin" | "Tilemap.size" | "Tilemap.cells" | "Tilemap.solid"
        | "Tilemap.palette" | "Tilemap.z" | "Tilemap.sorting_layer" | "Tilemap.enabled" => {
            dest.components.tilemap = src.components.tilemap.clone();
        }
        "Collider" => dest.components.collider = src.components.collider.clone(),
        "Collider.kind" | "Collider.size" | "Collider.radius" | "Collider.offset"
        | "Collider.solid" | "Collider.trigger" | "Collider.filter_tag" | "Collider.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.collider, &src.components.collider) {
                match field {
                    "Collider.kind" => d.kind = s.kind,
                    "Collider.size" => d.size = s.size,
                    "Collider.radius" => d.radius = s.radius,
                    "Collider.offset" => d.offset = s.offset,
                    "Collider.solid" => d.solid = s.solid,
                    "Collider.trigger" => d.trigger = s.trigger,
                    "Collider.filter_tag" => d.filter_tag = s.filter_tag,
                    "Collider.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.collider = src.components.collider.clone();
            }
        }
        "Animation" => dest.components.animation = src.components.animation.clone(),
        "Animation.clip" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animation, &src.components.animation)
            {
                d.clip = s.clip.clone();
            }
        }
        "Animation.fps" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animation, &src.components.animation)
            {
                d.fps = s.fps;
            }
        }
        "Animation.loop" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animation, &src.components.animation)
            {
                d.loop_ = s.loop_;
            }
        }
        "Animation.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animation, &src.components.animation)
            {
                d.enabled = s.enabled;
            }
        }
        "GridMover" => dest.components.grid_mover = src.components.grid_mover.clone(),
        "GridMover.cell" | "GridMover.speed" | "GridMover.queued_dir" | "GridMover.enabled" => {
            if let (Some(d), Some(s)) =
                (&mut dest.components.grid_mover, &src.components.grid_mover)
            {
                match field {
                    "GridMover.cell" => d.cell = s.cell,
                    "GridMover.speed" => d.speed = s.speed,
                    "GridMover.queued_dir" => d.queued_dir = s.queued_dir,
                    "GridMover.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.grid_mover = src.components.grid_mover.clone();
            }
        }
        "AudioSource" => dest.components.audio_source = src.components.audio_source.clone(),
        "AudioSource.clip" | "AudioSource.volume" | "AudioSource.play_on_awake"
        | "AudioSource.enabled" => {
            if let (Some(d), Some(s)) = (
                &mut dest.components.audio_source,
                &src.components.audio_source,
            ) {
                match field {
                    "AudioSource.clip" => d.clip = s.clip.clone(),
                    "AudioSource.volume" => d.volume = s.volume,
                    "AudioSource.play_on_awake" => d.play_on_awake = s.play_on_awake,
                    "AudioSource.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.audio_source = src.components.audio_source.clone();
            }
        }
        "Text" => dest.components.text = src.components.text.clone(),
        "Text.text" | "Text.size" | "Text.color" | "Text.align" | "Text.z"
        | "Text.sorting_layer" | "Text.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.text, &src.components.text) {
                match field {
                    "Text.text" => d.text = s.text.clone(),
                    "Text.size" => d.size = s.size,
                    "Text.color" => d.color = s.color,
                    "Text.align" => d.align = s.align,
                    "Text.z" => d.z = s.z,
                    "Text.sorting_layer" => d.sorting_layer = s.sorting_layer.clone(),
                    "Text.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.text = src.components.text.clone();
            }
        }
        _ => {}
    }
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
            base: None,
            overrides: Vec::new(),
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
            base: None,
            overrides: Vec::new(),
        };
        let ov = prefab_tree_overrides(&scene, "Ghost_1", &prefab);
        assert!(ov.has(Some("Eye"), "transform.position"), "{ov:?}");
        assert!(ov.has(Some("Eye"), "Disc.radius"), "{ov:?}");
        assert!(ov.component_at(Some("Eye"), "Disc"), "{ov:?}");
        assert!(!ov.contains("transform.position"));
    }

    #[test]
    fn old_prefab_without_base_still_loads() {
        let json = r#"{"name":"Ghost","transform":{"translation":[1.0,2.0,0.0]},"components":{},"tag":0}"#;
        let pf: Prefab = serde_json::from_str(json).unwrap();
        assert!(!pf.is_variant());
        assert!(pf.base.is_none());
        assert!(pf.overrides.is_empty());
    }

    #[test]
    fn variant_json_roundtrip() {
        let pf = Prefab {
            entity: EntityData {
                name: "GhostBlue".into(),
                parent: None,
                transform: SceneTransform::from_xy(0.0, 0.0),
                components: SceneComponents {
                    disc: Some(SceneDisc {
                        radius: 8.0,
                        color: [0, 0, 255, 255],
                        z: 0.0,
                        sorting_layer: String::new(),
                        enabled: true,
                    }),
                    ..Default::default()
                },
                tag: 0,
                prefab: None,
            },
            children: Vec::new(),
            base: Some("assets/prefabs/ghost.prefab.json".into()),
            overrides: vec!["Disc.color".into()],
        };
        let text = serde_json::to_string(&pf).unwrap();
        assert!(text.contains("\"base\""), "{text}");
        assert!(text.contains("\"overrides\""), "{text}");
        let back: Prefab = serde_json::from_str(&text).unwrap();
        assert!(back.is_variant());
        assert_eq!(back.base.as_deref(), Some("assets/prefabs/ghost.prefab.json"));
        assert_eq!(back.overrides, vec!["Disc.color".to_string()]);
    }
}