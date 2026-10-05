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
    let base_children: HashMap<&str, &EntityData> =
        base.children.iter().map(|c| (c.name.as_str(), c)).collect();
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

/// Where an Apply-to-Base writes in a variant chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyBaseTarget {
    /// The asset named by this variant's `base` field.
    Immediate,
    /// The first non-variant prefab at the end of the chain.
    Root,
}

/// Result of pushing override paths from a variant onto a base asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ApplyToBaseReport {
    /// Game-relative variant asset that lost the applied paths.
    pub variant: String,
    /// Game-relative asset that received the values (immediate or root).
    pub base: String,
    pub applied: Vec<String>,
    /// Overrides still stored on the variant asset after the write.
    pub variant_overrides: Vec<String>,
    /// `variant` … immediate bases … root.
    pub chain: Vec<String>,
    /// Scene instance roots that inherited `applied` because they had no local override.
    #[serde(default)]
    pub scene_synced: Vec<String>,
}

/// Inspector / CLI labels for a prefab variant (Unity Overrides dropdown).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefabVariantChrome {
    pub open_base: &'static str,
    pub select_base: &'static str,
    pub revert: &'static str,
    pub revert_all: &'static str,
    pub revert_to_base: &'static str,
    pub apply_all_to_base: &'static str,
    pub apply_to_variant: String,
    pub apply_all_to_variant: String,
    pub apply_to_base: String,
    pub apply_to_root: Option<String>,
    pub apply_all_to_root: Option<String>,
}

/// Unity-shaped control labels. `root_stem` is set when the chain is deeper than one base.
pub fn prefab_variant_chrome(
    variant_stem: &str,
    base_stem: &str,
    root_stem: Option<&str>,
) -> PrefabVariantChrome {
    let root = root_stem.filter(|r| !r.is_empty() && *r != base_stem);
    PrefabVariantChrome {
        open_base: "Open Base",
        select_base: "Select Base",
        revert: "Revert",
        revert_all: "Revert All",
        revert_to_base: "Revert to Base",
        apply_all_to_base: "Apply All to Base",
        apply_to_variant: format!("Apply to Prefab Variant '{variant_stem}'"),
        apply_all_to_variant: format!("Apply All to Prefab Variant '{variant_stem}'"),
        apply_to_base: format!("Apply to Base '{base_stem}'"),
        apply_to_root: root.map(|r| format!("Apply to Root '{r}'")),
        apply_all_to_root: root.map(|r| format!("Apply All to Root '{r}'")),
    }
}

/// Base chain for a prefab asset: `[self, immediate base, …, root]`.
pub fn prefab_base_chain(game_dir: &Path, source: &str) -> Result<Vec<String>> {
    let mut chain = Vec::new();
    let mut current = normalize_prefab_source(source);
    if current.is_empty() {
        bail!("prefab source cannot be empty");
    }
    for _ in 0..32 {
        if chain.iter().any(|s| s == &current) {
            bail!("prefab variant cycle involving {current}");
        }
        chain.push(current.clone());
        let raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &current)?)?;
        match raw.base.as_deref().filter(|s| !s.is_empty()) {
            Some(base) => current = normalize_prefab_source(base),
            None => return Ok(chain),
        }
    }
    bail!("prefab variant chain too deep");
}

/// Which overrides live on a variant asset vs its immediate base.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PrefabChainStatus {
    pub prefab: String,
    pub base: Option<String>,
    pub root: String,
    pub chain: Vec<String>,
    pub variant: bool,
    pub variant_overrides: Vec<String>,
    pub base_overrides: Vec<String>,
}

/// Load `source` and describe its base chain plus stored overrides.
pub fn prefab_chain_status(game_dir: &Path, source: &str) -> Result<PrefabChainStatus> {
    let chain = prefab_base_chain(game_dir, source)?;
    let prefab = chain.first().cloned().unwrap_or_default();
    let raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &prefab)?)?;
    let base = if chain.len() > 1 {
        Some(chain[1].clone())
    } else {
        None
    };
    let root = chain.last().cloned().unwrap_or_else(|| prefab.clone());
    let base_overrides = if let Some(base_link) = &base {
        let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, base_link)?)?;
        if base_raw.is_variant() {
            base_raw.overrides
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Ok(PrefabChainStatus {
        variant: raw.is_variant(),
        variant_overrides: raw.overrides,
        prefab,
        base,
        root,
        chain,
        base_overrides,
    })
}

/// Write variant-asset override values into the base and drop them from this variant.
///
/// `paths` empty applies every override stored on the variant (legacy empty list is
/// materialized from the snapshot diff first). A requested path that is not on that
/// list is rejected. Other variants of the same base inherit the value only when
/// their materialized override list does not already contain the path.
pub fn apply_variant_override_to_base(
    game_dir: &Path,
    variant_source: &str,
    paths: &[String],
    target: ApplyBaseTarget,
) -> Result<ApplyToBaseReport> {
    let (link, asset, paths_owned, target_link) =
        prepare_variant_apply(game_dir, variant_source, paths, target)?;
    if paths_owned.is_empty() {
        return empty_apply_report(game_dir, &link, &asset);
    }
    let _ = target_link;
    apply_paths_along_chain(game_dir, &link, &asset, &paths_owned, target)
}

/// [`apply_variant_override_to_base`] plus Unity-style refresh of `scene` instances
/// whose prefab chain includes the written base and that did not override those paths.
pub fn apply_variant_override_to_base_in_scene(
    game_dir: &Path,
    scene: &mut Scene,
    variant_source: &str,
    paths: &[String],
    target: ApplyBaseTarget,
) -> Result<ApplyToBaseReport> {
    let (link, asset, paths_owned, target_link) =
        prepare_variant_apply(game_dir, variant_source, paths, target)?;
    if paths_owned.is_empty() {
        return empty_apply_report(game_dir, &link, &asset);
    }
    let slots = plan_prefab_scene_inherit(game_dir, scene, &target_link, &paths_owned)?;
    let mut report = apply_paths_along_chain(game_dir, &link, &asset, &paths_owned, target)?;
    report.scene_synced = apply_prefab_scene_inherit(game_dir, scene, &slots)?;
    Ok(report)
}

/// Apply instance values onto the base asset and drop those paths from the variant.
///
/// `paths` empty applies every instance override versus the resolved variant
/// (not the variant asset's own stored overrides). Other instances in `scene`
/// whose chain includes the written asset inherit paths they did not override.
pub fn apply_instance_to_base(
    game_dir: &Path,
    scene: &mut Scene,
    instance_root: &str,
    paths: &[String],
    target: ApplyBaseTarget,
) -> Result<ApplyToBaseReport> {
    let (link, asset, resolved) = load_instance_variant(game_dir, scene, instance_root)?;
    let inst_paths = prefab_tree_overrides(scene, instance_root, &resolved).fields;
    let paths_owned = if paths.is_empty() {
        inst_paths
    } else {
        validate_instance_paths(&inst_paths, &asset.overrides, paths)?;
        paths.to_vec()
    };
    if paths_owned.is_empty() {
        return empty_apply_report(game_dir, &link, &asset);
    }
    let chain = prefab_base_chain(game_dir, &link)?;
    let target_link = chain_target_link(&chain, target)?;
    let slots = plan_prefab_scene_inherit(game_dir, scene, &target_link, &paths_owned)?;
    let mut values = asset.clone();
    overlay_instance_values(scene, instance_root, &resolved, &mut values)?;
    let mut report = apply_paths_along_chain(game_dir, &link, &values, &paths_owned, target)?;
    report.scene_synced = apply_prefab_scene_inherit(game_dir, scene, &slots)?;
    Ok(report)
}

/// Push specific instance override paths onto the linked prefab asset (not its base).
///
/// `paths` empty applies every instance override. Other instances in `scene` linked
/// through that prefab inherit paths they did not override.
pub fn apply_instance_fields_to_prefab(
    game_dir: &Path,
    scene: &mut Scene,
    instance_root: &str,
    paths: &[String],
) -> Result<Vec<String>> {
    let ent = scene
        .find_entity(instance_root)
        .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' not found"))?;
    let src = ent
        .prefab
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' is not a prefab instance"))?;
    let link = normalize_prefab_source(src);
    let disk = resolve_prefab_asset(game_dir, &link)?;
    let mut asset = crate::scene::load_prefab(&disk)?;
    let resolved = resolve_prefab(game_dir, &asset)?;
    let inst_paths = prefab_tree_overrides(scene, instance_root, &resolved).fields;
    let paths_owned = if paths.is_empty() {
        inst_paths
    } else {
        validate_instance_paths(&inst_paths, &[], paths)?;
        paths.to_vec()
    };
    if paths_owned.is_empty() {
        return Ok(asset.overrides);
    }
    let slots = plan_prefab_scene_inherit(game_dir, scene, &link, &paths_owned)?;
    let mut values = asset.clone();
    overlay_instance_values(scene, instance_root, &resolved, &mut values)?;
    apply_override_paths(&mut asset, &values, &paths_owned)?;
    if asset.is_variant() {
        record_paths_on_variant(game_dir, &mut asset, &paths_owned)?;
        let values = asset.clone();
        realign_variant_snapshot(game_dir, &mut asset, &values)?;
    }
    crate::scene::save_prefab(&disk, &asset)?;
    let _synced = apply_prefab_scene_inherit(game_dir, scene, &slots)?;
    Ok(asset.overrides)
}

/// Drop override paths on a variant asset so those fields inherit from the base.
///
/// `paths` empty reverts every stored variant override. Does not modify scene instances.
pub fn revert_variant_to_base(
    game_dir: &Path,
    variant_source: &str,
    paths: &[String],
) -> Result<ApplyToBaseReport> {
    let link = normalize_prefab_source(variant_source);
    let disk = resolve_prefab_asset(game_dir, &link)?;
    let mut asset = crate::scene::load_prefab(&disk)?;
    if !asset.is_variant() {
        bail!("prefab '{link}' is not a variant (no base to revert to)");
    }
    let effective = materialized_override_list(game_dir, &asset)?;
    let paths_owned = if paths.is_empty() {
        effective.clone()
    } else {
        for path in paths {
            if !is_known_override_path(path) {
                bail!("unknown override path '{path}'");
            }
            if !effective.iter().any(|p| p == path) {
                bail!("override '{path}' is not set on variant '{link}'");
            }
        }
        paths.to_vec()
    };
    let chain = prefab_base_chain(game_dir, &link)?;
    let base = chain.get(1).cloned().unwrap_or_default();
    if paths_owned.is_empty() {
        return Ok(ApplyToBaseReport {
            variant: link,
            base,
            applied: Vec::new(),
            variant_overrides: asset.overrides,
            chain,
            scene_synced: Vec::new(),
        });
    }
    if asset.overrides.is_empty() {
        asset.overrides = effective;
    }
    asset
        .overrides
        .retain(|p| !paths_owned.iter().any(|applied| applied == p));
    let values = asset.clone();
    realign_variant_snapshot(game_dir, &mut asset, &values)?;
    crate::scene::save_prefab(&disk, &asset)?;
    Ok(ApplyToBaseReport {
        variant: link,
        base,
        applied: paths_owned,
        variant_overrides: asset.overrides,
        chain,
        scene_synced: Vec::new(),
    })
}

/// Restore specific instance fields from a resolved prefab. Does not rebuild the tree.
pub fn revert_prefab_instance_fields(
    scene: &mut Scene,
    instance_root: &str,
    prefab: &Prefab,
    paths: &[String],
) -> Result<()> {
    if paths.is_empty() {
        bail!("no fields to revert");
    }
    let map = match_prefab_instance(scene, instance_root, prefab);
    for path in paths {
        if !is_known_override_path(path) {
            bail!("unknown override path '{path}'");
        }
        if path.ends_with("/<added>") || path.ends_with("/<missing>") {
            bail!("revert of structural override '{path}' needs a full Revert");
        }
        if let Some((child, field)) = path.split_once('/') {
            let inst_name = map
                .get(child)
                .ok_or_else(|| anyhow::anyhow!("prefab child '{child}' is not in the instance"))?;
            let src = prefab
                .children
                .iter()
                .find(|c| c.name == child)
                .ok_or_else(|| anyhow::anyhow!("prefab has no child '{child}'"))?;
            let dest = scene
                .entities
                .iter_mut()
                .find(|e| e.name == *inst_name)
                .ok_or_else(|| anyhow::anyhow!("entity '{inst_name}' not found"))?;
            copy_entity_field(dest, src, field);
        } else {
            let dest = scene
                .entities
                .iter_mut()
                .find(|e| e.name == instance_root)
                .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' not found"))?;
            copy_entity_field(dest, &prefab.entity, path);
        }
    }
    Ok(())
}

fn load_instance_variant(
    game_dir: &Path,
    scene: &Scene,
    instance_root: &str,
) -> Result<(String, Prefab, Prefab)> {
    let ent = scene
        .find_entity(instance_root)
        .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' not found"))?;
    let src = ent
        .prefab
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("entity '{instance_root}' is not a prefab instance"))?;
    let link = normalize_prefab_source(src);
    let asset = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &link)?)?;
    if !asset.is_variant() {
        bail!("prefab '{link}' is not a variant (no base to apply to)");
    }
    let resolved = resolve_prefab(game_dir, &asset)?;
    Ok((link, asset, resolved))
}

fn validate_instance_paths(
    instance_paths: &[String],
    asset_paths: &[String],
    paths: &[String],
) -> Result<()> {
    for path in paths {
        if !is_known_override_path(path) {
            bail!("unknown override path '{path}'");
        }
        let on_instance = instance_paths.iter().any(|p| p == path);
        let on_asset = asset_paths.iter().any(|p| p == path);
        if !on_instance && !on_asset {
            bail!("override '{path}' is not set on this instance or variant");
        }
    }
    Ok(())
}

fn empty_apply_report(game_dir: &Path, link: &str, asset: &Prefab) -> Result<ApplyToBaseReport> {
    let chain = prefab_base_chain(game_dir, link)?;
    Ok(ApplyToBaseReport {
        variant: chain.first().cloned().unwrap_or_else(|| link.to_string()),
        base: asset.base.clone().unwrap_or_default(),
        applied: Vec::new(),
        variant_overrides: asset.overrides.clone(),
        chain,
        scene_synced: Vec::new(),
    })
}

fn materialized_override_list(game_dir: &Path, asset: &Prefab) -> Result<Vec<String>> {
    if !asset.overrides.is_empty() || !asset.is_variant() {
        return Ok(asset.overrides.clone());
    }
    let mut copy = asset.clone();
    refresh_variant_overrides(game_dir, &mut copy)?;
    Ok(copy.overrides)
}

fn apply_paths_along_chain(
    game_dir: &Path,
    leaf_link: &str,
    value_src: &Prefab,
    paths: &[String],
    target: ApplyBaseTarget,
) -> Result<ApplyToBaseReport> {
    for path in paths {
        if !is_known_override_path(path) {
            bail!("unknown override path '{path}'");
        }
    }
    let chain = prefab_base_chain(game_dir, leaf_link)?;
    if chain.len() < 2 {
        bail!("prefab '{leaf_link}' is not a variant (no base to apply to)");
    }
    let target_idx = match target {
        ApplyBaseTarget::Immediate => 1,
        ApplyBaseTarget::Root => chain.len() - 1,
    };
    let target_link = chain[target_idx].clone();
    // Gate sibling copies against the *old* base. After the target is written,
    // an empty-override snapshot that still has the previous value would look
    // like a new override vs the new base (or inherit-all if we only inspect
    // `overrides`). Plan now; apply after the chain write.
    let pending_inherit = plan_inherited_snapshot_updates(game_dir, &target_link, paths, &chain)?;
    // Intermediates between the leaf and the target: their own value for a path
    // (materialized vs the *old* base) must survive `--to-root` unless it already
    // equals the value being applied.
    let mut intermediate_materialized = Vec::new();
    for idx in 1..target_idx {
        let disk = resolve_prefab_asset(game_dir, &chain[idx])?;
        let asset = crate::scene::load_prefab(&disk)?;
        intermediate_materialized.push(materialized_override_list(game_dir, &asset)?);
    }
    {
        let disk = resolve_prefab_asset(game_dir, &target_link)?;
        let mut asset = crate::scene::load_prefab(&disk)?;
        apply_override_paths(&mut asset, value_src, paths)?;
        if asset.is_variant() {
            record_paths_on_variant(game_dir, &mut asset, paths)?;
            let values = asset.clone();
            realign_variant_snapshot(game_dir, &mut asset, &values)?;
        }
        crate::scene::save_prefab(&disk, &asset)?;
    }
    let mut leaf_overrides = Vec::new();
    for idx in (0..target_idx).rev() {
        let disk = resolve_prefab_asset(game_dir, &chain[idx])?;
        let mut asset = crate::scene::load_prefab(&disk)?;
        if idx == 0 {
            if asset.is_variant() && asset.overrides.is_empty() {
                refresh_variant_overrides(game_dir, &mut asset)?;
            }
            asset
                .overrides
                .retain(|p| !paths.iter().any(|applied| applied == p));
        } else if asset.is_variant() {
            let materialized = &intermediate_materialized[idx - 1];
            if asset.overrides.is_empty() {
                asset.overrides = materialized.clone();
            }
            for path in paths {
                let owns = materialized.iter().any(|p| p == path);
                let same_as_applied = !path_is_override(&asset, value_src, path);
                if owns && !same_as_applied {
                    if !asset.overrides.iter().any(|p| p == path) {
                        asset.overrides.push(path.clone());
                    }
                } else {
                    asset.overrides.retain(|p| p != path);
                }
            }
        }
        if asset.is_variant() {
            let values = asset.clone();
            realign_variant_snapshot(game_dir, &mut asset, &values)?;
        }
        if idx == 0 {
            leaf_overrides = asset.overrides.clone();
        }
        crate::scene::save_prefab(&disk, &asset)?;
    }
    apply_inherited_snapshot_updates(value_src, pending_inherit)?;
    Ok(ApplyToBaseReport {
        variant: chain[0].clone(),
        base: target_link,
        applied: paths.to_vec(),
        variant_overrides: leaf_overrides,
        chain,
        scene_synced: Vec::new(),
    })
}

fn prepare_variant_apply(
    game_dir: &Path,
    variant_source: &str,
    paths: &[String],
    target: ApplyBaseTarget,
) -> Result<(String, Prefab, Vec<String>, String)> {
    let link = normalize_prefab_source(variant_source);
    let disk = resolve_prefab_asset(game_dir, &link)?;
    let asset = crate::scene::load_prefab(&disk)?;
    if !asset.is_variant() {
        bail!("prefab '{link}' is not a variant (no base to apply to)");
    }
    let effective = materialized_override_list(game_dir, &asset)?;
    let paths_owned = if paths.is_empty() {
        effective
    } else {
        for path in paths {
            if !is_known_override_path(path) {
                bail!("unknown override path '{path}'");
            }
            if !effective.iter().any(|p| p == path) {
                bail!("override '{path}' is not set on variant '{link}'");
            }
        }
        paths.to_vec()
    };
    let chain = prefab_base_chain(game_dir, &link)?;
    let target_link = chain_target_link(&chain, target)?;
    Ok((link, asset, paths_owned, target_link))
}

fn chain_target_link(chain: &[String], target: ApplyBaseTarget) -> Result<String> {
    if chain.len() < 2 {
        bail!("prefab is not a variant (no base to apply to)");
    }
    let idx = match target {
        ApplyBaseTarget::Immediate => 1,
        ApplyBaseTarget::Root => chain.len() - 1,
    };
    Ok(chain[idx].clone())
}

/// One scene instance that should pick up prefab values after an Apply.
#[derive(Clone, Debug)]
pub struct SceneInheritSlot {
    pub root: String,
    pub paths: Vec<String>,
}

/// Instance roots whose prefab chain includes `written_link` and that do not
/// already override `paths` (compared to the prefab *before* the asset write).
pub fn plan_prefab_scene_inherit(
    game_dir: &Path,
    scene: &Scene,
    written_link: &str,
    paths: &[String],
) -> Result<Vec<SceneInheritSlot>> {
    let written = normalize_prefab_source(written_link);
    let inherit_paths: Vec<String> = paths
        .iter()
        .filter(|p| {
            is_known_override_path(p) && !p.ends_with("/<added>") && !p.ends_with("/<missing>")
        })
        .cloned()
        .collect();
    if inherit_paths.is_empty() || written.is_empty() {
        return Ok(Vec::new());
    }
    let roots: Vec<(String, String)> = scene
        .entities
        .iter()
        .filter_map(|e| {
            e.prefab
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| (e.name.clone(), normalize_prefab_source(s)))
        })
        .collect();
    let mut slots = Vec::new();
    for (name, link) in roots {
        let chain = prefab_base_chain(game_dir, &link)?;
        if !chain.iter().any(|c| c == &written) {
            continue;
        }
        let ent = scene
            .find_entity(&name)
            .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
        let resolved = load_prefab_for_instance(game_dir, ent)?;
        let overrides = prefab_tree_overrides(scene, &name, &resolved).fields;
        let keep: Vec<String> = inherit_paths
            .iter()
            .filter(|p| !overrides.iter().any(|o| o == *p))
            .cloned()
            .collect();
        if keep.is_empty() {
            continue;
        }
        slots.push(SceneInheritSlot {
            root: name,
            paths: keep,
        });
    }
    Ok(slots)
}

pub fn apply_prefab_scene_inherit(
    game_dir: &Path,
    scene: &mut Scene,
    slots: &[SceneInheritSlot],
) -> Result<Vec<String>> {
    let mut synced = Vec::new();
    for slot in slots {
        if slot.paths.is_empty() {
            continue;
        }
        let Some(ent) = scene.find_entity(&slot.root).cloned() else {
            continue;
        };
        let resolved = load_prefab_for_instance(game_dir, &ent)?;
        revert_prefab_instance_fields(scene, &slot.root, &resolved, &slot.paths)?;
        synced.push(slot.root.clone());
    }
    Ok(synced)
}

fn plan_inherited_snapshot_updates(
    game_dir: &Path,
    target_link: &str,
    paths: &[String],
    skip: &[String],
) -> Result<Vec<(PathBuf, Vec<String>)>> {
    let dir = game_dir.join("assets").join("prefabs");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut pending = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".prefab.json") {
            continue;
        }
        let link = normalize_prefab_source(name);
        if skip.iter().any(|s| s == &link) {
            continue;
        }
        let asset = crate::scene::load_prefab(&path)?;
        if !asset.is_variant() {
            continue;
        }
        let asset_chain = prefab_base_chain(game_dir, &link)?;
        if !asset_chain.iter().any(|c| c == target_link) {
            continue;
        }
        // Explicit list: skip listed paths. Empty list: effective = snapshot vs
        // current (old) base — a divergent snapshot is a real override.
        let blocked = materialized_override_list(game_dir, &asset)?;
        let inherited: Vec<String> = paths
            .iter()
            .filter(|p| !blocked.iter().any(|have| have == *p))
            .cloned()
            .collect();
        if inherited.is_empty() {
            continue;
        }
        pending.push((path, inherited));
    }
    Ok(pending)
}

fn apply_inherited_snapshot_updates(
    value_src: &Prefab,
    pending: Vec<(PathBuf, Vec<String>)>,
) -> Result<()> {
    for (path, inherited) in pending {
        let mut asset = crate::scene::load_prefab(&path)?;
        apply_override_paths(&mut asset, value_src, &inherited)?;
        crate::scene::save_prefab(&path, &asset)?;
    }
    Ok(())
}

fn record_paths_on_variant(game_dir: &Path, asset: &mut Prefab, paths: &[String]) -> Result<()> {
    if asset.overrides.is_empty() {
        refresh_variant_overrides(game_dir, asset)?;
        return Ok(());
    }
    let base_src = asset
        .base
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("variant is missing a base"))?;
    let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &base_src)?)?;
    let base_resolved = resolve_prefab(game_dir, &base_raw)?;
    for path in paths {
        if path_is_override(asset, &base_resolved, path) {
            if !asset.overrides.iter().any(|p| p == path) {
                asset.overrides.push(path.clone());
            }
        } else {
            asset.overrides.retain(|p| p != path);
        }
    }
    Ok(())
}

fn path_is_override(asset: &Prefab, base_resolved: &Prefab, path: &str) -> bool {
    if let Some(child) = path.strip_suffix("/<added>") {
        return asset.children.iter().any(|c| c.name == child)
            && !base_resolved.children.iter().any(|c| c.name == child);
    }
    if let Some(child) = path.strip_suffix("/<missing>") {
        return !asset.children.iter().any(|c| c.name == child)
            && base_resolved.children.iter().any(|c| c.name == child);
    }
    if let Some((child, field)) = path.split_once('/') {
        let Some(a) = asset.children.iter().find(|c| c.name == child) else {
            return false;
        };
        let Some(b) = base_resolved.children.iter().find(|c| c.name == child) else {
            return true;
        };
        return prefab_overrides(a, b).contains(field);
    }
    prefab_overrides(&asset.entity, &base_resolved.entity).contains(path)
}

/// Rebuild a variant snapshot as resolved base + its explicit overrides.
fn realign_variant_snapshot(
    game_dir: &Path,
    variant: &mut Prefab,
    value_from: &Prefab,
) -> Result<()> {
    let base_src = variant
        .base
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("not a prefab variant"))?;
    let base_raw = crate::scene::load_prefab(&resolve_prefab_asset(game_dir, &base_src)?)?;
    let resolved = resolve_prefab(game_dir, &base_raw)?;
    let keep_name = variant.entity.name.clone();
    let keep_overrides = variant.overrides.clone();
    let keep_base = variant.base.clone();
    let mut next = resolved;
    apply_override_paths(&mut next, value_from, &keep_overrides)?;
    if next.entity.name != keep_name {
        let old = next.entity.name.clone();
        next.entity.name = keep_name.clone();
        for child in &mut next.children {
            if child.parent.as_deref() == Some(old.as_str()) {
                child.parent = Some(keep_name.clone());
            }
        }
    }
    next.base = keep_base;
    next.overrides = keep_overrides;
    *variant = next;
    Ok(())
}

fn overlay_instance_values(
    scene: &Scene,
    instance_root: &str,
    shape: &Prefab,
    dest: &mut Prefab,
) -> Result<()> {
    let map = match_prefab_instance(scene, instance_root, shape);
    if let Some(inst_name) = map.get(&shape.entity.name) {
        if let Some(inst) = scene.find_entity(inst_name) {
            dest.entity.transform = inst.transform.clone();
            dest.entity.components = inst.components.clone();
            dest.entity.tag = inst.tag;
        }
    }
    for child in &mut dest.children {
        let Some(inst_name) = map.get(&child.name) else {
            continue;
        };
        let Some(inst) = scene.find_entity(inst_name) else {
            continue;
        };
        child.transform = inst.transform.clone();
        child.components = inst.components.clone();
        child.tag = inst.tag;
    }
    Ok(())
}

/// True for override paths the variant resolver understands (`Disc.color`, `Eye/Disc.radius`).
pub fn is_known_override_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    let field = if let Some((child, field)) = path.split_once('/') {
        if child.is_empty() || field.is_empty() {
            return false;
        }
        field
    } else {
        path
    };
    is_known_override_field(field)
}

fn is_known_override_field(field: &str) -> bool {
    matches!(
        field,
        "<added>"
            | "<missing>"
            | "transform.position"
            | "transform.rotation"
            | "transform.scale"
            | "tag"
            | "Sprite"
            | "Sprite.texture"
            | "Sprite.size"
            | "Sprite.color"
            | "Sprite.z"
            | "Sprite.sorting_layer"
            | "Sprite.enabled"
            | "Sprite.pivot"
            | "Disc"
            | "Disc.radius"
            | "Disc.color"
            | "Disc.z"
            | "Disc.sorting_layer"
            | "Disc.enabled"
            | "Camera"
            | "Camera.active"
            | "Camera.follow"
            | "Camera.lerp"
            | "Tilemap"
            | "Tilemap.cell"
            | "Tilemap.origin"
            | "Tilemap.size"
            | "Tilemap.cells"
            | "Tilemap.solid"
            | "Tilemap.palette"
            | "Tilemap.z"
            | "Tilemap.sorting_layer"
            | "Tilemap.enabled"
            | "Collider"
            | "Collider.kind"
            | "Collider.size"
            | "Collider.radius"
            | "Collider.offset"
            | "Collider.solid"
            | "Collider.trigger"
            | "Collider.filter_tag"
            | "Collider.enabled"
            | "Animation"
            | "Animation.clip"
            | "Animation.fps"
            | "Animation.loop"
            | "Animation.enabled"
            | "Animator"
            | "Animator.controller"
            | "Animator.parameters"
            | "Animator.enabled"
            | "PlayableDirector"
            | "PlayableDirector.timeline"
            | "PlayableDirector.play_on_awake"
            | "PlayableDirector.loop"
            | "PlayableDirector.enabled"
            | "GridMover"
            | "GridMover.cell"
            | "GridMover.speed"
            | "GridMover.queued_dir"
            | "GridMover.enabled"
            | "AudioSource"
            | "AudioSource.clip"
            | "AudioSource.volume"
            | "AudioSource.play_on_awake"
            | "AudioSource.enabled"
            | "Text"
            | "Text.text"
            | "Text.size"
            | "Text.color"
            | "Text.align"
            | "Text.z"
            | "Text.sorting_layer"
            | "Text.enabled"
    )
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
        "Tilemap.cell"
        | "Tilemap.origin"
        | "Tilemap.size"
        | "Tilemap.cells"
        | "Tilemap.solid"
        | "Tilemap.palette"
        | "Tilemap.z"
        | "Tilemap.sorting_layer"
        | "Tilemap.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.tilemap, &src.components.tilemap) {
                match field {
                    "Tilemap.cell" => d.cell = s.cell,
                    "Tilemap.origin" => d.origin = s.origin,
                    "Tilemap.size" => {
                        d.width = s.width;
                        d.height = s.height;
                    }
                    "Tilemap.cells" => d.cells = s.cells.clone(),
                    "Tilemap.solid" => d.solid = s.solid.clone(),
                    "Tilemap.palette" => d.palette = s.palette.clone(),
                    "Tilemap.z" => d.z = s.z,
                    "Tilemap.sorting_layer" => d.sorting_layer = s.sorting_layer.clone(),
                    "Tilemap.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.tilemap = src.components.tilemap.clone();
            }
        }
        "Collider" => dest.components.collider = src.components.collider.clone(),
        "Collider.kind"
        | "Collider.size"
        | "Collider.radius"
        | "Collider.offset"
        | "Collider.solid"
        | "Collider.trigger"
        | "Collider.filter_tag"
        | "Collider.enabled" => {
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
        "Animator" => dest.components.animator = src.components.animator.clone(),
        "Animator.controller" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animator, &src.components.animator) {
                d.controller = s.controller.clone();
            } else {
                dest.components.animator = src.components.animator.clone();
            }
        }
        "PlayableDirector" => {
            dest.components.playable_director = src.components.playable_director.clone()
        }
        "PlayableDirector.timeline"
        | "PlayableDirector.play_on_awake"
        | "PlayableDirector.loop"
        | "PlayableDirector.enabled" => {
            if let (Some(d), Some(s)) = (
                &mut dest.components.playable_director,
                &src.components.playable_director,
            ) {
                match field {
                    "PlayableDirector.timeline" => d.timeline = s.timeline.clone(),
                    "PlayableDirector.play_on_awake" => d.play_on_awake = s.play_on_awake,
                    "PlayableDirector.loop" => d.loop_ = s.loop_,
                    "PlayableDirector.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.playable_director = src.components.playable_director.clone();
            }
        }
        "Animator.parameters" | "Animator.enabled" => {
            if let (Some(d), Some(s)) = (&mut dest.components.animator, &src.components.animator) {
                match field {
                    "Animator.parameters" => d.parameters = s.parameters.clone(),
                    "Animator.enabled" => d.enabled = s.enabled,
                    _ => {}
                }
            } else {
                dest.components.animator = src.components.animator.clone();
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
        "AudioSource.clip"
        | "AudioSource.volume"
        | "AudioSource.play_on_awake"
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
    match (&a.animator, &b.animator) {
        (Some(i), Some(p)) => {
            if i.controller != p.controller {
                out.push("Animator.controller".into());
            }
            if i.parameters != p.parameters {
                out.push("Animator.parameters".into());
            }
            if i.enabled != p.enabled {
                out.push("Animator.enabled".into());
            }
        }
        (Some(_), None) | (None, Some(_)) => out.push("Animator".into()),
        (None, None) => {}
    }
    match (&a.playable_director, &b.playable_director) {
        (Some(i), Some(p)) => {
            if i.timeline != p.timeline {
                out.push("PlayableDirector.timeline".into());
            }
            if i.play_on_awake != p.play_on_awake {
                out.push("PlayableDirector.play_on_awake".into());
            }
            if i.loop_ != p.loop_ {
                out.push("PlayableDirector.loop".into());
            }
            if i.enabled != p.enabled {
                out.push("PlayableDirector.enabled".into());
            }
        }
        (Some(_), None) | (None, Some(_)) => out.push("PlayableDirector".into()),
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
        let json =
            r#"{"name":"Ghost","transform":{"translation":[1.0,2.0,0.0]},"components":{},"tag":0}"#;
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
        let json =
            r#"{"name":"Ghost","transform":{"translation":[1.0,2.0,0.0]},"components":{},"tag":0}"#;
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
        assert_eq!(
            back.base.as_deref(),
            Some("assets/prefabs/ghost.prefab.json")
        );
        assert_eq!(back.overrides, vec!["Disc.color".to_string()]);
    }
}
