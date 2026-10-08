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

/// Runtime animator: owns state machine, feeds sibling [`Animation`].
#[derive(Clone, Debug)]
pub struct Animator {
    pub controller: String,
    pub state: String,
    pub state_time: f32,
    pub parameters: Vec<AnimatorParam>,
    pub states: Vec<AnimatorState>,
    pub transitions: Vec<AnimatorTransition>,
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
        }
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

    /// Blend input: Float value, Bool/Trigger as 0/1. Missing params read as 0.
    fn param_value(&self, name: &str) -> f32 {
        match self.param(name) {
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

    /// Re-read live params into the current state's blend tree. False when the state has no tree.
    fn evaluate_blend(&mut self) -> bool {
        let Some(idx) = self.states.iter().position(|s| s.name == self.state) else {
            return false;
        };
        let inputs = match self.states[idx].blend.as_ref() {
            None => return false,
            Some(b) => {
                let mut v = [0.0_f32; 2];
                for (i, name) in b.params.iter().take(2).enumerate() {
                    v[i] = self.param_value(name);
                }
                v
            }
        };
        if let Some(b) = self.states[idx].blend.as_mut() {
            b.evaluate(inputs);
        }
        true
    }

    /// Clip the current state should drive on `Animation` (dominant blend motion when blended).
    fn playback(&self) -> Option<Playback> {
        let s = self.current_state()?;
        let speed = s.speed.max(0.001);
        match &s.blend {
            Some(b) => {
                let m = b.active_motion()?;
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

    /// Blend tree on the current state, with weights from the last evaluation.
    pub fn current_blend(&self) -> Option<&BlendTree> {
        self.current_state().and_then(|s| s.blend.as_ref())
    }

    fn condition_met(&self, c: &AnimatorCondition) -> bool {
        match c {
            AnimatorCondition::BoolEq { param, value } => {
                self.param(param).is_some_and(|p| match p.kind {
                    AnimatorParamKind::Float => (p.float_value != 0.0) == *value,
                    AnimatorParamKind::Bool | AnimatorParamKind::Trigger => p.bool_value == *value,
                })
            }
            AnimatorCondition::FloatEq { param, value } => self
                .param(param)
                .is_some_and(|p| (p.float_value - *value).abs() < 1e-4),
            AnimatorCondition::FloatGreater { param, value } => {
                self.param(param).is_some_and(|p| p.float_value > *value)
            }
            AnimatorCondition::FloatLess { param, value } => {
                self.param(param).is_some_and(|p| p.float_value < *value)
            }
            AnimatorCondition::Trigger { param } => self
                .param(param)
                .is_some_and(|p| p.kind == AnimatorParamKind::Trigger && p.bool_value),
        }
    }

    fn conditions_met(&self, conditions: &[AnimatorCondition]) -> bool {
        conditions.iter().all(|c| self.condition_met(c))
    }

    fn consume_triggers(&mut self, conditions: &[AnimatorCondition]) {
        for c in conditions {
            if let AnimatorCondition::Trigger { param } = c {
                if let Some(p) = self.param_mut(param) {
                    p.bool_value = false;
                }
            }
            if let AnimatorCondition::BoolEq { param, value } = c {
                if *value {
                    if let Some(p) = self.param_mut(param) {
                        if p.kind == AnimatorParamKind::Trigger {
                            p.bool_value = false;
                        }
                    }
                }
            }
        }
    }

    /// First matching transition, or `None` to stay.
    pub fn pick_transition(&self) -> Option<&AnimatorTransition> {
        let current = self.state.as_str();
        let duration = self.current_state().map(|s| s.duration()).unwrap_or(0.0);
        for t in &self.transitions {
            let from_ok = t.from_any || t.from == current;
            if !from_ok {
                continue;
            }
            if t.to == current && t.from_any {
                continue;
            }
            if t.has_exit_time && duration > 0.0 && self.state_time < duration {
                continue;
            }
            if self.conditions_met(&t.conditions) {
                return Some(t);
            }
        }
        None
    }
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

    /// Apply a state's clip onto the sibling Animation (create if missing).
    /// Blend states evaluate live params and play the dominant motion from its first frame.
    pub fn apply_animator_state(&mut self, id: EntityId, state_name: &str) {
        let Some(playback) = self.animator_mut(id).and_then(|a| {
            a.find_state(state_name)?;
            a.state = state_name.into();
            a.state_time = 0.0;
            a.evaluate_blend();
            a.playback()
        }) else {
            return;
        };
        let Playback {
            clip,
            cells,
            fps,
            loop_,
        } = playback;
        match self.animation_mut(id) {
            Some(anim) => {
                anim.clip = clip;
                anim.cells = cells;
                anim.fps = fps;
                anim.loop_ = loop_;
                anim.time = 0.0;
                anim.frame = 0;
            }
            None => {
                self.set_animation(id, Some(Animation::new(clip, cells, fps, loop_)));
            }
        }
    }

    /// Re-evaluate the blend tree; when the dominant motion changes, switch the clip and keep phase.
    fn sync_blend_motion(&mut self, id: EntityId) {
        let Some(playback) = self.animator_mut(id).and_then(|a| {
            if a.evaluate_blend() {
                a.playback()
            } else {
                None
            }
        }) else {
            return;
        };
        let Some(anim) = self.animation_mut(id) else {
            return;
        };
        if anim.clip == playback.clip {
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
        let new_dur = if playback.cells.is_empty() || playback.fps <= 0.0 {
            0.0
        } else {
            playback.cells.len() as f32 / playback.fps
        };
        anim.clip = playback.clip;
        anim.cells = playback.cells;
        anim.fps = playback.fps;
        anim.loop_ = playback.loop_;
        anim.time = phase * new_dur;
        anim.frame = 0;
    }

    /// Evaluate transitions then advance state time. Call before clip frame advance.
    pub fn tick_animators(&mut self, dt: f32) {
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
            if let Some(a) = self.animator_mut(id) {
                let speed = a.current_state().map(|s| s.speed.max(0.001)).unwrap_or(1.0);
                a.state_time += dt * speed;
            }
            let Some(next) = self.animator(id).and_then(|a| {
                a.pick_transition()
                    .map(|t| (t.to.clone(), t.conditions.clone()))
            }) else {
                self.sync_blend_motion(id);
                continue;
            };
            let (to, conditions) = next;
            if let Some(a) = self.animator_mut(id) {
                a.consume_triggers(&conditions);
            }
            self.apply_animator_state(id, &to);
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
}
