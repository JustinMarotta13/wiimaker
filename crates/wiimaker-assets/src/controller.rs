//! Animator controller sidecars (`assets/<name>.controller.json`).
//!
//! A controller is a small state machine: named clip states, Bool/Float/Trigger
//! parameters, and transitions. Clip cells stay in `*.anim.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

fn default_speed() -> f32 {
    1.0
}

/// Parameter kind on an animator controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerParamType {
    Bool,
    Float,
    Trigger,
}

impl ControllerParamType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bool" | "boolean" => Some(Self::Bool),
            "float" | "number" => Some(Self::Float),
            "trigger" => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// Authoring parameter with a default value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerParam {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ControllerParamType,
    /// Bool / trigger default, or omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

impl ControllerParam {
    pub fn default_bool(&self) -> bool {
        match self.kind {
            ControllerParamType::Bool | ControllerParamType::Trigger => self
                .default
                .as_ref()
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            ControllerParamType::Float => false,
        }
    }

    pub fn default_float(&self) -> f32 {
        match self.kind {
            ControllerParamType::Float => self
                .default
                .as_ref()
                .and_then(|v| v.as_f64())
                .map(|f| f as f32)
                .unwrap_or(0.0),
            ControllerParamType::Bool | ControllerParamType::Trigger => 0.0,
        }
    }
}

/// Blend dimension on a state (`"1D"` / `"2D"` in JSON).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendDimension {
    #[serde(rename = "1D")]
    OneD,
    #[serde(rename = "2D")]
    TwoD,
}

impl BlendDimension {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "1D" => Some(Self::OneD),
            "2D" => Some(Self::TwoD),
            _ => None,
        }
    }

    pub fn param_count(self) -> usize {
        match self {
            Self::OneD => 1,
            Self::TwoD => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneD => "1D",
            Self::TwoD => "2D",
        }
    }
}

/// Child motion in a blend tree: `threshold` for 1D, `position` `[x, y]` for 2D.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendMotion {
    pub clip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
}

/// 1D or 2D blend tree. `params` lists Float parameter names (one for 1D, X then Y for 2D).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendTreeMeta {
    #[serde(rename = "type")]
    pub dimension: BlendDimension,
    pub params: Vec<String>,
    #[serde(default)]
    pub motions: Vec<BlendMotion>,
}

impl BlendTreeMeta {
    pub fn validate(&self, state: &str) -> Result<()> {
        let want = self.dimension.param_count();
        if self.params.len() != want {
            bail!(
                "state '{state}' blend tree {} needs {want} param(s), got {}",
                self.dimension.as_str(),
                self.params.len()
            );
        }
        for p in &self.params {
            if p.trim().is_empty() {
                bail!("state '{state}' blend tree has an empty param name");
            }
        }
        if self.dimension == BlendDimension::TwoD && self.params[0] == self.params[1] {
            bail!("state '{state}' blend tree 2D needs two different params");
        }
        for m in &self.motions {
            if m.clip.is_empty() {
                bail!("state '{state}' blend motion has empty clip");
            }
            match self.dimension {
                BlendDimension::OneD => {
                    if m.position.is_some() {
                        bail!(
                            "state '{state}' 1D motion '{}' uses position (use threshold)",
                            m.clip
                        );
                    }
                    match m.threshold {
                        Some(t) if t.is_finite() => {}
                        _ => bail!(
                            "state '{state}' 1D motion '{}' needs a finite threshold",
                            m.clip
                        ),
                    }
                }
                BlendDimension::TwoD => {
                    if m.threshold.is_some() {
                        bail!(
                            "state '{state}' 2D motion '{}' uses threshold (use position)",
                            m.clip
                        );
                    }
                    match m.position {
                        Some(p) if p[0].is_finite() && p[1].is_finite() => {}
                        _ => bail!(
                            "state '{state}' 2D motion '{}' needs a finite position",
                            m.clip
                        ),
                    }
                }
            }
        }
        Ok(())
    }
}

/// One named state → clip stem, or a blend tree (then `clip` stays empty).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerState {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clip: String,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_tree: Option<BlendTreeMeta>,
}

impl ControllerState {
    pub fn plain(name: impl Into<String>, clip: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            clip: clip.into(),
            speed: default_speed(),
            blend_tree: None,
        }
    }
}

/// Transition condition (AND together on a transition).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ControllerCondition {
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub less: Option<f32>,
}

/// Directed edge between states. `from` may be `"Any"`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerTransition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub conditions: Vec<ControllerCondition>,
    #[serde(default)]
    pub has_exit_time: bool,
}

/// Authoring animator controller (`assets/<name>.controller.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorControllerMeta {
    #[serde(rename = "default")]
    pub default_state: String,
    #[serde(default)]
    pub parameters: Vec<ControllerParam>,
    pub states: Vec<ControllerState>,
    #[serde(default)]
    pub transitions: Vec<ControllerTransition>,
}

impl AnimatorControllerMeta {
    pub fn path(assets_dir: impl AsRef<Path>, name: &str) -> PathBuf {
        assets_dir.as_ref().join(format!("{name}.controller.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let meta: Self =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        meta.validate()?;
        Ok(meta)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text + "\n").with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn state(&self, name: &str) -> Option<&ControllerState> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn validate(&self) -> Result<()> {
        if self.states.is_empty() {
            bail!("animator controller must list at least one state");
        }
        let mut seen = HashMap::new();
        for s in &self.states {
            if s.name.is_empty() {
                bail!("animator state name must not be empty");
            }
            match (&s.blend_tree, s.clip.is_empty()) {
                (Some(_), false) => {
                    bail!("animator state '{}' sets both clip and blend_tree", s.name)
                }
                (Some(bt), true) => bt.validate(&s.name)?,
                (None, true) => bail!("animator state '{}' has empty clip", s.name),
                (None, false) => {}
            }
            if seen.insert(s.name.as_str(), ()).is_some() {
                bail!("duplicate animator state '{}'", s.name);
            }
        }
        if self.default_state.is_empty() {
            bail!("animator controller default state must not be empty");
        }
        if self.state(&self.default_state).is_none() {
            bail!(
                "animator default state '{}' is not in states",
                self.default_state
            );
        }
        let mut pseen = HashMap::new();
        for p in &self.parameters {
            if p.name.is_empty() {
                bail!("animator parameter name must not be empty");
            }
            if pseen.insert(p.name.as_str(), ()).is_some() {
                bail!("duplicate animator parameter '{}'", p.name);
            }
        }
        for t in &self.transitions {
            if !is_any_state(&t.from) && self.state(&t.from).is_none() {
                bail!("animator transition from unknown state '{}'", t.from);
            }
            if self.state(&t.to).is_none() {
                bail!("animator transition to unknown state '{}'", t.to);
            }
        }
        Ok(())
    }
}

/// `Any` / `*` matches every from-state at runtime.
pub fn is_any_state(name: &str) -> bool {
    let n = name.trim();
    n.eq_ignore_ascii_case("any") || n == "*"
}

/// Write / overwrite `assets/<name>.controller.json`.
pub fn write_animator_controller(
    assets_dir: &Path,
    name: &str,
    meta: AnimatorControllerMeta,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    if name.is_empty() {
        bail!("animator controller name must not be empty");
    }
    meta.validate()?;
    let path = AnimatorControllerMeta::path(assets_dir, name);
    meta.save(&path)?;
    Ok((path, meta))
}

/// Set `state` to a blend tree on an existing controller (adds the state when missing).
///
/// CLI `asset blend-tree` and the editor Inspector both call this, so their files match.
pub fn write_state_blend_tree(
    assets_dir: &Path,
    controller: &str,
    state: &str,
    tree: BlendTreeMeta,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    tree.validate(state)?;
    let path = AnimatorControllerMeta::path(assets_dir, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    match meta.states.iter_mut().find(|s| s.name == state) {
        Some(s) => {
            s.clip.clear();
            s.blend_tree = Some(tree);
        }
        None => meta.states.push(ControllerState {
            name: state.to_string(),
            clip: String::new(),
            speed: default_speed(),
            blend_tree: Some(tree),
        }),
    }
    meta.save(&path)?;
    Ok((path, meta))
}

/// Replace `state`'s blend tree with a plain clip. The state must already exist.
pub fn write_state_clip(
    assets_dir: &Path,
    controller: &str,
    state: &str,
    clip: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    if clip.trim().is_empty() {
        bail!("state '{state}' clip must not be empty");
    }
    let path = AnimatorControllerMeta::path(assets_dir, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    let Some(s) = meta.states.iter_mut().find(|s| s.name == state) else {
        bail!("animator controller '{controller}' has no state '{state}'");
    };
    s.clip = clip.to_string();
    s.blend_tree = None;
    meta.save(&path)?;
    Ok((path, meta))
}

/// Stem names of every `*.controller.json` under `assets_dir`.
pub fn list_animator_controllers(assets_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !assets_dir.is_dir() {
        return Ok(names);
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(stem) = fname.strip_suffix(".controller.json") {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Lookup table of animator controllers by stem.
#[derive(Clone, Debug, Default)]
pub struct AnimatorControllerCatalog {
    by_name: HashMap<String, AnimatorControllerMeta>,
    names: Vec<String>,
}

impl AnimatorControllerCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn lookup(&self, name: &str) -> Option<&AnimatorControllerMeta> {
        self.by_name.get(name)
    }

    pub fn load_dir(assets_dir: &Path) -> Result<Self> {
        let mut cat = Self::empty();
        if !assets_dir.is_dir() {
            return Ok(cat);
        }
        for name in list_animator_controllers(assets_dir)? {
            let path = AnimatorControllerMeta::path(assets_dir, &name);
            let meta = AnimatorControllerMeta::load(&path)?;
            cat.names.push(name.clone());
            cat.by_name.insert(name, meta);
        }
        Ok(cat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn sample() -> AnimatorControllerMeta {
        AnimatorControllerMeta {
            default_state: "Idle".into(),
            parameters: vec![ControllerParam {
                name: "Moving".into(),
                kind: ControllerParamType::Bool,
                default: Some(serde_json::json!(false)),
            }],
            states: vec![
                ControllerState::plain("Idle", "idle"),
                ControllerState::plain("Walk", "walk"),
            ],
            transitions: vec![
                ControllerTransition {
                    from: "Idle".into(),
                    to: "Walk".into(),
                    conditions: vec![ControllerCondition {
                        param: "Moving".into(),
                        equals: Some(serde_json::json!(true)),
                        ..Default::default()
                    }],
                    has_exit_time: false,
                },
                ControllerTransition {
                    from: "Walk".into(),
                    to: "Idle".into(),
                    conditions: vec![ControllerCondition {
                        param: "Moving".into(),
                        equals: Some(serde_json::json!(false)),
                        ..Default::default()
                    }],
                    has_exit_time: false,
                },
            ],
        }
    }

    #[test]
    fn roundtrip_controller_json() {
        let dir = env::temp_dir().join(format!("wiimaker-ctrl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (path, meta) = write_animator_controller(&dir, "player", sample()).unwrap();
        assert!(path.ends_with("player.controller.json"));
        assert_eq!(meta.default_state, "Idle");
        let loaded = AnimatorControllerMeta::load(&path).unwrap();
        assert_eq!(loaded.states.len(), 2);
        assert_eq!(loaded.parameters[0].default_bool(), false);
        let names = list_animator_controllers(&dir).unwrap();
        assert_eq!(names, vec!["player"]);
        let cat = AnimatorControllerCatalog::load_dir(&dir).unwrap();
        assert!(cat.lookup("player").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_default_state() {
        let mut m = sample();
        m.default_state = "Nope".into();
        assert!(m.validate().is_err());
    }

    fn motion1d(clip: &str, t: f32) -> BlendMotion {
        BlendMotion {
            clip: clip.into(),
            threshold: Some(t),
            position: None,
        }
    }

    fn motion2d(clip: &str, x: f32, y: f32) -> BlendMotion {
        BlendMotion {
            clip: clip.into(),
            threshold: None,
            position: Some([x, y]),
        }
    }

    fn tree1d() -> BlendTreeMeta {
        BlendTreeMeta {
            dimension: BlendDimension::OneD,
            params: vec!["Speed".into()],
            motions: vec![motion1d("idle", 0.0), motion1d("walk", 1.0)],
        }
    }

    fn tree2d() -> BlendTreeMeta {
        BlendTreeMeta {
            dimension: BlendDimension::TwoD,
            params: vec!["DirX".into(), "DirY".into()],
            motions: vec![
                motion2d("idle", 0.0, 0.0),
                motion2d("up", 0.0, -1.0),
                motion2d("right", 1.0, 0.0),
            ],
        }
    }

    fn with_blend(tree: BlendTreeMeta) -> AnimatorControllerMeta {
        AnimatorControllerMeta {
            default_state: "Locomotion".into(),
            parameters: vec![ControllerParam {
                name: "Speed".into(),
                kind: ControllerParamType::Float,
                default: Some(serde_json::json!(0.0)),
            }],
            states: vec![ControllerState {
                name: "Locomotion".into(),
                clip: String::new(),
                speed: 1.0,
                blend_tree: Some(tree),
            }],
            transitions: Vec::new(),
        }
    }

    fn tmp_dir(label: &str) -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!("wiimaker-blend-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn legacy_controller_json_resaves_byte_identical() {
        let legacy = "{\n  \"default\": \"Idle\",\n  \"parameters\": [\n    {\n      \"name\": \"Moving\",\n      \"type\": \"Bool\",\n      \"default\": false\n    }\n  ],\n  \"states\": [\n    {\n      \"name\": \"Idle\",\n      \"clip\": \"idle\",\n      \"speed\": 1.0\n    }\n  ],\n  \"transitions\": []\n}\n";
        let dir = tmp_dir("legacy");
        let path = dir.join("p.controller.json");
        fs::write(&path, legacy).unwrap();
        let meta = AnimatorControllerMeta::load(&path).unwrap();
        assert!(meta.states[0].blend_tree.is_none());
        meta.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn blend_tree_roundtrip_is_identical() {
        for tree in [tree1d(), tree2d()] {
            let meta = with_blend(tree.clone());
            let dir = tmp_dir("roundtrip");
            let (path, _) = write_animator_controller(&dir, "ctrl", meta).unwrap();
            let first = fs::read_to_string(&path).unwrap();
            let loaded = AnimatorControllerMeta::load(&path).unwrap();
            assert_eq!(loaded.states[0].blend_tree.as_ref(), Some(&tree));
            assert!(loaded.states[0].clip.is_empty());
            loaded.save(&path).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), first);
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn blend_tree_json_shape() {
        let text = serde_json::to_string(&with_blend(tree1d())).unwrap();
        assert!(text.contains("\"type\":\"1D\""), "{text}");
        assert!(text.contains("\"params\":[\"Speed\"]"), "{text}");
        assert!(text.contains("\"threshold\":1.0"), "{text}");
        assert!(!text.contains("\"clip\":\"\""), "{text}");
        let text2 = serde_json::to_string(&with_blend(tree2d())).unwrap();
        assert!(text2.contains("\"type\":\"2D\""), "{text2}");
        assert!(text2.contains("\"position\":[0.0,-1.0]"), "{text2}");
    }

    #[test]
    fn validate_rejects_bad_blend_trees() {
        let mut t = tree1d();
        t.params.push("Extra".into());
        assert!(with_blend(t).validate().is_err());

        let mut t = tree1d();
        t.motions[0].threshold = None;
        assert!(with_blend(t).validate().is_err());

        let mut t = tree2d();
        t.motions[0].position = None;
        assert!(with_blend(t).validate().is_err());

        let mut t = tree2d();
        t.motions[1].threshold = Some(0.5);
        assert!(with_blend(t).validate().is_err());

        let mut t = tree2d();
        t.params[1] = t.params[0].clone();
        assert!(with_blend(t).validate().is_err());

        let mut t = tree1d();
        t.motions[0].threshold = Some(f32::INFINITY);
        assert!(with_blend(t).validate().is_err());

        let mut m = with_blend(tree1d());
        m.states[0].clip = "idle".into();
        assert!(m.validate().is_err());

        let mut m = with_blend(tree1d());
        m.states[0].blend_tree = None;
        assert!(m.validate().is_err());
    }

    #[test]
    fn empty_blend_tree_is_allowed() {
        let t = BlendTreeMeta {
            dimension: BlendDimension::OneD,
            params: vec!["Speed".into()],
            motions: Vec::new(),
        };
        assert!(with_blend(t).validate().is_ok());
    }

    #[test]
    fn write_state_blend_tree_adds_or_replaces_and_clip_restores() {
        let dir = tmp_dir("write");
        let mut base = sample();
        base.parameters.push(ControllerParam {
            name: "Speed".into(),
            kind: ControllerParamType::Float,
            default: Some(serde_json::json!(0.0)),
        });
        write_animator_controller(&dir, "player", base).unwrap();

        let (path, meta) = write_state_blend_tree(&dir, "player", "Locomotion", tree1d()).unwrap();
        assert!(path.ends_with("player.controller.json"));
        assert_eq!(meta.state("Locomotion").unwrap().blend_tree, Some(tree1d()));

        let (_, meta) = write_state_blend_tree(&dir, "player", "Walk", tree2d()).unwrap();
        assert!(meta.state("Walk").unwrap().clip.is_empty());
        assert_eq!(meta.state("Walk").unwrap().blend_tree, Some(tree2d()));

        let (_, meta) = write_state_blend_tree(&dir, "player", "Walk", tree1d()).unwrap();
        assert!(meta.state("Walk").unwrap().blend_tree.is_some());
        assert!(meta.state("Walk").unwrap().clip.is_empty());

        let (_, meta) = write_state_clip(&dir, "player", "Walk", "walk").unwrap();
        assert!(meta.state("Walk").unwrap().blend_tree.is_none());
        assert_eq!(meta.state("Walk").unwrap().clip, "walk");

        assert!(write_state_clip(&dir, "player", "Nope", "walk").is_err());
        assert!(write_state_blend_tree(&dir, "player", "X", {
            let mut t = tree1d();
            t.params.clear();
            t
        })
        .is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
