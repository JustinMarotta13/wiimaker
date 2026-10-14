//! Animator controller sidecars (`assets/<name>.controller.json`).
//!
//! A controller is a small state machine: named clip states, Bool/Float/Trigger
//! parameters, and transitions. Clip cells stay in `*.anim.json`.
//!
//! Optional `layers` are extra state machines (Unity override layers). The
//! top-level `default` / `states` / `transitions` are the base layer. Parameters
//! stay controller-wide. Sprites cannot bone-blend, so layers are Override only:
//! no avatar masks and no Additive blending. A layer with `weight > 0` whose
//! current state has a clip or blend tree replaces the base clip.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::atomic_io::atomic_write;

fn default_speed() -> f32 {
    1.0
}

fn default_layer_weight() -> f32 {
    1.0
}

/// True when a serialized weight is the default `1` (omit it so old files stay stable).
fn is_default_weight(w: &f32) -> bool {
    w.is_finite() && (*w - 1.0).abs() <= 1e-6
}

/// Layer 0 in the editor and CLI. Not stored in `layers` (that array is overrides only).
pub const BASE_LAYER_NAME: &str = "Base";

/// `Base` / empty selects the top-level state machine.
pub fn is_base_layer_name(name: &str) -> bool {
    let n = name.trim();
    n.is_empty() || n.eq_ignore_ascii_case(BASE_LAYER_NAME)
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
    /// Animator window node position `[x, y]` in graph space. Omitted when unset
    /// so files without a layout resave unchanged. Missing → auto-layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
}

impl ControllerState {
    pub fn plain(name: impl Into<String>, clip: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            clip: clip.into(),
            speed: default_speed(),
            blend_tree: None,
            position: None,
        }
    }
}

/// Optional Animator-window positions for the Entry / Any State / Exit nodes.
///
/// Omitted entirely when every field is unset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControllerGraph {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<[f32; 2]>,
}

impl ControllerGraph {
    pub fn is_empty(&self) -> bool {
        self.entry.is_none() && self.any.is_none() && self.exit.is_none()
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

/// Override layer: its own state machine and a weight in 0..1 (clamped at runtime).
///
/// Omitted on old controllers. Index 0 of the runtime animator is always [`BASE_LAYER_NAME`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerLayer {
    pub name: String,
    #[serde(default = "default_layer_weight")]
    pub weight: f32,
    #[serde(rename = "default", default, skip_serializing_if = "String::is_empty")]
    pub default_state: String,
    #[serde(default)]
    pub states: Vec<ControllerState>,
    #[serde(default)]
    pub transitions: Vec<ControllerTransition>,
    /// Entry / Any / Exit node positions for this override layer.
    #[serde(default, skip_serializing_if = "ControllerGraph::is_empty")]
    pub graph: ControllerGraph,
}

/// Authoring animator controller (`assets/<name>.controller.json`).
///
/// Files without `layers` load as a single base machine. `weight` is the base
/// layer weight (default 1, omitted when it is 1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorControllerMeta {
    #[serde(rename = "default")]
    pub default_state: String,
    #[serde(default)]
    pub parameters: Vec<ControllerParam>,
    pub states: Vec<ControllerState>,
    #[serde(default)]
    pub transitions: Vec<ControllerTransition>,
    /// Base layer weight. Override weights live on [`ControllerLayer`].
    #[serde(
        default = "default_layer_weight",
        skip_serializing_if = "is_default_weight"
    )]
    pub weight: f32,
    /// Override layers (runtime index 1+). Empty / omitted = base only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ControllerLayer>,
    /// Entry / Any / Exit positions for the base layer.
    #[serde(default, skip_serializing_if = "ControllerGraph::is_empty")]
    pub graph: ControllerGraph,
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
        self.ensure_finite_weights()?;
        self.validate()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        atomic_write(path, (text + "\n").as_bytes())
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn state(&self, name: &str) -> Option<&ControllerState> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn layer(&self, name: &str) -> Option<&ControllerLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn layer_mut(&mut self, name: &str) -> Option<&mut ControllerLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// JSON cannot hold NaN or ±inf (serde writes `null`, which then fails to load as `f32`).
    fn ensure_finite_weights(&self) -> Result<()> {
        if !self.weight.is_finite() {
            bail!(
                "animator base weight must be a finite number, got {}",
                self.weight
            );
        }
        for layer in &self.layers {
            if !layer.weight.is_finite() {
                bail!(
                    "animator layer '{}' weight must be a finite number, got {}",
                    layer.name,
                    layer.weight
                );
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.states.is_empty() {
            bail!("animator controller must list at least one state");
        }
        validate_machine(
            "base",
            &self.default_state,
            &self.states,
            &self.transitions,
            true,
        )?;
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
        for layer in &self.layers {
            if layer.name.trim().is_empty() {
                bail!("animator layer name must not be empty");
            }
            if is_base_layer_name(&layer.name) {
                bail!(
                    "animator layer name '{}' is reserved for the base layer",
                    layer.name
                );
            }
            // Duplicate names and a default that is not a state are doctor warnings
            // so a hand-edited file still loads. Empty layers are allowed.
            validate_machine(
                &format!("layer '{}'", layer.name),
                &layer.default_state,
                &layer.states,
                &layer.transitions,
                true,
            )?;
        }
        Ok(())
    }
}

/// Shared state/transition checks. `allow_placeholder` lets an override state omit its clip.
fn validate_machine(
    label: &str,
    default_state: &str,
    states: &[ControllerState],
    transitions: &[ControllerTransition],
    allow_placeholder: bool,
) -> Result<()> {
    let mut seen = HashMap::new();
    for s in states {
        if s.name.is_empty() {
            bail!("animator state name must not be empty ({label})");
        }
        match (&s.blend_tree, s.clip.is_empty()) {
            (Some(_), false) => {
                bail!("animator state '{}' sets both clip and blend_tree", s.name)
            }
            (Some(bt), true) => bt.validate(&s.name)?,
            (None, true) if allow_placeholder => {}
            (None, true) => bail!("animator state '{}' has empty clip", s.name),
            (None, false) => {}
        }
        if seen.insert(s.name.as_str(), ()).is_some() {
            bail!("duplicate animator state '{}'", s.name);
        }
    }
    let known = |name: &str| states.iter().any(|s| s.name == name);
    if !default_state.is_empty() && !states.is_empty() && !known(default_state) {
        // Unknown defaults on override layers are a doctor warning, not a load error.
        if !allow_placeholder {
            bail!("animator default state '{default_state}' is not in states");
        }
    }
    for t in transitions {
        if !is_any_state(&t.from) && !known(&t.from) {
            bail!("animator transition from unknown state '{}'", t.from);
        }
        if !known(&t.to) {
            bail!("animator transition to unknown state '{}'", t.to);
        }
    }
    Ok(())
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

/// Set `state` to a blend tree on the base layer (adds the state when missing).
///
/// CLI `asset blend-tree` and the editor Inspector both call this, so their files match.
pub fn write_state_blend_tree(
    assets_dir: &Path,
    controller: &str,
    state: &str,
    tree: BlendTreeMeta,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    write_layer_state_blend_tree(assets_dir, controller, None, state, tree)
}

/// Same as [`write_state_blend_tree`], scoped to an override layer (`None` / `Base` = base).
pub fn write_layer_state_blend_tree(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    state: &str,
    tree: BlendTreeMeta,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    tree.validate(state)?;
    edit_controller(assets_dir, controller, |meta| {
        let (states, default) = machine_mut(meta, layer)?;
        match states.iter_mut().find(|s| s.name == state) {
            Some(s) => {
                s.clip.clear();
                s.blend_tree = Some(tree);
            }
            None => {
                let mut created = ControllerState::plain(state, "");
                created.blend_tree = Some(tree);
                states.push(created);
                if default.is_empty() {
                    *default = state.to_string();
                }
            }
        }
        Ok(())
    })
}

/// Replace `state`'s blend tree with a plain clip on the base layer. The state must already exist.
pub fn write_state_clip(
    assets_dir: &Path,
    controller: &str,
    state: &str,
    clip: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    write_layer_state_clip(assets_dir, controller, None, state, clip)
}

/// Same as [`write_state_clip`], scoped to a layer. An empty clip is a placeholder (no motion).
pub fn write_layer_state_clip(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    state: &str,
    clip: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let (states, _) = machine_mut(meta, layer)?;
        let Some(s) = states.iter_mut().find(|s| s.name == state) else {
            bail!("animator controller '{controller}' has no state '{state}'");
        };
        s.clip = clip.trim().to_string();
        s.blend_tree = None;
        Ok(())
    })
}

/// Add or replace a clip state. An empty `clip` is a placeholder (no motion).
pub fn write_layer_state(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    state: &str,
    clip: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    if state.trim().is_empty() {
        bail!("animator state name must not be empty");
    }
    edit_controller(assets_dir, controller, |meta| {
        let (states, default) = machine_mut(meta, layer)?;
        match states.iter_mut().find(|s| s.name == state) {
            Some(s) => {
                s.clip = clip.trim().to_string();
                s.blend_tree = None;
            }
            None => {
                states.push(ControllerState::plain(state.trim(), clip.trim()));
                if default.is_empty() {
                    *default = state.trim().to_string();
                }
            }
        }
        Ok(())
    })
}

pub fn remove_layer_state(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    state: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let base = layer.map(is_base_layer_name).unwrap_or(true);
        let (states, default, transitions) = machine_mut_full(meta, layer)?;
        let before = states.len();
        states.retain(|s| s.name != state);
        if states.len() == before {
            bail!("animator controller '{controller}' has no state '{state}'");
        }
        if base && states.is_empty() {
            bail!("animator controller must list at least one state");
        }
        transitions.retain(|t| t.to != state && (is_any_state(&t.from) || t.from != state));
        if default == state {
            *default = states.first().map(|s| s.name.clone()).unwrap_or_default();
        }
        Ok(())
    })
}

pub fn set_layer_default(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    state: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let base = layer.map(is_base_layer_name).unwrap_or(true);
        let (states, default) = machine_mut(meta, layer)?;
        if states.iter().all(|s| s.name != state) {
            if base {
                bail!("animator default state '{state}' is not in states");
            }
        }
        *default = state.to_string();
        Ok(())
    })
}

pub fn add_layer_transition(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    transition: ControllerTransition,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let (states, _, transitions) = machine_mut_full(meta, layer)?;
        let known = |name: &str| states.iter().any(|s| s.name == name);
        if !is_any_state(&transition.from) && !known(&transition.from) {
            bail!(
                "animator transition from unknown state '{}'",
                transition.from
            );
        }
        if !known(&transition.to) {
            bail!("animator transition to unknown state '{}'", transition.to);
        }
        transitions.push(transition);
        Ok(())
    })
}

pub fn remove_layer_transition(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    index: usize,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let (_, _, transitions) = machine_mut_full(meta, layer)?;
        if index >= transitions.len() {
            bail!("animator transition index {index} is out of range");
        }
        transitions.remove(index);
        Ok(())
    })
}

/// Rejects NaN and ±inf. Finite out-of-range weights are stored as authored and clamped at runtime.
pub fn check_layer_weight(weight: f32) -> Result<()> {
    if !weight.is_finite() {
        bail!("animator layer weight must be a finite number, got {weight}");
    }
    Ok(())
}

pub fn add_controller_layer(
    assets_dir: &Path,
    controller: &str,
    name: &str,
    weight: f32,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    check_layer_weight(weight)?;
    let name = name.trim();
    if name.is_empty() {
        bail!("animator layer name must not be empty");
    }
    if is_base_layer_name(name) {
        bail!("animator layer name '{name}' is reserved for the base layer");
    }
    edit_controller(assets_dir, controller, |meta| {
        if meta
            .layers
            .iter()
            .any(|l| l.name.eq_ignore_ascii_case(name))
        {
            bail!("animator controller '{controller}' already has layer '{name}'");
        }
        meta.layers.push(ControllerLayer {
            name: name.to_string(),
            weight,
            default_state: String::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            graph: ControllerGraph::default(),
        });
        Ok(())
    })
}

pub fn remove_controller_layer(
    assets_dir: &Path,
    controller: &str,
    name: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let before = meta.layers.len();
        meta.layers.retain(|l| l.name != name);
        if meta.layers.len() == before {
            bail!("animator controller '{controller}' has no layer '{name}'");
        }
        Ok(())
    })
}

pub fn rename_controller_layer(
    assets_dir: &Path,
    controller: &str,
    from: &str,
    to: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let to = to.trim();
    if to.is_empty() {
        bail!("animator layer name must not be empty");
    }
    if is_base_layer_name(to) {
        bail!("animator layer name '{to}' is reserved for the base layer");
    }
    edit_controller(assets_dir, controller, |meta| {
        if meta
            .layers
            .iter()
            .any(|l| l.name.eq_ignore_ascii_case(to) && l.name != from)
        {
            bail!("animator controller '{controller}' already has layer '{to}'");
        }
        let Some(layer) = meta.layers.iter_mut().find(|l| l.name == from) else {
            bail!("animator controller '{controller}' has no layer '{from}'");
        };
        layer.name = to.to_string();
        Ok(())
    })
}

/// Store a finite `weight` as authored (runtime clamps out-of-range values to 0..1).
///
/// The base layer weight does not affect playback, so `Base` is rejected. Override
/// layers keep finite out-of-range values raw so doctor can warn.
pub fn set_controller_layer_weight(
    assets_dir: &Path,
    controller: &str,
    layer: &str,
    weight: f32,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    check_layer_weight(weight)?;
    if is_base_layer_name(layer) {
        bail!(
            "animator base layer weight does not affect playback; set an override layer weight instead"
        );
    }
    edit_controller(assets_dir, controller, |meta| {
        let Some(found) = meta.layers.iter_mut().find(|l| l.name == layer) else {
            bail!("animator controller '{controller}' has no layer '{layer}'");
        };
        found.weight = weight;
        Ok(())
    })
}

/// Rename a state and retarget transitions and the layer default that name it.
pub fn rename_layer_state(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    from: &str,
    to: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let to = to.trim();
    if to.is_empty() {
        bail!("animator state name must not be empty");
    }
    if from == to {
        let path = AnimatorControllerMeta::path(assets_dir, controller);
        let meta = AnimatorControllerMeta::load(&path)?;
        return Ok((path, meta));
    }
    edit_controller(assets_dir, controller, |meta| {
        let (states, default, transitions) = machine_mut_full(meta, layer)?;
        if states.iter().any(|s| s.name == to) {
            bail!("animator state '{to}' already exists");
        }
        let Some(state) = states.iter_mut().find(|s| s.name == from) else {
            bail!("animator controller '{controller}' has no state '{from}'");
        };
        state.name = to.to_string();
        if default == from {
            *default = to.to_string();
        }
        for t in transitions.iter_mut() {
            if t.from == from {
                t.from = to.to_string();
            }
            if t.to == from {
                t.to = to.to_string();
            }
        }
        Ok(())
    })
}

/// Move a state node, or Entry / Any / Exit chrome, on one layer.
///
/// An exact state name wins over the Entry/Any/Exit chrome labels.
pub fn move_layer_node(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    node: &str,
    pos: [f32; 2],
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    if !pos[0].is_finite() || !pos[1].is_finite() {
        bail!("animator node position must be finite, got {pos:?}");
    }
    let node = node.trim();
    if node.is_empty() {
        bail!("animator node name must not be empty");
    }
    edit_controller(assets_dir, controller, |meta| {
        let (states, _, _, graph) = machine_mut_graph(meta, layer)?;
        if let Some(state) = states.iter_mut().find(|s| s.name == node) {
            state.position = Some(pos);
            return Ok(());
        }
        match node.to_ascii_lowercase().as_str() {
            "entry" => graph.entry = Some(pos),
            "any" | "any state" => graph.any = Some(pos),
            "exit" => graph.exit = Some(pos),
            _ => bail!("animator controller '{controller}' has no node '{node}'"),
        }
        Ok(())
    })
}

/// Replace conditions and/or `has_exit_time` on one transition.
///
/// `None` leaves that field unchanged. Pass `Some(vec![])` to clear conditions.
pub fn set_layer_transition(
    assets_dir: &Path,
    controller: &str,
    layer: Option<&str>,
    index: usize,
    has_exit_time: Option<bool>,
    conditions: Option<Vec<ControllerCondition>>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let (_, _, transitions) = machine_mut_full(meta, layer)?;
        let Some(t) = transitions.get_mut(index) else {
            bail!("animator transition index {index} is out of range");
        };
        if let Some(flag) = has_exit_time {
            t.has_exit_time = flag;
        }
        if let Some(conditions) = conditions {
            t.conditions = conditions;
        }
        Ok(())
    })
}

pub fn add_controller_param(
    assets_dir: &Path,
    controller: &str,
    param: ControllerParam,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let name = param.name.trim().to_string();
    if name.is_empty() {
        bail!("animator parameter name must not be empty");
    }
    edit_controller(assets_dir, controller, |meta| {
        if meta.parameters.iter().any(|p| p.name == name) {
            bail!("animator controller '{controller}' already has parameter '{name}'");
        }
        meta.parameters.push(ControllerParam {
            name,
            kind: param.kind,
            default: param.default,
        });
        Ok(())
    })
}

/// Rename a parameter in memory (transition conditions, blend-tree params).
pub fn apply_rename_controller_param(
    meta: &mut AnimatorControllerMeta,
    controller: &str,
    from: &str,
    to: &str,
) -> Result<()> {
    let to = to.trim();
    if to.is_empty() {
        bail!("animator parameter name must not be empty");
    }
    if from != to && meta.parameters.iter().any(|p| p.name == to) {
        bail!("animator controller '{controller}' already has parameter '{to}'");
    }
    let Some(param) = meta.parameters.iter_mut().find(|p| p.name == from) else {
        bail!("animator controller '{controller}' has no parameter '{from}'");
    };
    param.name = to.to_string();
    retarget_param_name(meta, from, to);
    Ok(())
}

/// Remove a parameter in memory and strip transition conditions that referenced it.
pub fn apply_remove_controller_param(
    meta: &mut AnimatorControllerMeta,
    controller: &str,
    name: &str,
) -> Result<()> {
    let before = meta.parameters.len();
    meta.parameters.retain(|p| p.name != name);
    if meta.parameters.len() == before {
        bail!("animator controller '{controller}' has no parameter '{name}'");
    }
    strip_param_references(meta, name);
    Ok(())
}

/// Rename a parameter and retarget transition conditions and blend-tree param names.
pub fn rename_controller_param(
    assets_dir: &Path,
    controller: &str,
    from: &str,
    to: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        apply_rename_controller_param(meta, controller, from, to)
    })
}

pub fn remove_controller_param(
    assets_dir: &Path,
    controller: &str,
    name: &str,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        apply_remove_controller_param(meta, controller, name)
    })
}

pub fn set_controller_param_default(
    assets_dir: &Path,
    controller: &str,
    name: &str,
    default: serde_json::Value,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    edit_controller(assets_dir, controller, |meta| {
        let Some(param) = meta.parameters.iter_mut().find(|p| p.name == name) else {
            bail!("animator controller '{controller}' has no parameter '{name}'");
        };
        match param.kind {
            ControllerParamType::Float => {
                if !default.is_number() {
                    bail!("parameter '{name}' default must be a number");
                }
            }
            ControllerParamType::Bool | ControllerParamType::Trigger => {
                if !default.is_boolean() {
                    bail!("parameter '{name}' default must be true or false");
                }
            }
        }
        param.default = Some(default);
        Ok(())
    })
}

fn retarget_param_name(meta: &mut AnimatorControllerMeta, from: &str, to: &str) {
    if from == to {
        return;
    }
    retarget_machine(&mut meta.states, &mut meta.transitions, from, to);
    for layer in &mut meta.layers {
        retarget_machine(&mut layer.states, &mut layer.transitions, from, to);
    }
}

fn strip_param_references(meta: &mut AnimatorControllerMeta, name: &str) {
    strip_machine(&mut meta.states, &mut meta.transitions, name);
    for layer in &mut meta.layers {
        strip_machine(&mut layer.states, &mut layer.transitions, name);
    }
}

fn strip_machine(
    _states: &mut [ControllerState],
    transitions: &mut [ControllerTransition],
    name: &str,
) {
    for t in transitions.iter_mut() {
        t.conditions.retain(|c| c.param != name);
    }
}

fn retarget_machine(
    states: &mut [ControllerState],
    transitions: &mut [ControllerTransition],
    from: &str,
    to: &str,
) {
    for t in transitions.iter_mut() {
        for c in &mut t.conditions {
            if c.param == from {
                c.param = to.to_string();
            }
        }
    }
    for s in states.iter_mut() {
        if let Some(tree) = s.blend_tree.as_mut() {
            for p in &mut tree.params {
                if p == from {
                    *p = to.to_string();
                }
            }
        }
    }
}

fn edit_controller(
    assets_dir: &Path,
    controller: &str,
    edit: impl FnOnce(&mut AnimatorControllerMeta) -> Result<()>,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    let path = AnimatorControllerMeta::path(assets_dir, controller);
    let mut meta = AnimatorControllerMeta::load(&path)?;
    edit(&mut meta)?;
    meta.save(&path)?;
    Ok((path, meta))
}

fn machine_mut_graph<'a>(
    meta: &'a mut AnimatorControllerMeta,
    layer: Option<&str>,
) -> Result<(
    &'a mut Vec<ControllerState>,
    &'a mut String,
    &'a mut Vec<ControllerTransition>,
    &'a mut ControllerGraph,
)> {
    match layer.map(str::trim).filter(|n| !is_base_layer_name(n)) {
        None => Ok((
            &mut meta.states,
            &mut meta.default_state,
            &mut meta.transitions,
            &mut meta.graph,
        )),
        Some(name) => {
            let Some(found) = meta.layers.iter_mut().find(|l| l.name == name) else {
                bail!("animator controller has no layer '{name}'");
            };
            Ok((
                &mut found.states,
                &mut found.default_state,
                &mut found.transitions,
                &mut found.graph,
            ))
        }
    }
}

fn machine_mut<'a>(
    meta: &'a mut AnimatorControllerMeta,
    layer: Option<&str>,
) -> Result<(&'a mut Vec<ControllerState>, &'a mut String)> {
    let (states, default, _) = machine_mut_full(meta, layer)?;
    Ok((states, default))
}

fn machine_mut_full<'a>(
    meta: &'a mut AnimatorControllerMeta,
    layer: Option<&str>,
) -> Result<(
    &'a mut Vec<ControllerState>,
    &'a mut String,
    &'a mut Vec<ControllerTransition>,
)> {
    match layer.map(str::trim).filter(|n| !is_base_layer_name(n)) {
        None => Ok((
            &mut meta.states,
            &mut meta.default_state,
            &mut meta.transitions,
        )),
        Some(name) => {
            let Some(found) = meta.layers.iter_mut().find(|l| l.name == name) else {
                bail!("animator controller has no layer '{name}'");
            };
            Ok((
                &mut found.states,
                &mut found.default_state,
                &mut found.transitions,
            ))
        }
    }
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
            weight: 1.0,
            layers: Vec::new(),
            graph: ControllerGraph::default(),
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
                position: None,
            }],
            transitions: Vec::new(),
            weight: 1.0,
            layers: Vec::new(),
            graph: ControllerGraph::default(),
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
        m.states[0].clip.clear();
        assert!(m.validate().is_ok(), "empty clip is a placeholder state");
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

    #[test]
    fn layers_roundtrip_and_old_files_omit_them() {
        let legacy = "{\n  \"default\": \"Idle\",\n  \"parameters\": [],\n  \"states\": [\n    {\n      \"name\": \"Idle\",\n      \"clip\": \"idle\",\n      \"speed\": 1.0\n    }\n  ],\n  \"transitions\": []\n}\n";
        let dir = tmp_dir("layers");
        let path = dir.join("p.controller.json");
        fs::write(&path, legacy).unwrap();
        let meta = AnimatorControllerMeta::load(&path).unwrap();
        assert!(meta.layers.is_empty());
        assert!((meta.weight - 1.0).abs() < 1e-6);
        meta.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), legacy);

        let mut authored = sample();
        authored.layers.push(ControllerLayer {
            name: "UpperBody".into(),
            weight: 2.5,
            default_state: "Aim".into(),
            states: vec![
                ControllerState::plain("Aim", "aim"),
                ControllerState::plain("Empty", ""),
            ],
            transitions: vec![],
            graph: ControllerGraph::default(),
        });
        assert!(authored.validate().is_ok());
        let (path, _) = write_animator_controller(&dir, "player", authored).unwrap();
        let loaded = AnimatorControllerMeta::load(&path).unwrap();
        assert_eq!(loaded.layers.len(), 1);
        assert_eq!(loaded.layers[0].weight, 2.5);
        assert!(loaded.layers[0].states[1].clip.is_empty());

        let (_, meta) = add_controller_layer(&dir, "player", "Face", 0.0).unwrap();
        assert_eq!(meta.layers.len(), 2);
        assert!(add_controller_layer(&dir, "player", "Base", 1.0).is_err());
        assert!(add_controller_layer(&dir, "player", "face", 1.0).is_err());
        let (_, meta) = set_controller_layer_weight(&dir, "player", "UpperBody", 0.25).unwrap();
        assert_eq!(meta.layer("UpperBody").unwrap().weight, 0.25);
        let (_, meta) = write_layer_state(&dir, "player", Some("Face"), "Blink", "blink").unwrap();
        assert_eq!(meta.layer("Face").unwrap().default_state, "Blink");
        let (_, meta) = rename_controller_layer(&dir, "player", "Face", "Head").unwrap();
        assert!(meta.layer("Head").is_some());
        let (_, meta) = remove_controller_layer(&dir, "player", "Head").unwrap();
        assert!(meta.layer("Head").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_finite_layer_weights_are_rejected_without_writing() {
        let dir = tmp_dir("nonfinite");
        let path = AnimatorControllerMeta::path(&dir, "player");
        let mut authored = sample();
        authored.layers.push(ControllerLayer {
            name: "UpperBody".into(),
            weight: 1.0,
            default_state: "Idle".into(),
            states: vec![ControllerState::plain("Idle", "idle")],
            transitions: vec![],
            graph: ControllerGraph::default(),
        });
        write_animator_controller(&dir, "player", authored).unwrap();
        let before = fs::read_to_string(&path).unwrap();

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(check_layer_weight(bad).is_err());
            assert!(add_controller_layer(&dir, "player", "Face", bad).is_err());
            assert!(set_controller_layer_weight(&dir, "player", "UpperBody", bad).is_err());
            assert!(set_controller_layer_weight(&dir, "player", "Base", bad).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), before);
        }
        assert!(check_layer_weight(-0.5).is_ok());
        assert!(check_layer_weight(2.5).is_ok());

        let mut poisoned = AnimatorControllerMeta::load(&path).unwrap();
        poisoned.layer_mut("UpperBody").unwrap().weight = f32::NAN;
        assert!(poisoned.save(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert!(set_controller_layer_weight(&dir, "player", "Base", 0.25).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn positions_roundtrip_and_missing_positions_stay_omitted() {
        let legacy = "{\n  \"default\": \"Idle\",\n  \"parameters\": [],\n  \"states\": [\n    {\n      \"name\": \"Idle\",\n      \"clip\": \"idle\",\n      \"speed\": 1.0\n    }\n  ],\n  \"transitions\": []\n}\n";
        let dir = tmp_dir("pos");
        let path = AnimatorControllerMeta::path(&dir, "player");
        fs::write(&path, legacy).unwrap();
        let meta = AnimatorControllerMeta::load(&path).unwrap();
        assert!(meta.states[0].position.is_none());
        assert!(meta.graph.is_empty());
        meta.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), legacy);

        write_animator_controller(&dir, "player", sample()).unwrap();
        let (_, meta) = move_layer_node(&dir, "player", None, "Walk", [180.0, 40.0]).unwrap();
        assert_eq!(meta.state("Walk").unwrap().position, Some([180.0, 40.0]));
        assert!(meta.state("Idle").unwrap().position.is_none());
        let (_, meta) = move_layer_node(&dir, "player", None, "Entry", [12.0, 8.0]).unwrap();
        assert_eq!(meta.graph.entry, Some([12.0, 8.0]));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"position\": ["));
        assert!(text.contains("\"entry\": ["));
        let loaded = AnimatorControllerMeta::load(&path).unwrap();
        assert_eq!(loaded.state("Walk").unwrap().position, Some([180.0, 40.0]));
        loaded.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);

        let before = fs::read_to_string(&path).unwrap();
        assert!(move_layer_node(&dir, "player", None, "Walk", [f32::NAN, 1.0]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_state_updates_transitions_and_default() {
        let dir = tmp_dir("rename-state");
        write_animator_controller(&dir, "player", sample()).unwrap();
        let (_, meta) = rename_layer_state(&dir, "player", None, "Idle", "Stand").unwrap();
        assert!(meta.state("Idle").is_none());
        assert!(meta.state("Stand").is_some());
        assert_eq!(meta.default_state, "Stand");
        assert!(meta
            .transitions
            .iter()
            .any(|t| t.from == "Stand" && t.to == "Walk"));
        assert!(meta
            .transitions
            .iter()
            .any(|t| t.from == "Walk" && t.to == "Stand"));
        assert!(rename_layer_state(&dir, "player", None, "Stand", "Walk").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_state_drops_its_transitions_including_any() {
        let dir = tmp_dir("rm-state");
        let mut meta = sample();
        meta.transitions.push(ControllerTransition {
            from: "Any".into(),
            to: "Walk".into(),
            conditions: vec![],
            has_exit_time: false,
        });
        write_animator_controller(&dir, "player", meta).unwrap();
        let (_, meta) = remove_layer_state(&dir, "player", None, "Walk").unwrap();
        assert!(meta.state("Walk").is_none());
        assert!(meta
            .transitions
            .iter()
            .all(|t| t.from != "Walk" && t.to != "Walk"));
        assert!(meta.transitions.is_empty());
        assert_eq!(meta.default_state, "Idle");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn param_rename_updates_conditions_and_blend_params() {
        let dir = tmp_dir("param");
        let mut meta = sample();
        meta.parameters.push(ControllerParam {
            name: "Speed".into(),
            kind: ControllerParamType::Float,
            default: Some(serde_json::json!(0.0)),
        });
        write_animator_controller(&dir, "player", meta).unwrap();
        write_state_blend_tree(&dir, "player", "Walk", tree1d()).unwrap();
        let (_, meta) = rename_controller_param(&dir, "player", "Moving", "Running").unwrap();
        assert!(meta.parameters.iter().any(|p| p.name == "Running"));
        assert!(meta
            .transitions
            .iter()
            .all(|t| t.conditions.iter().all(|c| c.param == "Running")));
        let (_, meta) = rename_controller_param(&dir, "player", "Speed", "Pace").unwrap();
        let params = &meta
            .state("Walk")
            .unwrap()
            .blend_tree
            .as_ref()
            .unwrap()
            .params;
        assert_eq!(params, &vec!["Pace".to_string()]);
        let (_, meta) = remove_controller_param(&dir, "player", "Running").unwrap();
        assert!(meta.parameters.iter().all(|p| p.name != "Running"));
        assert!(meta
            .transitions
            .iter()
            .all(|t| t.conditions.iter().all(|c| c.param != "Running")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn param_remove_strips_layer_transition_conditions() {
        let dir = tmp_dir("param-rm-layer");
        let mut meta = sample();
        meta.layers.push(ControllerLayer {
            name: "Upper".into(),
            weight: 1.0,
            default_state: "Idle".into(),
            states: meta.states.clone(),
            transitions: vec![ControllerTransition {
                from: "Idle".into(),
                to: "Walk".into(),
                conditions: vec![ControllerCondition {
                    param: "Moving".into(),
                    equals: Some(serde_json::json!(true)),
                    ..Default::default()
                }],
                has_exit_time: false,
            }],
            graph: ControllerGraph::default(),
        });
        write_animator_controller(&dir, "player", meta).unwrap();
        let (_, meta) = remove_controller_param(&dir, "player", "Moving").unwrap();
        assert!(meta.layers[0].transitions[0].conditions.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
