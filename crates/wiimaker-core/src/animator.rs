//! Runtime animator (Unity Animator analogue) — baked controller + live params.

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

/// Resolved state (clip cells already looked up at hydrate).
#[derive(Clone, Debug)]
pub struct AnimatorState {
    pub name: String,
    pub clip: String,
    pub speed: f32,
    pub cells: Vec<String>,
    pub fps: f32,
    pub loop_: bool,
}

impl AnimatorState {
    pub fn duration(&self) -> f32 {
        if self.cells.is_empty() || self.fps <= 0.0 {
            return 0.0;
        }
        self.cells.len() as f32 / self.fps
    }

    pub fn playback_fps(&self) -> f32 {
        (self.fps * self.speed.max(0.001)).max(0.001)
    }
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
    pub fn apply_animator_state(&mut self, id: EntityId, state_name: &str) {
        let Some((clip, cells, fps, loop_)) = self.animator(id).and_then(|a| {
            a.find_state(state_name)
                .map(|s| (s.clip.clone(), s.cells.clone(), s.playback_fps(), s.loop_))
        }) else {
            return;
        };
        if let Some(a) = self.animator_mut(id) {
            a.state = state_name.into();
            a.state_time = 0.0;
        }
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
        });
        a.states.push(AnimatorState {
            name: "Walk".into(),
            clip: "walk".into(),
            speed: 1.0,
            cells: vec!["w0".into(), "w1".into()],
            fps: 10.0,
            loop_: true,
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
