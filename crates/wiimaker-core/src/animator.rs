//! Runtime animator (Unity Animator analogue) — baked controller + live params.

use crate::blend::{choose_active, weights_1d, weights_2d};
use crate::world::{Animation, EntityId, World};

#[cfg(feature = "std")]
mod alloc_types {
    pub use std::string::String;
    pub use std::vec::Vec;
}

#[cfg(not(feature = "std"))]
mod alloc_types {
    extern crate alloc;
    pub use alloc::string::String;
    pub use alloc::vec::Vec;
}

use alloc_types::{String, Vec};

/// Parameter kind copied from the controller asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatorParamKind {
    Bool,
    Float,
    Trigger,
}

/// Live parameter value.
#[derive(Clone, Debug)]
pub struct AnimatorParam {
    pub name: String,
    pub kind: AnimatorParamKind,
    pub bool_value: bool,
    pub float_value: f32,
}

impl AnimatorParam {
    pub fn bool_param(name: impl Into<String>, value: bool) -> Self {
        Self {
            name: name.into(),
            kind: AnimatorParamKind::Bool,
            bool_value: value,
            float_value: 0.0,
        }
    }

    pub fn float_param(name: impl Into<String>, value: f32) -> Self {
        Self {
            name: name.into(),
            kind: AnimatorParamKind::Float,
            bool_value: false,
            float_value: value,
        }
    }

    pub fn trigger_param(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: AnimatorParamKind::Trigger,
            bool_value: false,
            float_value: 0.0,
        }
    }
}

/// Blend dimension: thresholds on one Float param, or positions on two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendDimension {
    OneD,
    TwoD,
}

impl BlendDimension {
    pub fn param_count(self) -> usize {
        match self {
            Self::OneD => 1,
            Self::TwoD => 2,
        }
    }
}

/// One child motion of a blend tree; cells are resolved at hydrate like a state clip.
#[derive(Clone, Debug)]
pub struct BlendMotion {
    pub clip: String,
    pub cells: Vec<String>,
    pub fps: f32,
    pub loop_: bool,
    pub threshold: f32,
    pub position: [f32; 2],
}

impl BlendMotion {
    pub fn duration(&self) -> f32 {
        if self.cells.is_empty() || self.fps <= 0.0 {
            return 0.0;
        }
        self.cells.len() as f32 / self.fps
    }
}

/// 1D or 2D blend tree on a state. `weights` are the last evaluation (same order as `motions`).
#[derive(Clone, Debug)]
pub struct BlendTree {
    pub dimension: BlendDimension,
    pub params: Vec<String>,
    pub motions: Vec<BlendMotion>,
    pub weights: Vec<f32>,
    pub active: usize,
}

impl BlendTree {
    pub fn new(dimension: BlendDimension, params: Vec<String>, motions: Vec<BlendMotion>) -> Self {
        let mut weights = Vec::new();
        weights.resize(motions.len(), 0.0);
        Self {
            dimension,
            params,
            motions,
            weights,
            active: 0,
        }
    }

    pub fn active_motion(&self) -> Option<&BlendMotion> {
        self.motions.get(self.active)
    }

    fn evaluate(&mut self, inputs: [f32; 2]) {
        self.weights = match self.dimension {
            BlendDimension::OneD => {
                let thresholds: Vec<f32> = self.motions.iter().map(|m| m.threshold).collect();
                weights_1d(&thresholds, inputs[0])
            }
            BlendDimension::TwoD => {
                let positions: Vec<[f32; 2]> = self.motions.iter().map(|m| m.position).collect();
                weights_2d(&positions, inputs)
            }
        };
        self.active = choose_active(&self.weights, self.active);
    }
}

/// Resolved state (clip cells already looked up at hydrate).
#[derive(Clone, Debug)]
pub struct AnimatorState {
    pub name: String,
    pub clip: String,
    pub speed: f32,
    pub cells: Vec<String>,
    pub fps: f32,
    pub loop_: bool,
    /// When set, the state plays the dominant motion instead of `clip`/`cells`.
    pub blend: Option<BlendTree>,
}

impl AnimatorState {
    pub fn duration(&self) -> f32 {
        if let Some(b) = &self.blend {
            return b.active_motion().map(|m| m.duration()).unwrap_or(0.0);
        }
        if self.cells.is_empty() || self.fps <= 0.0 {
            return 0.0;
        }
        self.cells.len() as f32 / self.fps
    }

    pub fn playback_fps(&self) -> f32 {
        (self.fps * self.speed.max(0.001)).max(0.001)
    }
}

/// Clip cells + playback fps/loop for the sibling `Animation` (already scaled by state speed).
#[derive(Clone, Debug)]
struct Playback {
    clip: String,
    cells: Vec<String>,
    fps: f32,
    loop_: bool,
}

/// AND-ed transition condition.
#[derive(Clone, Debug)]
pub enum AnimatorCondition {
    BoolEq { param: String, value: bool },
    FloatEq { param: String, value: f32 },
    FloatGreater { param: String, value: f32 },
    FloatLess { param: String, value: f32 },
    Trigger { param: String },
}

/// Directed edge. `from_any` matches every current state.
#[derive(Clone, Debug)]
pub struct AnimatorTransition {
    pub from: String,
    pub from_any: bool,
    pub to: String,
    pub conditions: Vec<AnimatorCondition>,
    pub has_exit_time: bool,
}

/// One override layer. The base machine stays on [`Animator`]'s own fields (index 0).
///
/// Sprites cannot cross-fade or bone-blend, so a layer is Override only: when its
/// clamped weight is > 0 and its current state has a clip or blend tree, it can
/// replace the base clip. Higher indices win.
#[derive(Clone, Debug)]
pub struct AnimatorLayer {
    pub name: String,
    pub weight: f32,
    pub state: String,
    pub state_time: f32,
    pub states: Vec<AnimatorState>,
    pub transitions: Vec<AnimatorTransition>,
}

/// Display name of layer 0. Not stored as an [`AnimatorLayer`].
pub const BASE_LAYER_NAME: &str = "Base";

/// NaN → 0. Values outside 0..1 clamp.
pub fn clamp_layer_weight(weight: f32) -> f32 {
    if !weight.is_finite() {
        0.0
    } else {
        weight.clamp(0.0, 1.0)
    }
}

/// Runtime animator: owns state machine, feeds sibling [`Animation`].
///
/// `state` / `states` / `transitions` are the base layer. `layers` are override
/// machines (index 1+). Parameters are shared. Triggers are consumed once per
/// tick after every layer has been evaluated.
#[derive(Clone, Debug)]
pub struct Animator {
    pub controller: String,
    pub state: String,
    pub state_time: f32,
    pub parameters: Vec<AnimatorParam>,
    pub states: Vec<AnimatorState>,
    pub transitions: Vec<AnimatorTransition>,
    /// Base layer weight. The base clip still plays when no override contributes.
    pub base_weight: f32,
    pub layers: Vec<AnimatorLayer>,
    /// Layer index that last wrote the sibling Animation.
    playback_layer: Option<usize>,
}

impl Animator {
    pub fn new(controller: impl Into<String>) -> Self {
        Self {
            controller: controller.into(),
            state: String::new(),
            state_time: 0.0,
            parameters: Vec::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            base_weight: 1.0,
            layers: Vec::new(),
            playback_layer: None,
        }
    }

    pub fn layer_count(&self) -> usize {
        1 + self.layers.len()
    }

    pub fn playback_layer(&self) -> Option<usize> {
        self.playback_layer
    }

    pub fn layer_name(&self, index: usize) -> Option<&str> {
        if index == 0 {
            Some(BASE_LAYER_NAME)
        } else {
            self.layers.get(index - 1).map(|l| l.name.as_str())
        }
    }

    pub fn layer_weight_raw(&self, index: usize) -> Option<f32> {
        if index == 0 {
            Some(self.base_weight)
        } else {
            self.layers.get(index - 1).map(|l| l.weight)
        }
    }

    /// Clamped weight (NaN and out-of-range become 0..1).
    pub fn layer_weight(&self, index: usize) -> Option<f32> {
        self.layer_weight_raw(index).map(clamp_layer_weight)
    }

    pub fn layer_state_name(&self, index: usize) -> Option<&str> {
        if index == 0 {
            Some(self.state.as_str())
        } else {
            self.layers.get(index - 1).map(|l| l.state.as_str())
        }
    }

    pub fn layer_blend(&self, index: usize) -> Option<&BlendTree> {
        let (name, states) = if index == 0 {
            (self.state.as_str(), self.states.as_slice())
        } else {
            let layer = self.layers.get(index - 1)?;
            (layer.state.as_str(), layer.states.as_slice())
        };
        states
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.blend.as_ref())
    }

    /// Set a layer weight by name (`Base` or an override name). Stores the raw value.
    pub fn set_layer_weight(&mut self, name: &str, weight: f32) -> bool {
        if name.eq_ignore_ascii_case(BASE_LAYER_NAME) {
            self.base_weight = weight;
            return true;
        }
        if let Some(layer) = self.layers.iter_mut().find(|l| l.name == name) {
            layer.weight = weight;
            return true;
        }
        false
    }

    /// Highest override index that contributes, else 0 (the base layer).
    pub fn winning_layer(&self) -> usize {
        for i in (0..self.layers.len()).rev() {
            if layer_contributes(
                self.layers[i].weight,
                &self.layers[i].state,
                &self.layers[i].states,
            ) {
                return i + 1;
            }
        }
        0
    }

    pub fn find_state(&self, name: &str) -> Option<&AnimatorState> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn current_state(&self) -> Option<&AnimatorState> {
        self.find_state(&self.state)
    }

    pub fn param(&self, name: &str) -> Option<&AnimatorParam> {
        self.parameters.iter().find(|p| p.name == name)
    }

    pub fn param_mut(&mut self, name: &str) -> Option<&mut AnimatorParam> {
        self.parameters.iter_mut().find(|p| p.name == name)
    }

    pub fn bool_value(&self, name: &str) -> Option<bool> {
        self.param(name).map(|p| match p.kind {
            AnimatorParamKind::Float => p.float_value != 0.0,
            AnimatorParamKind::Bool | AnimatorParamKind::Trigger => p.bool_value,
        })
    }

    pub fn set_bool(&mut self, name: &str, value: bool) -> bool {
        if let Some(p) = self.param_mut(name) {
            match p.kind {
                AnimatorParamKind::Bool => {
                    p.bool_value = value;
                    return true;
                }
                AnimatorParamKind::Trigger => {
                    p.bool_value = value;
                    return true;
                }
                AnimatorParamKind::Float => {
                    p.float_value = if value { 1.0 } else { 0.0 };
                    return true;
                }
            }
        }
        false
    }

    pub fn set_float(&mut self, name: &str, value: f32) -> bool {
        if let Some(p) = self.param_mut(name) {
            if p.kind == AnimatorParamKind::Float {
                p.float_value = value;
                return true;
            }
        }
        false
    }

    pub fn set_trigger(&mut self, name: &str) -> bool {
        if let Some(p) = self.param_mut(name) {
            if p.kind == AnimatorParamKind::Trigger {
                p.bool_value = true;
                return true;
            }
        }
        false
    }

    fn evaluate_all_blends(&mut self) {
        let params = self.parameters.clone();
        evaluate_machine(&params, &self.state, &mut self.states);
        for layer in &mut self.layers {
            evaluate_machine(&params, &layer.state, &mut layer.states);
        }
    }

    /// Clip the current base state should drive on `Animation` (dominant blend motion when blended).
    fn playback(&self) -> Option<Playback> {
        let s = self.current_state()?;
        state_playback(s)
    }

    fn layer_playback(&self, index: usize) -> Option<Playback> {
        if index == 0 {
            return self.playback();
        }
        let layer = self.layers.get(index - 1)?;
        let s = layer.states.iter().find(|s| s.name == layer.state)?;
        state_playback(s)
    }

    /// Blend tree on the current state, with weights from the last evaluation.
    pub fn current_blend(&self) -> Option<&BlendTree> {
        self.current_state().and_then(|s| s.blend.as_ref())
    }

    /// First matching transition on the base layer, or `None` to stay.
    pub fn pick_transition(&self) -> Option<&AnimatorTransition> {
        let idx = pick_index(
            &self.parameters,
            &self.state,
            self.state_time,
            &self.states,
            &self.transitions,
        )?;
        self.transitions.get(idx)
    }

    fn pick_owned(&self, index: usize) -> Option<(String, Vec<AnimatorCondition>)> {
        if index == 0 {
            return self
                .pick_transition()
                .map(|t| (t.to.clone(), t.conditions.clone()));
        }
        let layer = self.layers.get(index - 1)?;
        let idx = pick_index(
            &self.parameters,
            &layer.state,
            layer.state_time,
            &layer.states,
            &layer.transitions,
        )?;
        let t = layer.transitions.get(idx)?;
        Some((t.to.clone(), t.conditions.clone()))
    }

    fn advance_times(&mut self, dt: f32) {
        let speed = self
            .current_state()
            .map(|s| s.speed.max(0.001))
            .unwrap_or(1.0);
        self.state_time += dt * speed;
        for layer in &mut self.layers {
            let speed = layer
                .states
                .iter()
                .find(|s| s.name == layer.state)
                .map(|s| s.speed.max(0.001))
                .unwrap_or(1.0);
            layer.state_time += dt * speed;
        }
    }
}

fn state_playback(s: &AnimatorState) -> Option<Playback> {
    if !state_has_motion(s) {
        return None;
    }
    let speed = s.speed.max(0.001);
    match &s.blend {
        Some(b) => {
            let m = b.active_motion()?;
            if m.clip.is_empty() && m.cells.is_empty() {
                return None;
            }
            Some(Playback {
                clip: m.clip.clone(),
                cells: m.cells.clone(),
                fps: (m.fps * speed).max(0.001),
                loop_: m.loop_,
            })
        }
        None => Some(Playback {
            clip: s.clip.clone(),
            cells: s.cells.clone(),
            fps: s.playback_fps(),
            loop_: s.loop_,
        }),
    }
}

fn state_has_motion(s: &AnimatorState) -> bool {
    match &s.blend {
        Some(b) => b
            .motions
            .iter()
            .any(|m| !m.clip.is_empty() || !m.cells.is_empty()),
        None => !s.clip.is_empty() || !s.cells.is_empty(),
    }
}

fn layer_contributes(weight: f32, state: &str, states: &[AnimatorState]) -> bool {
    if clamp_layer_weight(weight) <= 0.0 {
        return false;
    }
    states
        .iter()
        .find(|s| s.name == state)
        .is_some_and(state_has_motion)
}

fn param_as_float(params: &[AnimatorParam], name: &str) -> f32 {
    match params.iter().find(|p| p.name == name) {
        Some(p) => match p.kind {
            AnimatorParamKind::Float => p.float_value,
            AnimatorParamKind::Bool | AnimatorParamKind::Trigger => {
                if p.bool_value {
                    1.0
                } else {
                    0.0
                }
            }
        },
        None => 0.0,
    }
}

fn evaluate_machine(params: &[AnimatorParam], state: &str, states: &mut [AnimatorState]) -> bool {
    let Some(idx) = states.iter().position(|s| s.name == state) else {
        return false;
    };
    let inputs = match states[idx].blend.as_ref() {
        None => return false,
        Some(b) => {
            let mut v = [0.0_f32; 2];
            for (i, name) in b.params.iter().take(2).enumerate() {
                v[i] = param_as_float(params, name);
            }
            v
        }
    };
    if let Some(b) = states[idx].blend.as_mut() {
        b.evaluate(inputs);
    }
    true
}

fn condition_met(params: &[AnimatorParam], c: &AnimatorCondition) -> bool {
    match c {
        AnimatorCondition::BoolEq { param, value } => params
            .iter()
            .find(|p| p.name == *param)
            .is_some_and(|p| match p.kind {
                AnimatorParamKind::Float => (p.float_value != 0.0) == *value,
                AnimatorParamKind::Bool | AnimatorParamKind::Trigger => p.bool_value == *value,
            }),
        AnimatorCondition::FloatEq { param, value } => params
            .iter()
            .find(|p| p.name == *param)
            .is_some_and(|p| (p.float_value - *value).abs() < 1e-4),
        AnimatorCondition::FloatGreater { param, value } => params
            .iter()
            .find(|p| p.name == *param)
            .is_some_and(|p| p.float_value > *value),
        AnimatorCondition::FloatLess { param, value } => params
            .iter()
            .find(|p| p.name == *param)
            .is_some_and(|p| p.float_value < *value),
        AnimatorCondition::Trigger { param } => params
            .iter()
            .find(|p| p.name == *param)
            .is_some_and(|p| p.kind == AnimatorParamKind::Trigger && p.bool_value),
    }
}

fn pick_index(
    params: &[AnimatorParam],
    current: &str,
    state_time: f32,
    states: &[AnimatorState],
    transitions: &[AnimatorTransition],
) -> Option<usize> {
    let duration = states
        .iter()
        .find(|s| s.name == current)
        .map(|s| s.duration())
        .unwrap_or(0.0);
    for (i, t) in transitions.iter().enumerate() {
        let from_ok = t.from_any || t.from == current;
        if !from_ok {
            continue;
        }
        if t.to == current && t.from_any {
            continue;
        }
        if t.has_exit_time && duration > 0.0 && state_time < duration {
            continue;
        }
        if t.conditions.iter().all(|c| condition_met(params, c)) {
            return Some(i);
        }
    }
    None
}

fn trigger_param_names(params: &[AnimatorParam], conditions: &[AnimatorCondition]) -> Vec<String> {
    let mut names = Vec::new();
    for c in conditions {
        let hit = match c {
            AnimatorCondition::Trigger { param } => Some(param.as_str()),
            AnimatorCondition::BoolEq { param, value: true } => {
                let is_trigger = params
                    .iter()
                    .any(|p| p.name == *param && p.kind == AnimatorParamKind::Trigger);
                is_trigger.then_some(param.as_str())
            }
            _ => None,
        };
        if let Some(name) = hit {
            if !names.iter().any(|n| n == name) {
                names.push(String::from(name));
            }
        }
    }
    names
}

impl World {
    pub fn animator(&self, id: EntityId) -> Option<&Animator> {
        self.slot(id).and_then(|s| s.animator.as_ref())
    }

    pub fn animator_mut(&mut self, id: EntityId) -> Option<&mut Animator> {
        self.slot_mut(id).and_then(|s| s.animator.as_mut())
    }

    pub fn set_animator(&mut self, id: EntityId, animator: Option<Animator>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.animator = animator;
        }
    }

    /// Set a Bool (or Trigger) parameter. Returns false if missing.
    pub fn set_animator_bool(&mut self, id: EntityId, name: &str, value: bool) -> bool {
        self.animator_mut(id)
            .map(|a| a.set_bool(name, value))
            .unwrap_or(false)
    }

    pub fn set_animator_float(&mut self, id: EntityId, name: &str, value: f32) -> bool {
        self.animator_mut(id)
            .map(|a| a.set_float(name, value))
            .unwrap_or(false)
    }

    pub fn set_animator_trigger(&mut self, id: EntityId, name: &str) -> bool {
        self.animator_mut(id)
            .map(|a| a.set_trigger(name))
            .unwrap_or(false)
    }

    pub fn set_animator_layer_weight(&mut self, id: EntityId, name: &str, weight: f32) -> bool {
        let ok = self
            .animator_mut(id)
            .map(|a| a.set_layer_weight(name, weight))
            .unwrap_or(false);
        if ok {
            self.drive_animator(id, None);
        }
        ok
    }

    /// Apply a base-layer state, then drive Animation from the winning layer.
    /// Blend states evaluate live params. A state change on the winner starts at frame 0.
    pub fn apply_animator_state(&mut self, id: EntityId, state_name: &str) {
        let Some(a) = self.animator_mut(id) else {
            return;
        };
        if a.find_state(state_name).is_none() {
            return;
        }
        a.state = state_name.into();
        a.state_time = 0.0;
        self.drive_animator(id, Some(0));
    }

    /// Re-evaluate every layer and write the winning clip.
    ///
    /// `reset_layer` forces frame 0 when that layer is the winner (a transition).
    /// A blend-motion change on the same winner keeps normalized phase.
    /// Switching which layer wins also starts at frame 0.
    fn drive_animator(&mut self, id: EntityId, reset_layer: Option<usize>) {
        let Some(playback) = self.animator_mut(id).and_then(|a| {
            a.evaluate_all_blends();
            let win = a.winning_layer();
            let playback = a.layer_playback(win)?;
            let reset = a.playback_layer != Some(win) || reset_layer == Some(win);
            a.playback_layer = Some(win);
            Some((playback, reset))
        }) else {
            return;
        };
        let (playback, reset) = playback;
        self.write_playback(id, playback, reset);
    }

    fn write_playback(&mut self, id: EntityId, playback: Playback, reset: bool) {
        let Playback {
            clip,
            cells,
            fps,
            loop_,
        } = playback;
        match self.animation_mut(id) {
            Some(anim) => {
                if reset {
                    anim.clip = clip;
                    anim.cells = cells;
                    anim.fps = fps;
                    anim.loop_ = loop_;
                    anim.time = 0.0;
                    anim.frame = 0;
                    return;
                }
                if anim.clip == clip {
                    anim.cells = cells;
                    anim.fps = fps;
                    anim.loop_ = loop_;
                    return;
                }
                let old_dur = if anim.cells.is_empty() || anim.fps <= 0.0 {
                    0.0
                } else {
                    anim.cells.len() as f32 / anim.fps
                };
                let phase = if old_dur <= 0.0 {
                    0.0
                } else if anim.loop_ {
                    (anim.time / old_dur) % 1.0
                } else {
                    (anim.time / old_dur).clamp(0.0, 1.0)
                };
                let new_dur = if cells.is_empty() || fps <= 0.0 {
                    0.0
                } else {
                    cells.len() as f32 / fps
                };
                anim.clip = clip;
                anim.cells = cells;
                anim.fps = fps;
                anim.loop_ = loop_;
                anim.time = phase * new_dur;
                anim.frame = 0;
            }
            None => {
                self.set_animation(id, Some(Animation::new(clip, cells, fps, loop_)));
            }
        }
    }

    /// Advance every layer, then take transitions against one parameter snapshot.
    ///
    /// Triggers referenced by any taken transition are cleared once after all
    /// layers have chosen, so two layers can both see the same trigger in a tick.
    /// A trigger nobody took stays set. Call before clip frame advance.
    pub fn tick_animators(&mut self, dt: f32) {
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
            if let Some(a) = self.animator_mut(id) {
                a.advance_times(dt);
            }
            let Some(a) = self.animator(id) else {
                continue;
            };
            let n = a.layer_count();
            let mut taken: Vec<(usize, String, Vec<AnimatorCondition>)> = Vec::new();
            for index in 0..n {
                if let Some((to, conditions)) = a.pick_owned(index) {
                    taken.push((index, to, conditions));
                }
            }
            let mut consume: Vec<String> = Vec::new();
            if let Some(a) = self.animator(id) {
                for (_, _, conditions) in &taken {
                    for name in trigger_param_names(&a.parameters, conditions) {
                        if !consume.iter().any(|n| n == &name) {
                            consume.push(name);
                        }
                    }
                }
            }
            if let Some(a) = self.animator_mut(id) {
                for name in &consume {
                    if let Some(p) = a.param_mut(name) {
                        if p.kind == AnimatorParamKind::Trigger {
                            p.bool_value = false;
                        }
                    }
                }
                for (index, to, _) in &taken {
                    if *index == 0 {
                        if a.find_state(to).is_some() {
                            a.state = to.clone();
                            a.state_time = 0.0;
                        }
                    } else if let Some(layer) = a.layers.get_mut(index - 1) {
                        if layer.states.iter().any(|s| s.name == *to) {
                            layer.state = to.clone();
                            layer.state_time = 0.0;
                        }
                    }
                }
            }
            let reset = taken.iter().map(|(index, _, _)| *index).collect::<Vec<_>>();
            // Drive once. If several layers transitioned, reset when the winner is one of them.
            let winner_reset = self
                .animator(id)
                .map(|a| a.winning_layer())
                .filter(|win| reset.contains(win));
            self.drive_animator(id, winner_reset);
        }
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::Transform;

    fn idle_walk() -> Animator {
        let mut a = Animator::new("player");
        a.state = "Idle".into();
        a.parameters
            .push(AnimatorParam::bool_param("Moving", false));
        a.states.push(AnimatorState {
            name: "Idle".into(),
            clip: "idle".into(),
            speed: 1.0,
            cells: vec!["i0".into(), "i1".into()],
            fps: 10.0,
            loop_: true,
            blend: None,
        });
        a.states.push(AnimatorState {
            name: "Walk".into(),
            clip: "walk".into(),
            speed: 1.0,
            cells: vec!["w0".into(), "w1".into()],
            fps: 10.0,
            loop_: true,
            blend: None,
        });
        a.transitions.push(AnimatorTransition {
            from: "Idle".into(),
            from_any: false,
            to: "Walk".into(),
            conditions: vec![AnimatorCondition::BoolEq {
                param: "Moving".into(),
                value: true,
            }],
            has_exit_time: false,
        });
        a.transitions.push(AnimatorTransition {
            from: "Walk".into(),
            from_any: false,
            to: "Idle".into(),
            conditions: vec![AnimatorCondition::BoolEq {
                param: "Moving".into(),
                value: false,
            }],
            has_exit_time: false,
        });
        a
    }

    #[test]
    fn idle_walk_on_bool() {
        let mut world = World::new();
        let id = world.spawn_named("p", Transform::from_xy(0.0, 0.0));
        world.set_animator(id, Some(idle_walk()));
        world.apply_animator_state(id, "Idle");
        assert_eq!(world.animation(id).unwrap().clip, "idle");
        assert_eq!(world.animation(id).unwrap().cells[0], "i0");
        world.tick_animators(0.1);
        assert_eq!(world.animator(id).unwrap().state, "Idle");
        assert!(world.set_animator_bool(id, "Moving", true));
        world.tick_animators(0.0);
        assert_eq!(world.animator(id).unwrap().state, "Walk");
        assert_eq!(world.animation(id).unwrap().clip, "walk");
        assert_eq!(world.animation(id).unwrap().cells[0], "w0");
        assert_eq!(world.animation(id).unwrap().frame, 0);
        world.set_animator_bool(id, "Moving", false);
        world.tick_animators(0.0);
        assert_eq!(world.animator(id).unwrap().state, "Idle");
        assert_eq!(world.animation(id).unwrap().clip, "idle");
    }

    fn blend_motion(clip: &str, threshold: f32, position: [f32; 2], cells: &[&str]) -> BlendMotion {
        BlendMotion {
            clip: clip.into(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
            fps: 10.0,
            loop_: true,
            threshold,
            position,
        }
    }

    fn locomotion(
        dimension: BlendDimension,
        params: &[&str],
        motions: Vec<BlendMotion>,
    ) -> Animator {
        let mut a = Animator::new("ctrl");
        for p in params {
            a.parameters.push(AnimatorParam::float_param(*p, 0.0));
        }
        a.states.push(AnimatorState {
            name: "Locomotion".into(),
            clip: String::new(),
            speed: 1.0,
            cells: Vec::new(),
            fps: 10.0,
            loop_: true,
            blend: Some(BlendTree::new(
                dimension,
                params.iter().map(|p| p.to_string()).collect(),
                motions,
            )),
        });
        a.state = "Locomotion".into();
        a
    }

    fn speed_1d() -> Animator {
        locomotion(
            BlendDimension::OneD,
            &["Speed"],
            vec![
                blend_motion("idle", 0.0, [0.0, 0.0], &["i0", "i1"]),
                blend_motion("walk", 1.0, [0.0, 0.0], &["w0", "w1"]),
            ],
        )
    }

    #[test]
    fn blend_1d_switches_clip_from_float_param() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(speed_1d()));
        world.apply_animator_state(id, "Locomotion");
        assert_eq!(world.animation(id).unwrap().clip, "idle");
        assert!(world.set_animator_float(id, "Speed", 0.9));
        world.tick_animators(0.0);
        assert_eq!(world.animation(id).unwrap().clip, "walk");
        assert_eq!(world.animation(id).unwrap().cells[0], "w0");
        let w = world
            .animator(id)
            .unwrap()
            .current_blend()
            .unwrap()
            .weights
            .clone();
        assert!((w[0] + w[1] - 1.0).abs() < 1e-5);
        assert!((w[1] - 0.9).abs() < 1e-5);
        world.set_animator_float(id, "Speed", 0.1);
        world.tick_animators(0.0);
        assert_eq!(world.animation(id).unwrap().clip, "idle");
    }

    #[test]
    fn blend_1d_clamps_param_outside_range() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(speed_1d()));
        world.apply_animator_state(id, "Locomotion");
        world.set_animator_float(id, "Speed", 99.0);
        world.tick_animators(0.0);
        let w = world
            .animator(id)
            .unwrap()
            .current_blend()
            .unwrap()
            .weights
            .clone();
        assert_eq!(w, vec![0.0, 1.0]);
        world.set_animator_float(id, "Speed", -99.0);
        world.tick_animators(0.0);
        let w = world
            .animator(id)
            .unwrap()
            .current_blend()
            .unwrap()
            .weights
            .clone();
        assert_eq!(w, vec![1.0, 0.0]);
    }

    #[test]
    fn blend_missing_param_reads_as_zero() {
        let mut a = speed_1d();
        a.parameters.clear();
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Locomotion");
        world.tick_animators(0.0);
        assert_eq!(world.animation(id).unwrap().clip, "idle");
    }

    #[test]
    fn blend_single_motion_plays_that_clip() {
        let a = locomotion(
            BlendDimension::OneD,
            &["Speed"],
            vec![blend_motion("walk", 0.0, [0.0, 0.0], &["w0"])],
        );
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Locomotion");
        world.set_animator_float(id, "Speed", 5.0);
        world.tick_animators(0.0);
        assert_eq!(world.animation(id).unwrap().clip, "walk");
        assert_eq!(
            world.animator(id).unwrap().current_blend().unwrap().weights,
            vec![1.0]
        );
    }

    #[test]
    fn blend_switch_keeps_normalized_phase() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(speed_1d()));
        world.apply_animator_state(id, "Locomotion");
        world.animation_mut(id).unwrap().time = 0.1;
        world.set_animator_float(id, "Speed", 1.0);
        world.tick_animators(0.0);
        let anim = world.animation(id).unwrap();
        assert_eq!(anim.clip, "walk");
        assert!((anim.time - 0.1).abs() < 1e-5, "time {}", anim.time);
    }

    #[test]
    fn blend_2d_picks_dominant_direction() {
        let mut a = locomotion(
            BlendDimension::TwoD,
            &["DirX", "DirY"],
            vec![
                blend_motion("idle", 0.0, [0.0, 0.0], &["i"]),
                blend_motion("up", 0.0, [0.0, -1.0], &["u"]),
                blend_motion("right", 0.0, [1.0, 0.0], &["r"]),
                blend_motion("down", 0.0, [0.0, 1.0], &["d"]),
                blend_motion("left", 0.0, [-1.0, 0.0], &["l"]),
            ],
        );
        a.set_float("DirX", 1.0);
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Locomotion");
        assert_eq!(world.animation(id).unwrap().clip, "right");
        world.set_animator_float(id, "DirX", 0.0);
        world.set_animator_float(id, "DirY", 1.0);
        world.tick_animators(0.0);
        assert_eq!(world.animation(id).unwrap().clip, "down");
        let w = world
            .animator(id)
            .unwrap()
            .current_blend()
            .unwrap()
            .weights
            .clone();
        assert_eq!(w.len(), 5);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(w.iter().all(|v| *v >= 0.0));
    }

    #[test]
    fn blend_state_exit_time_uses_active_motion_duration() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        let mut a = speed_1d();
        a.states.push(AnimatorState {
            name: "Idle".into(),
            clip: "idle".into(),
            speed: 1.0,
            cells: vec!["i0".into(), "i1".into()],
            fps: 10.0,
            loop_: true,
            blend: None,
        });
        a.transitions.push(AnimatorTransition {
            from: "Locomotion".into(),
            from_any: false,
            to: "Idle".into(),
            conditions: vec![AnimatorCondition::FloatLess {
                param: "Speed".into(),
                value: 0.5,
            }],
            has_exit_time: true,
        });
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Locomotion");
        world.set_animator_float(id, "Speed", 0.0);
        world.tick_animators(0.05);
        assert_eq!(world.animator(id).unwrap().state, "Locomotion");
        world.tick_animators(0.2);
        assert_eq!(world.animator(id).unwrap().state, "Idle");
    }

    #[test]
    fn exit_time_blocks_until_cycle() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        let mut a = idle_walk();
        a.transitions[0].has_exit_time = true;
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Idle");
        world.set_animator_bool(id, "Moving", true);
        world.tick_animators(0.05);
        assert_eq!(world.animator(id).unwrap().state, "Idle");
        world.tick_animators(0.16);
        assert_eq!(world.animator(id).unwrap().state, "Walk");
    }

    fn clip_state(name: &str, clip: &str) -> AnimatorState {
        AnimatorState {
            name: name.into(),
            clip: clip.into(),
            speed: 1.0,
            cells: vec![format!("{clip}_0")],
            fps: 8.0,
            loop_: true,
            blend: None,
        }
    }

    fn walk_aim() -> Animator {
        let mut a = Animator::new("player");
        a.state = "Walk".into();
        a.states.push(clip_state("Walk", "walk"));
        a.states.push(clip_state("Idle", "idle"));
        a.parameters.push(AnimatorParam::trigger_param("Fire"));
        a.parameters.push(AnimatorParam::bool_param("Moving", true));
        a.transitions.push(AnimatorTransition {
            from: "Walk".into(),
            from_any: false,
            to: "Idle".into(),
            conditions: vec![AnimatorCondition::Trigger {
                param: "Fire".into(),
            }],
            has_exit_time: false,
        });
        a.layers.push(AnimatorLayer {
            name: "UpperBody".into(),
            weight: 1.0,
            state: "Aim".into(),
            state_time: 0.0,
            states: vec![clip_state("Aim", "aim"), clip_state("Shoot", "shoot")],
            transitions: vec![AnimatorTransition {
                from: "Aim".into(),
                from_any: false,
                to: "Shoot".into(),
                conditions: vec![AnimatorCondition::Trigger {
                    param: "Fire".into(),
                }],
                has_exit_time: false,
            }],
        });
        a.layers.push(AnimatorLayer {
            name: "Face".into(),
            weight: 0.0,
            state: "Blink".into(),
            state_time: 0.0,
            states: vec![clip_state("Blink", "blink")],
            transitions: Vec::new(),
        });
        a
    }

    #[test]
    fn override_layer_wins_until_weight_is_zero() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(walk_aim()));
        world.apply_animator_state(id, "Walk");
        assert_eq!(world.animation(id).unwrap().clip, "aim");
        assert_eq!(world.animator(id).unwrap().state, "Walk");
        assert_eq!(world.animator(id).unwrap().winning_layer(), 1);
        assert_eq!(world.animator(id).unwrap().layer_state_name(1), Some("Aim"));

        assert!(world.set_animator_layer_weight(id, "UpperBody", 0.0));
        assert_eq!(world.animation(id).unwrap().clip, "walk");
        assert_eq!(world.animator(id).unwrap().winning_layer(), 0);

        assert!(world.set_animator_layer_weight(id, "UpperBody", 2.0));
        assert_eq!(world.animator(id).unwrap().layer_weight(1), Some(1.0));
        assert_eq!(world.animation(id).unwrap().clip, "aim");

        world.set_animator_layer_weight(id, "UpperBody", f32::NAN);
        assert_eq!(world.animator(id).unwrap().layer_weight(1), Some(0.0));
        assert_eq!(world.animation(id).unwrap().clip, "walk");
    }

    #[test]
    fn higher_index_override_beats_lower_when_both_contribute() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(walk_aim()));
        world.apply_animator_state(id, "Walk");
        world.set_animator_layer_weight(id, "Face", 1.0);
        assert_eq!(world.animation(id).unwrap().clip, "blink");
        assert_eq!(world.animator(id).unwrap().winning_layer(), 2);
        world.set_animator_layer_weight(id, "Face", 0.0);
        assert_eq!(world.animation(id).unwrap().clip, "aim");
    }

    #[test]
    fn empty_placeholder_does_not_override() {
        let mut a = walk_aim();
        a.layers[0].states[0].clip.clear();
        a.layers[0].states[0].cells.clear();
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Walk");
        assert_eq!(world.animation(id).unwrap().clip, "walk");
    }

    #[test]
    fn shared_trigger_fires_every_layer_then_consumes_once() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(walk_aim()));
        world.apply_animator_state(id, "Walk");
        assert!(world.set_animator_trigger(id, "Fire"));
        world.tick_animators(0.0);
        let a = world.animator(id).unwrap();
        assert_eq!(a.state, "Idle");
        assert_eq!(a.layer_state_name(1), Some("Shoot"));
        assert_eq!(a.bool_value("Fire"), Some(false));
        assert_eq!(world.animation(id).unwrap().clip, "shoot");

        world.set_animator_trigger(id, "Fire");
        world.tick_animators(0.0);
        assert_eq!(world.animator(id).unwrap().bool_value("Fire"), Some(true));
        assert_eq!(world.animator(id).unwrap().state, "Idle");
    }

    #[test]
    fn weight_zero_layer_still_ticks_its_state_machine() {
        let mut a = walk_aim();
        a.layers[0].weight = 0.0;
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animator(id, Some(a));
        world.apply_animator_state(id, "Walk");
        assert_eq!(world.animation(id).unwrap().clip, "walk");
        world.set_animator_trigger(id, "Fire");
        world.tick_animators(0.0);
        assert_eq!(
            world.animator(id).unwrap().layer_state_name(1),
            Some("Shoot")
        );
        assert_eq!(world.animation(id).unwrap().clip, "idle");
        world.set_animator_layer_weight(id, "UpperBody", 1.0);
        assert_eq!(world.animation(id).unwrap().clip, "shoot");
    }
}
