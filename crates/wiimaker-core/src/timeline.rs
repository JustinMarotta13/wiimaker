//! Runtime PlayableDirector (Unity Timeline analogue). Host/editor only — not WSCN.

use crate::world::{Animation, EntityId, World};

#[cfg(feature = "std")]
mod alloc_types {
    pub use std::string::{String, ToString};
    pub use std::vec::Vec;
}

#[cfg(not(feature = "std"))]
mod alloc_types {
    extern crate alloc;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

use alloc_types::{String, ToString, Vec};

/// How to blend from this key to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveInterp {
    Linear,
    /// Hold this key's value until the next key.
    Constant,
    /// Smoothstep (cubic ease-in-out) toward the next key.
    Ease,
}

/// One key on a runtime curve. `t` is clip-local seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveKey {
    pub t: f32,
    pub v: f32,
    pub interp: CurveInterp,
}

/// Sorted keyframe curve. Sample holds the first value before the first key
/// and the last value after the last key.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curve {
    pub keys: Vec<CurveKey>,
}

impl Curve {
    /// Sort by time. Unsorted authoring input is normalized here and on asset load.
    pub fn from_keys(mut keys: Vec<CurveKey>) -> Self {
        keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(core::cmp::Ordering::Equal));
        Self { keys }
    }

    /// `None` when the curve has no keys.
    pub fn sample(&self, t: f32) -> Option<f32> {
        let keys = &self.keys;
        if keys.is_empty() {
            return None;
        }
        if keys.len() == 1 || t <= keys[0].t {
            return Some(keys[0].v);
        }
        let last = keys.len() - 1;
        if t >= keys[last].t {
            return Some(keys[last].v);
        }
        let mut idx = 0;
        for (i, key) in keys.iter().enumerate() {
            if key.t <= t {
                idx = i;
            } else {
                break;
            }
        }
        if idx >= last {
            return Some(keys[last].v);
        }
        let a = &keys[idx];
        let b = &keys[idx + 1];
        let span = b.t - a.t;
        if (-1e-8_f32..=1e-8_f32).contains(&span) {
            return Some(b.v);
        }
        let u = ((t - a.t) / span).clamp(0.0, 1.0);
        let w = match a.interp {
            CurveInterp::Constant => 0.0,
            CurveInterp::Linear => u,
            CurveInterp::Ease => {
                let u = u.clamp(0.0, 1.0);
                u * u * (3.0 - 2.0 * u)
            }
        };
        Some(a.v + (b.v - a.v) * w)
    }
}

/// Track kind. Mirrors `assets/<name>.timeline.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineTrackKind {
    Activation,
    Animation,
    Audio,
    Transform,
    /// Instant marker. Fires `signal` when the playhead crosses `start`.
    Signal,
    /// Activates `binding` inside the clip and drives that entity's director.
    Control,
    /// One float property (`Transform.rotation`, `Transform.scale_x`, `Transform.scale_y`, `Sprite.alpha`).
    Float,
}

/// One signal emitted when a Signal marker is crossed.
///
/// Queued on [`crate::world::World`] until [`crate::world::World::take_timeline_signals`].
/// `tick_timelines` clears the queue at the start of the tick, so events belong to that frame.
/// Scrub (`set_timeline_time`) appends until the next tick or take.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineSignal {
    /// PlayableDirector entity name (empty if unnamed).
    pub director: String,
    /// Timeline stem (`assets/<stem>.timeline.json`).
    pub timeline: String,
    /// Marker name (Unity signal name).
    pub signal: String,
    /// Optional string payload. Empty when the marker has none.
    pub payload: String,
    /// Bound receiver entity. Empty when the track is unbound.
    pub binding: String,
    /// Marker time on the director (clip `start`).
    pub time: f32,
}

/// One clip baked onto a runtime track.
#[derive(Clone, Debug)]
pub struct TimelineClipRuntime {
    pub start: f32,
    pub end: f32,
    /// Activation: GameObject flag while the playhead is inside this clip.
    /// `true` = appear (entity is off outside every clip on this binding).
    /// `false` = hide for the window, then restore the pre-timeline flag.
    pub active: bool,
    /// Animation stem (`*.anim.json`).
    pub clip: String,
    pub cells: Vec<String>,
    pub fps: f32,
    pub loop_clip: bool,
    /// Audio stem (`*.wav`).
    pub audio: String,
    pub volume: f32,
    pub from: [f32; 2],
    pub to: [f32; 2],
    /// Signal marker name. Empty on non-signal clips.
    pub signal: String,
    /// Signal string payload. Empty when omitted.
    pub payload: String,
    /// Clip-local X curve. `None` keeps the `from`→`to` lerp on X.
    pub curve_x: Option<Curve>,
    /// Clip-local Y curve. `None` keeps the `from`→`to` lerp on Y.
    pub curve_y: Option<Curve>,
    /// Float-track curve (`curves.value`).
    pub curve_value: Option<Curve>,
}

impl TimelineClipRuntime {
    pub fn contains(&self, time: f32) -> bool {
        time >= self.start && time <= self.end
    }

    pub fn local_t(&self, time: f32) -> f32 {
        let span = (self.end - self.start).max(1e-6);
        ((time - self.start) / span).clamp(0.0, 1.0)
    }
}

/// Resolved track. `binding` empty means unbound (Audio).
#[derive(Clone, Debug)]
pub struct TimelineTrackRuntime {
    pub name: String,
    pub kind: TimelineTrackKind,
    pub binding: String,
    /// Float property id. Empty on other kinds.
    pub property: String,
    pub clips: Vec<TimelineClipRuntime>,
}

#[derive(Clone, Debug)]
struct XyBaseline {
    name: String,
    x: f32,
    y: f32,
}

#[derive(Clone, Debug)]
struct AnimBaseline {
    name: String,
    had: bool,
    clip: String,
    cells: Vec<String>,
    fps: f32,
    loop_: bool,
    /// False while a timeline clip is driving this entity.
    released: bool,
}

/// Pre-timeline GameObject active flag for Activation tracks.
#[derive(Clone, Debug)]
struct ActiveBaseline {
    name: String,
    active: bool,
}

/// Pre-timeline value for a Float track property.
#[derive(Clone, Debug)]
struct FloatBaseline {
    name: String,
    property: String,
    value: f32,
}

/// Unity PlayableDirector. Authored stem + live transport + baked tracks.
#[derive(Clone, Debug)]
pub struct PlayableDirector {
    pub timeline: String,
    pub play_on_awake: bool,
    pub loop_: bool,
    pub time: f32,
    pub playing: bool,
    pub finished: bool,
    pub duration: f32,
    pub tracks: Vec<TimelineTrackRuntime>,
    /// Audio clips already fired this playthrough (`track_index << 16 | clip_index`).
    fired: Vec<u32>,
    xy_base: Vec<XyBaseline>,
    anim_base: Vec<AnimBaseline>,
    active_base: Vec<ActiveBaseline>,
    float_base: Vec<FloatBaseline>,
    baselines_ready: bool,
}

impl PlayableDirector {
    pub fn new(
        timeline: impl Into<String>,
        duration: f32,
        play_on_awake: bool,
        loop_: bool,
    ) -> Self {
        Self {
            timeline: timeline.into(),
            play_on_awake,
            loop_,
            time: 0.0,
            playing: play_on_awake,
            finished: false,
            duration: duration.max(0.0),
            tracks: Vec::new(),
            fired: Vec::new(),
            xy_base: Vec::new(),
            anim_base: Vec::new(),
            active_base: Vec::new(),
            float_base: Vec::new(),
            baselines_ready: false,
        }
    }
}

fn fire_key(track: usize, clip: usize) -> u32 {
    ((track as u32) << 16) | (clip as u32 & 0xffff)
}

impl World {
    /// Start (or restart if already finished) the director on `id`.
    pub fn play_timeline(&mut self, id: EntityId) -> bool {
        let restart = self
            .director(id)
            .is_some_and(|d| d.finished || (d.duration > 0.0 && d.time >= d.duration - 1e-4));
        let Some(d) = self.director_mut(id) else {
            return false;
        };
        d.playing = true;
        d.finished = false;
        if restart {
            d.time = 0.0;
            d.fired.clear();
            for b in &mut d.anim_base {
                b.released = false;
            }
        }
        let t = d.time;
        self.ensure_timeline_baselines(id);
        self.apply_director_root(id, -1.0, t, false, false);
        true
    }

    /// Stop and snap the playhead to 0 (restores pre-clip poses).
    pub fn stop_timeline(&mut self, id: EntityId) -> bool {
        let Some(d) = self.director_mut(id) else {
            return false;
        };
        d.playing = false;
        d.finished = false;
        d.time = 0.0;
        d.fired.clear();
        for b in &mut d.anim_base {
            b.released = false;
        }
        self.ensure_timeline_baselines(id);
        self.apply_director_root(id, -1.0, 0.0, false, true);
        true
    }

    /// Scrub. Fires audio when the playhead crosses a clip start.
    pub fn set_timeline_time(&mut self, id: EntityId, time: f32) -> bool {
        let Some(d) = self.director(id) else {
            return false;
        };
        let prev = d.time;
        let duration = d.duration;
        let t = if duration > 0.0 {
            time.clamp(0.0, duration)
        } else {
            0.0
        };
        if let Some(d) = self.director_mut(id) {
            d.time = t;
            d.finished = false;
        }
        self.ensure_timeline_baselines(id);
        self.apply_director_root(id, prev, t, true, false);
        true
    }

    /// Advance playing directors and apply every track.
    ///
    /// Clears the signal queue first. Directors driven by a playing Control track
    /// (and their nested targets) do not advance on their own clock.
    pub fn tick_timelines(&mut self, dt: f32) {
        self.clear_timeline_signals();
        let slaves = self.timeline_control_slaves();
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
            if slaves.iter().any(|s| *s == id) {
                continue;
            }
            let playing = self.director(id).is_some_and(|d| d.playing);
            if !playing {
                continue;
            }
            self.ensure_timeline_baselines(id);
            let (prev, duration, loop_) = {
                let d = self.director(id).unwrap();
                (d.time, d.duration, d.loop_)
            };
            if duration <= 0.0 {
                if let Some(d) = self.director_mut(id) {
                    d.time = 0.0;
                    if !loop_ {
                        d.playing = false;
                        d.finished = true;
                    }
                }
                self.apply_director_root(id, prev, 0.0, true, false);
                continue;
            }
            let t = prev + dt.max(0.0);
            if t >= duration {
                if loop_ {
                    self.apply_director_root(id, prev, duration, true, false);
                    let wrapped = t % duration;
                    if let Some(d) = self.director_mut(id) {
                        d.fired.clear();
                        d.time = wrapped;
                        d.finished = false;
                        d.playing = true;
                        for b in &mut d.anim_base {
                            b.released = false;
                        }
                    }
                    self.apply_director_root(id, -1.0, wrapped, true, false);
                } else {
                    if let Some(d) = self.director_mut(id) {
                        d.time = duration;
                        d.playing = false;
                        d.finished = true;
                    }
                    self.apply_director_root(id, prev, duration, true, true);
                }
            } else {
                if let Some(d) = self.director_mut(id) {
                    d.time = t;
                }
                self.apply_director_root(id, prev, t, true, false);
            }
        }
    }

    /// Snapshot bound entity poses once (after the whole scene has spawned).
    pub fn capture_timeline_baselines(&mut self) {
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
            if self.director(id).is_some() {
                self.capture_one_baseline(id);
            }
        }
    }

    /// Apply playing directors at the current playhead (no audio). Used after
    /// hydrate so `play_on_awake` Activation/`Transform` match `time == 0`.
    pub fn apply_playing_timelines(&mut self) {
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
            let Some(t) = self.director(id).and_then(|d| d.playing.then_some(d.time)) else {
                continue;
            };
            self.ensure_timeline_baselines(id);
            self.apply_director_root(id, -1.0, t, false, false);
        }
    }

    fn ensure_timeline_baselines(&mut self, id: EntityId) {
        let ready = self.director(id).is_some_and(|d| d.baselines_ready);
        if !ready && self.director(id).is_some() {
            self.capture_one_baseline(id);
        }
    }

    fn capture_one_baseline(&mut self, id: EntityId) {
        let Some(tracks) = self.director(id).map(|d| d.tracks.clone()) else {
            return;
        };
        let mut xy = Vec::new();
        let mut anims = Vec::new();
        let mut actives = Vec::new();
        let mut floats = Vec::new();
        for track in &tracks {
            if track.binding.is_empty() {
                continue;
            }
            let Some(eid) = self.find_by_name(&track.binding) else {
                continue;
            };
            if track.kind == TimelineTrackKind::Transform
                && !xy.iter().any(|b: &XyBaseline| b.name == track.binding)
            {
                if let Some(xf) = self.transform(eid) {
                    xy.push(XyBaseline {
                        name: track.binding.clone(),
                        x: xf.translation.x,
                        y: xf.translation.y,
                    });
                }
            }
            if track.kind == TimelineTrackKind::Animation
                && !anims.iter().any(|b: &AnimBaseline| b.name == track.binding)
            {
                if let Some(a) = self.animation(eid) {
                    anims.push(AnimBaseline {
                        name: track.binding.clone(),
                        had: true,
                        clip: a.clip.clone(),
                        cells: a.cells.clone(),
                        fps: a.fps,
                        loop_: a.loop_,
                        released: true,
                    });
                } else {
                    anims.push(AnimBaseline {
                        name: track.binding.clone(),
                        had: false,
                        clip: String::new(),
                        cells: Vec::new(),
                        fps: 10.0,
                        loop_: true,
                        released: true,
                    });
                }
            }
            if track.kind == TimelineTrackKind::Activation
                && !actives
                    .iter()
                    .any(|b: &ActiveBaseline| b.name == track.binding)
            {
                actives.push(ActiveBaseline {
                    name: track.binding.clone(),
                    active: self.is_active(eid),
                });
            }
            if track.kind == TimelineTrackKind::Float
                && !track.property.is_empty()
                && !floats.iter().any(|b: &FloatBaseline| {
                    b.name == track.binding && b.property == track.property
                })
            {
                if let Some(value) = self.read_float_property(&track.binding, &track.property) {
                    floats.push(FloatBaseline {
                        name: track.binding.clone(),
                        property: track.property.clone(),
                        value,
                    });
                }
            }
        }
        if let Some(d) = self.director_mut(id) {
            if !d.baselines_ready {
                d.xy_base = xy;
                d.anim_base = anims;
                d.active_base = actives;
                d.float_base = floats;
                d.baselines_ready = true;
            }
        }
    }

    fn apply_director_root(
        &mut self,
        id: EntityId,
        prev: f32,
        time: f32,
        fire_edges: bool,
        release_control: bool,
    ) {
        let mut stack = Vec::new();
        self.apply_director(id, prev, time, fire_edges, release_control, &mut stack);
    }

    /// Playing directors that own a Control track, plus the directors those tracks
    /// reach. Back-edges (self / cycles) are dropped so a timeline cannot drive itself.
    fn timeline_control_slaves(&self) -> Vec<EntityId> {
        let playing: Vec<EntityId> = self
            .iter_entities()
            .filter(|id| self.director(*id).is_some_and(|d| d.playing))
            .collect();
        let mut slaves = Vec::new();
        let mut visiting = Vec::new();
        for id in playing {
            if slaves.iter().any(|s| *s == id) {
                continue;
            }
            self.walk_control_targets(id, &mut visiting, &mut slaves);
        }
        slaves
    }

    fn walk_control_targets(
        &self,
        id: EntityId,
        visiting: &mut Vec<EntityId>,
        slaves: &mut Vec<EntityId>,
    ) {
        if visiting.iter().any(|v| *v == id) {
            return;
        }
        visiting.push(id);
        for child in self.control_director_targets(id) {
            if visiting.iter().any(|v| *v == child) {
                continue;
            }
            if !slaves.iter().any(|s| *s == child) {
                slaves.push(child);
            }
            self.walk_control_targets(child, visiting, slaves);
        }
        visiting.pop();
    }

    /// Other entities with a PlayableDirector named by this director's Control tracks.
    fn control_director_targets(&self, id: EntityId) -> Vec<EntityId> {
        let Some(tracks) = self.director(id).map(|d| d.tracks.clone()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for track in &tracks {
            if track.kind != TimelineTrackKind::Control || track.binding.is_empty() {
                continue;
            }
            let Some(eid) = self.find_by_name(&track.binding) else {
                continue;
            };
            if eid == id || self.director(eid).is_none() {
                continue;
            }
            if !out.iter().any(|e| *e == eid) {
                out.push(eid);
            }
        }
        out
    }

    fn apply_director(
        &mut self,
        id: EntityId,
        prev: f32,
        time: f32,
        fire_edges: bool,
        release_control: bool,
        stack: &mut Vec<EntityId>,
    ) {
        if stack.iter().any(|s| *s == id) {
            return;
        }
        stack.push(id);
        let Some(tracks) = self.director(id).map(|d| d.tracks.clone()) else {
            stack.pop();
            return;
        };
        let xy_base = self
            .director(id)
            .map(|d| d.xy_base.clone())
            .unwrap_or_default();
        let anim_base = self
            .director(id)
            .map(|d| d.anim_base.clone())
            .unwrap_or_default();
        let active_base = self
            .director(id)
            .map(|d| d.active_base.clone())
            .unwrap_or_default();
        let float_base = self
            .director(id)
            .map(|d| d.float_base.clone())
            .unwrap_or_default();

        self.apply_activation(&tracks, time, &active_base);
        self.apply_transform(&tracks, time, &xy_base);
        self.apply_float(&tracks, time, &float_base);
        self.apply_animation(id, &tracks, time, &anim_base);
        if fire_edges {
            self.apply_audio(id, &tracks, prev, time);
            self.apply_signals(id, &tracks, prev, time);
        }
        self.apply_control(id, &tracks, prev, time, fire_edges, release_control, stack);
        stack.pop();
    }

    fn apply_activation(
        &mut self,
        tracks: &[TimelineTrackRuntime],
        time: f32,
        base: &[ActiveBaseline],
    ) {
        let mut names: Vec<String> = Vec::new();
        for track in tracks {
            if track.kind == TimelineTrackKind::Activation
                && !track.binding.is_empty()
                && !names.iter().any(|n| n == &track.binding)
            {
                names.push(track.binding.clone());
            }
        }
        for name in names {
            let mut inside: Option<bool> = None;
            let mut has_appear = false;
            for track in tracks {
                if track.kind != TimelineTrackKind::Activation || track.binding != name {
                    continue;
                }
                for clip in &track.clips {
                    if clip.active {
                        has_appear = true;
                    }
                    if clip.contains(time) {
                        inside = Some(clip.active);
                    }
                }
            }
            let baseline = base
                .iter()
                .find(|b| b.name == name)
                .map(|b| b.active)
                .unwrap_or(true);
            // Appear (`active: true`): off until the first clip, off after the
            // last clip, off on Stop at t=0. Hide (`active: false` only): off
            // while inside, restore the capture flag outside. Scene JSON has no
            // GameObject inactive bit, so appear cannot rely on that snapshot.
            let outside = if has_appear { false } else { baseline };
            if let Some(eid) = self.find_by_name(&name) {
                self.set_active(eid, inside.unwrap_or(outside));
            }
        }
    }

    fn apply_transform(&mut self, tracks: &[TimelineTrackRuntime], time: f32, base: &[XyBaseline]) {
        let mut names: Vec<String> = Vec::new();
        for track in tracks {
            if track.kind == TimelineTrackKind::Transform
                && !track.binding.is_empty()
                && !names.iter().any(|n| n == &track.binding)
            {
                names.push(track.binding.clone());
            }
        }
        for name in names {
            let mut pose: Option<[f32; 2]> = None;
            let mut last_end = f32::NEG_INFINITY;
            let mut last_to: Option<[f32; 2]> = None;
            let mut any = false;
            for track in tracks {
                if track.kind != TimelineTrackKind::Transform || track.binding != name {
                    continue;
                }
                for clip in &track.clips {
                    any = true;
                    if clip.contains(time) {
                        pose = Some([eval_axis(clip, time, 0), eval_axis(clip, time, 1)]);
                    } else if time > clip.end && clip.end >= last_end {
                        last_end = clip.end;
                        last_to =
                            Some([eval_axis(clip, clip.end, 0), eval_axis(clip, clip.end, 1)]);
                    }
                }
            }
            let xy = if let Some(p) = pose {
                Some(p)
            } else if time > 0.0 {
                last_to.or_else(|| base.iter().find(|b| b.name == name).map(|b| [b.x, b.y]))
            } else if any {
                base.iter().find(|b| b.name == name).map(|b| [b.x, b.y])
            } else {
                None
            };
            if let Some([x, y]) = xy {
                if let Some(eid) = self.find_by_name(&name) {
                    if let Some(xf) = self.transform_mut(eid) {
                        xf.translation.x = x;
                        xf.translation.y = y;
                    }
                }
            }
        }
    }

    fn apply_float(&mut self, tracks: &[TimelineTrackRuntime], time: f32, base: &[FloatBaseline]) {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for track in tracks {
            if track.kind == TimelineTrackKind::Float
                && !track.binding.is_empty()
                && !track.property.is_empty()
                && !pairs
                    .iter()
                    .any(|(n, p)| n == &track.binding && p == &track.property)
            {
                pairs.push((track.binding.clone(), track.property.clone()));
            }
        }
        for (name, prop) in pairs {
            let mut pose: Option<f32> = None;
            let mut last_end = f32::NEG_INFINITY;
            let mut last_v: Option<f32> = None;
            let mut any = false;
            for track in tracks {
                if track.kind != TimelineTrackKind::Float
                    || track.binding != name
                    || track.property != prop
                {
                    continue;
                }
                for clip in &track.clips {
                    let Some(curve) = clip.curve_value.as_ref() else {
                        continue;
                    };
                    if curve.keys.is_empty() {
                        continue;
                    }
                    any = true;
                    if clip.contains(time) {
                        pose = curve.sample(time - clip.start);
                    } else if time > clip.end && clip.end >= last_end {
                        last_end = clip.end;
                        last_v = curve.sample(clip.end - clip.start);
                    }
                }
            }
            let value = if let Some(v) = pose {
                Some(v)
            } else if time > 0.0 {
                last_v.or_else(|| {
                    base.iter()
                        .find(|b| b.name == name && b.property == prop)
                        .map(|b| b.value)
                })
            } else if any {
                base.iter()
                    .find(|b| b.name == name && b.property == prop)
                    .map(|b| b.value)
            } else {
                None
            };
            if let Some(v) = value {
                self.write_float_property(&name, &prop, v);
            }
        }
    }

    fn read_float_property(&self, name: &str, property: &str) -> Option<f32> {
        let id = self.find_by_name(name)?;
        match property {
            "Transform.rotation" => {
                let xf = self.transform(id)?;
                let (z, _, _) = xf.rotation.to_euler(glam::EulerRot::ZYX);
                Some(z.to_degrees())
            }
            "Transform.scale_x" => self.transform(id).map(|t| t.scale.x),
            "Transform.scale_y" => self.transform(id).map(|t| t.scale.y),
            "Sprite.alpha" => self.sprite(id).map(|s| s.color.a as f32 / 255.0),
            _ => None,
        }
    }

    fn write_float_property(&mut self, name: &str, property: &str, value: f32) {
        let Some(id) = self.find_by_name(name) else {
            return;
        };
        match property {
            "Transform.rotation" => {
                if let Some(xf) = self.transform_mut(id) {
                    xf.rotation = crate::math::Quat::from_rotation_z(value.to_radians());
                }
            }
            "Transform.scale_x" => {
                if let Some(xf) = self.transform_mut(id) {
                    xf.scale.x = value;
                }
            }
            "Transform.scale_y" => {
                if let Some(xf) = self.transform_mut(id) {
                    xf.scale.y = value;
                }
            }
            "Sprite.alpha" => {
                if let Some(sprite) = self.sprite_mut(id) {
                    let scaled = value.clamp(0.0, 1.0) * 255.0;
                    sprite.color.a = (scaled + 0.5) as u8;
                }
            }
            _ => {}
        }
    }

    fn apply_animation(
        &mut self,
        director_id: EntityId,
        tracks: &[TimelineTrackRuntime],
        time: f32,
        base: &[AnimBaseline],
    ) {
        let mut names: Vec<String> = Vec::new();
        for track in tracks {
            if track.kind == TimelineTrackKind::Animation
                && !track.binding.is_empty()
                && !names.iter().any(|n| n == &track.binding)
            {
                names.push(track.binding.clone());
            }
        }
        for name in names {
            let mut driving: Option<&TimelineClipRuntime> = None;
            for track in tracks {
                if track.kind != TimelineTrackKind::Animation || track.binding != name {
                    continue;
                }
                for clip in &track.clips {
                    if clip.contains(time) {
                        driving = Some(clip);
                    }
                }
            }
            let Some(eid) = self.find_by_name(&name) else {
                continue;
            };
            if let Some(clip) = driving {
                let local = (time - clip.start).max(0.0);
                let fps = clip.fps.max(0.001);
                let cells = clip.cells.clone();
                let n = cells.len();
                let mut frame = if n == 0 { 0 } else { (local * fps) as usize };
                if n > 0 {
                    if clip.loop_clip {
                        frame %= n;
                    } else if frame >= n {
                        frame = n - 1;
                    }
                }
                match self.animation_mut(eid) {
                    Some(anim) => {
                        anim.clip = clip.clip.clone();
                        anim.cells = cells;
                        anim.fps = fps;
                        anim.loop_ = clip.loop_clip;
                        anim.time = local;
                        anim.frame = frame;
                        anim.hold_time = true;
                    }
                    None => {
                        let mut anim =
                            Animation::new(clip.clip.clone(), cells, fps, clip.loop_clip);
                        anim.time = local;
                        anim.frame = frame;
                        anim.hold_time = true;
                        self.set_animation(eid, Some(anim));
                    }
                }
                if let Some(d) = self.director_mut(director_id) {
                    if let Some(b) = d.anim_base.iter_mut().find(|b| b.name == name) {
                        b.released = false;
                    }
                }
            } else if let Some(b) = base.iter().find(|b| b.name == name) {
                let already = self
                    .director(director_id)
                    .and_then(|d| d.anim_base.iter().find(|x| x.name == name))
                    .is_some_and(|x| x.released);
                if !already {
                    if b.had {
                        let mut anim =
                            Animation::new(b.clip.clone(), b.cells.clone(), b.fps, b.loop_);
                        anim.hold_time = false;
                        self.set_animation(eid, Some(anim));
                    } else {
                        self.set_animation(eid, None);
                    }
                    if let Some(d) = self.director_mut(director_id) {
                        if let Some(slot) = d.anim_base.iter_mut().find(|x| x.name == name) {
                            slot.released = true;
                        }
                    }
                } else if let Some(anim) = self.animation_mut(eid) {
                    anim.hold_time = false;
                }
            }
        }
    }

    fn apply_audio(&mut self, id: EntityId, tracks: &[TimelineTrackRuntime], prev: f32, time: f32) {
        let mut oneshots: Vec<(String, f32)> = Vec::new();
        let mut arm: Vec<u32> = Vec::new();
        let mut disarm: Vec<u32> = Vec::new();
        let fired = self
            .director(id)
            .map(|d| d.fired.clone())
            .unwrap_or_default();
        for (ti, track) in tracks.iter().enumerate() {
            if track.kind != TimelineTrackKind::Audio {
                continue;
            }
            for (ci, clip) in track.clips.iter().enumerate() {
                let key = fire_key(ti, ci);
                if time < clip.start {
                    disarm.push(key);
                    continue;
                }
                // `prev == start && time > prev` covers a clip that begins at the
                // playhead (including t = 0 on the first advancing tick).
                let crossed = edge_crossed(prev, time, clip.start);
                if crossed && !fired.contains(&key) {
                    oneshots.push((clip.audio.clone(), clip.volume.clamp(0.0, 1.0)));
                    arm.push(key);
                }
            }
        }
        if let Some(d) = self.director_mut(id) {
            d.fired.retain(|k| !disarm.contains(k));
            for k in arm {
                if !d.fired.contains(&k) {
                    d.fired.push(k);
                }
            }
        }
        for (clip, volume) in oneshots {
            if !clip.is_empty() {
                self.play_oneshot(clip, volume);
            }
        }
    }

    fn apply_signals(
        &mut self,
        id: EntityId,
        tracks: &[TimelineTrackRuntime],
        prev: f32,
        time: f32,
    ) {
        let mut events: Vec<TimelineSignal> = Vec::new();
        let mut arm: Vec<u32> = Vec::new();
        let mut disarm: Vec<u32> = Vec::new();
        let fired = self
            .director(id)
            .map(|d| d.fired.clone())
            .unwrap_or_default();
        let director_name = self.name(id).unwrap_or("").to_string();
        let timeline = self
            .director(id)
            .map(|d| d.timeline.clone())
            .unwrap_or_default();
        for (ti, track) in tracks.iter().enumerate() {
            if track.kind != TimelineTrackKind::Signal {
                continue;
            }
            for (ci, clip) in track.clips.iter().enumerate() {
                let key = fire_key(ti, ci);
                let at = clip.start;
                if time < at {
                    disarm.push(key);
                    continue;
                }
                // Same crossing rule as Audio: once per playthrough until the
                // playhead moves strictly before the marker (scrub back, stop,
                // or loop wrap which clears `fired`). A backward scrub does not
                // fire. Holding or scrubbing further forward does not fire again.
                let crossed = edge_crossed(prev, time, at);
                if crossed && !fired.contains(&key) && !clip.signal.is_empty() {
                    events.push(TimelineSignal {
                        director: director_name.clone(),
                        timeline: timeline.clone(),
                        signal: clip.signal.clone(),
                        payload: clip.payload.clone(),
                        binding: track.binding.clone(),
                        time: at,
                    });
                    arm.push(key);
                }
            }
        }
        if let Some(d) = self.director_mut(id) {
            d.fired.retain(|k| !disarm.contains(k));
            for k in arm {
                if !d.fired.contains(&k) {
                    d.fired.push(k);
                }
            }
        }
        for event in events {
            self.push_timeline_signal(event);
        }
    }

    fn apply_control(
        &mut self,
        id: EntityId,
        tracks: &[TimelineTrackRuntime],
        prev: f32,
        time: f32,
        fire_edges: bool,
        release_control: bool,
        stack: &mut Vec<EntityId>,
    ) {
        let mut names: Vec<String> = Vec::new();
        for track in tracks {
            if track.kind == TimelineTrackKind::Control
                && !track.binding.is_empty()
                && !names.iter().any(|n| n == &track.binding)
            {
                names.push(track.binding.clone());
            }
        }
        for name in names {
            let Some(eid) = self.find_by_name(&name) else {
                continue;
            };
            // Self-control and cycles: the target is already on the evaluation stack.
            if eid == id || stack.iter().any(|s| *s == eid) {
                continue;
            }
            let mut driving: Option<(f32, f32)> = None;
            if !release_control {
                for track in tracks {
                    if track.kind != TimelineTrackKind::Control || track.binding != name {
                        continue;
                    }
                    for clip in &track.clips {
                        if clip.contains(time) {
                            driving = Some((clip.start, clip.end));
                        }
                    }
                }
            }
            if let Some((start, end)) = driving {
                self.set_active(eid, true);
                if self.director(eid).is_some() {
                    let dur = self.director(eid).map(|d| d.duration).unwrap_or(0.0);
                    let mut local = (time - start).max(0.0);
                    if dur > 0.0 {
                        local = local.min(dur);
                    }
                    let prev_inside = prev >= start && prev <= end;
                    let mut prev_local = if prev_inside {
                        (prev - start).max(0.0)
                    } else {
                        -1.0
                    };
                    if dur > 0.0 && prev_local >= 0.0 {
                        prev_local = prev_local.min(dur);
                    }
                    let parent_playing = self.director(id).is_some_and(|d| d.playing);
                    let child_finished = dur > 0.0 && local >= dur - 1e-4;
                    if let Some(d) = self.director_mut(eid) {
                        d.time = local;
                        d.playing = parent_playing && !child_finished;
                        d.finished = child_finished;
                    }
                    self.apply_director(eid, prev_local, local, fire_edges, false, stack);
                }
            } else {
                self.set_active(eid, false);
                if self.director(eid).is_some() {
                    self.stop_controlled(eid, stack);
                }
            }
        }
    }

    /// Stop a nested director and release its Control targets (does not sample clips).
    fn stop_controlled(&mut self, id: EntityId, stack: &mut Vec<EntityId>) {
        if stack.iter().any(|s| *s == id) {
            return;
        }
        let Some(d) = self.director_mut(id) else {
            return;
        };
        d.playing = false;
        d.finished = false;
        d.time = 0.0;
        d.fired.clear();
        for b in &mut d.anim_base {
            b.released = false;
        }
        self.ensure_timeline_baselines(id);
        self.apply_director(id, -1.0, 0.0, false, true, stack);
    }
}

fn edge_crossed(prev: f32, time: f32, at: f32) -> bool {
    time >= at && (prev < at || (prev == at && time > prev))
}

/// X (`axis == 0`) or Y. A curve replaces that axis; the other axis stays `from`→`to`.
fn eval_axis(clip: &TimelineClipRuntime, time: f32, axis: u8) -> f32 {
    let local = time - clip.start;
    let curve = if axis == 0 {
        clip.curve_x.as_ref()
    } else {
        clip.curve_y.as_ref()
    };
    if let Some(curve) = curve {
        if let Some(v) = curve.sample(local) {
            return v;
        }
    }
    let u = clip.local_t(time);
    if axis == 0 {
        clip.from[0] + (clip.to[0] - clip.from[0]) * u
    } else {
        clip.from[1] + (clip.to[1] - clip.from[1]) * u
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::Transform;

    fn clip_base(
        start: f32,
        end: f32,
        active: bool,
        clip: &str,
        cells: Vec<String>,
        audio: &str,
        from: [f32; 2],
        to: [f32; 2],
    ) -> TimelineClipRuntime {
        TimelineClipRuntime {
            start,
            end,
            active,
            clip: clip.into(),
            cells,
            fps: 10.0,
            loop_clip: true,
            audio: audio.into(),
            volume: 1.0,
            from,
            to,
            signal: String::new(),
            payload: String::new(),
            curve_x: None,
            curve_y: None,
            curve_value: None,
        }
    }

    fn clip_signal(time: f32, name: &str, payload: &str) -> TimelineClipRuntime {
        let mut clip = clip_base(time, time, true, "", Vec::new(), "", [0.0, 0.0], [0.0, 0.0]);
        clip.signal = name.into();
        clip.payload = payload.into();
        clip
    }

    fn clip_control(start: f32, end: f32) -> TimelineClipRuntime {
        clip_base(start, end, true, "", Vec::new(), "", [0.0, 0.0], [0.0, 0.0])
    }

    fn clip_xform(start: f32, end: f32, from: [f32; 2], to: [f32; 2]) -> TimelineClipRuntime {
        clip_base(start, end, true, "", Vec::new(), "", from, to)
    }

    fn clip_act(start: f32, end: f32, active: bool) -> TimelineClipRuntime {
        clip_base(
            start,
            end,
            active,
            "",
            Vec::new(),
            "",
            [0.0, 0.0],
            [0.0, 0.0],
        )
    }

    fn clip_anim(start: f32, end: f32, stem: &str, cells: &[&str]) -> TimelineClipRuntime {
        clip_base(
            start,
            end,
            true,
            stem,
            cells.iter().map(|s| (*s).to_string()).collect(),
            "",
            [0.0, 0.0],
            [0.0, 0.0],
        )
    }

    fn clip_audio(start: f32, end: f32, stem: &str) -> TimelineClipRuntime {
        clip_base(
            start,
            end,
            true,
            "",
            Vec::new(),
            stem,
            [0.0, 0.0],
            [0.0, 0.0],
        )
    }

    fn track(
        name: &str,
        kind: TimelineTrackKind,
        binding: &str,
        clips: Vec<TimelineClipRuntime>,
    ) -> TimelineTrackRuntime {
        TimelineTrackRuntime {
            name: name.into(),
            kind,
            binding: binding.into(),
            property: String::new(),
            clips,
        }
    }

    #[test]
    fn transform_lerps_and_holds_end() {
        let mut world = World::new();
        let cam = world.spawn_named("MainCamera", Transform::from_xy(10.0, 10.0));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 4.0, true, false);
        d.tracks.push(TimelineTrackRuntime {
            name: "CamSlide".into(),
            kind: TimelineTrackKind::Transform,
            binding: "MainCamera".into(),
            property: String::new(),
            clips: vec![clip_xform(0.0, 2.0, [320.0, 240.0], [400.0, 240.0])],
        });
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.set_timeline_time(dir, 1.0);
        let xf = world.transform(cam).unwrap();
        assert!((xf.translation.x - 360.0).abs() < 1e-3);
        assert!((xf.translation.y - 240.0).abs() < 1e-3);
        assert!((xf.translation.z).abs() < 1e-4);
        world.set_timeline_time(dir, 3.0);
        let xf = world.transform(cam).unwrap();
        assert!(
            (xf.translation.x - 400.0).abs() < 1e-3,
            "hold `to` after the clip"
        );
        world.stop_timeline(dir);
        let xf = world.transform(cam).unwrap();
        assert!(
            (xf.translation.x - 320.0).abs() < 1e-3,
            "stop snaps to t=0, which is the clip start"
        );
    }

    #[test]
    fn activation_appear_and_hide_without_manual_inactive() {
        let mut world = World::new();
        let ghost = world.spawn_named("IntroGhost", Transform::default());
        let hud = world.spawn_named("Hud", Transform::default());
        assert!(world.is_active(ghost), "scene-like spawn starts active");
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 5.0, true, false);
        d.tracks.push(track(
            "Appear",
            TimelineTrackKind::Activation,
            "IntroGhost",
            vec![clip_act(0.5, 4.0, true)],
        ));
        d.tracks.push(track(
            "HideHud",
            TimelineTrackKind::Activation,
            "Hud",
            vec![clip_act(1.0, 2.0, false)],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.apply_playing_timelines();

        assert!(!world.is_active(ghost), "appear: off before 0.5s");
        assert!(world.is_active(hud), "hide-only: on before the clip");

        world.set_timeline_time(dir, 0.2);
        assert!(!world.is_active(ghost));

        world.set_timeline_time(dir, 1.5);
        assert!(world.is_active(ghost), "inside appear clip");
        assert!(!world.is_active(hud), "active:false disables while inside");

        world.set_timeline_time(dir, 3.0);
        assert!(world.is_active(ghost));
        assert!(world.is_active(hud), "after hide clip restore baseline");

        world.set_timeline_time(dir, 4.0);
        assert!(
            world.is_active(ghost),
            "clip end is inclusive; still inside appear"
        );

        world.set_timeline_time(dir, 4.5);
        assert!(
            !world.is_active(ghost),
            "after the last appear clip the entity stays off"
        );

        world.stop_timeline(dir);
        assert!(!world.is_active(ghost), "Stop at t=0 is still before 0.5s");
        assert!(world.is_active(hud), "stop restores HUD");
    }

    #[test]
    fn play_on_awake_false_stays_stopped() {
        let mut world = World::new();
        let cam = world.spawn_named("MainCamera", Transform::from_xy(10.0, 10.0));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 4.0, false, false);
        d.tracks.push(track(
            "CamSlide",
            TimelineTrackKind::Transform,
            "MainCamera",
            vec![clip_xform(0.0, 2.0, [320.0, 240.0], [400.0, 240.0])],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.tick_timelines(0.5);
        world.tick_timelines(0.5);
        let d = world.director(dir).unwrap();
        assert!(!d.playing);
        assert!(!d.finished);
        assert!((d.time).abs() < 1e-6);
        let xf = world.transform(cam).unwrap();
        assert!(
            (xf.translation.x - 10.0).abs() < 1e-3,
            "stopped director does not apply tracks"
        );
    }

    #[test]
    fn loop_wrap_refires_audio_and_resets_animation() {
        let mut world = World::new();
        let player = world.spawn_named("Player", Transform::default());
        world.set_animation(
            player,
            Some(Animation::new("idle", vec!["idle0".into()], 8.0, true)),
        );
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 2.0, true, true);
        d.tracks.push(track(
            "Move",
            TimelineTrackKind::Animation,
            "Player",
            vec![clip_anim(0.0, 1.0, "chomp", &["a", "b"])],
        ));
        d.tracks.push(track(
            "Stinger",
            TimelineTrackKind::Audio,
            "",
            vec![clip_audio(0.0, 0.4, "beep")],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();

        world.tick_timelines(0.05);
        let shots = world.drain_oneshots();
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].clip, "beep");
        assert_eq!(world.animation(player).unwrap().clip, "chomp");

        world.tick_timelines(1.2);
        assert_eq!(
            world.animation(player).unwrap().clip,
            "idle",
            "outside anim clip restores baseline"
        );
        assert!(world.drain_oneshots().is_empty());

        // duration 2.0; time is ~1.25; another 1.0 wraps to ~0.25
        world.tick_timelines(1.0);
        let d = world.director(dir).unwrap();
        assert!(d.playing);
        assert!(!d.finished);
        assert!(d.time < 1.0, "wrapped time {}", d.time);
        let shots = world.drain_oneshots();
        assert_eq!(shots.len(), 1, "audio re-fires after loop wrap");
        assert_eq!(shots[0].clip, "beep");
        assert_eq!(
            world.animation(player).unwrap().clip,
            "chomp",
            "wrap re-enters anim clip from local t"
        );
        assert!(world.animation(player).unwrap().hold_time);
    }

    #[test]
    fn non_loop_finish_then_play_timeline_restarts() {
        let mut world = World::new();
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 1.0, true, false);
        d.tracks.push(track(
            "Stinger",
            TimelineTrackKind::Audio,
            "",
            vec![clip_audio(0.0, 0.2, "beep")],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.tick_timelines(0.05);
        assert_eq!(world.drain_oneshots().len(), 1);
        world.tick_timelines(2.0);
        {
            let d = world.director(dir).unwrap();
            assert!(!d.playing);
            assert!(d.finished);
            assert!((d.time - 1.0).abs() < 1e-4);
        }
        assert!(world.drain_oneshots().is_empty());

        assert!(world.play_timeline(dir));
        {
            let d = world.director(dir).unwrap();
            assert!(d.playing);
            assert!(!d.finished);
            assert!((d.time).abs() < 1e-4);
        }
        world.tick_timelines(0.05);
        let shots = world.drain_oneshots();
        assert_eq!(shots.len(), 1, "restart re-fires audio from t=0");
        assert_eq!(shots[0].clip, "beep");
    }

    #[test]
    fn scrub_back_rearms_audio() {
        let mut world = World::new();
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 2.0, false, false);
        d.tracks.push(track(
            "Stinger",
            TimelineTrackKind::Audio,
            "",
            vec![clip_audio(0.5, 0.8, "beep")],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.set_timeline_time(dir, 0.6);
        assert_eq!(world.drain_oneshots().len(), 1);
        world.set_timeline_time(dir, 0.1);
        assert!(world.drain_oneshots().is_empty());
        world.set_timeline_time(dir, 0.6);
        assert_eq!(world.drain_oneshots().len(), 1, "scrub back re-arms");
    }

    #[test]
    fn missing_animation_and_stem_do_not_panic() {
        let mut world = World::new();
        let player = world.spawn_named("Player", Transform::default());
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 2.0, true, false);
        d.tracks.push(track(
            "Move",
            TimelineTrackKind::Animation,
            "Player",
            vec![clip_anim(0.0, 1.0, "missing-clip", &[])],
        ));
        d.tracks.push(track(
            "Nobody",
            TimelineTrackKind::Activation,
            "NoSuchEntity",
            vec![clip_act(0.0, 1.0, false)],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.set_timeline_time(dir, 0.2);
        let anim = world.animation(player).unwrap();
        assert_eq!(anim.clip, "missing-clip");
        assert!(anim.cells.is_empty());
        world.set_timeline_time(dir, 1.5);
        assert!(
            world.animation(player).is_none(),
            "missing pre-timeline Animation is removed outside the clip"
        );
    }

    fn signal_names(world: &mut World) -> Vec<String> {
        world
            .take_timeline_signals()
            .into_iter()
            .map(|s| s.signal)
            .collect()
    }

    #[test]
    fn signal_fires_once_per_crossing_scrub_and_restart() {
        let mut world = World::new();
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 2.0, true, false);
        d.tracks.push(track(
            "Cues",
            TimelineTrackKind::Signal,
            "Player",
            vec![
                clip_signal(0.0, "Begin", ""),
                clip_signal(0.5, "IntroDone", "go"),
            ],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();

        world.tick_timelines(0.05);
        let fired = world.take_timeline_signals();
        assert_eq!(
            fired.len(),
            1,
            "marker at 0 fires on the first advancing tick"
        );
        assert_eq!(fired[0].signal, "Begin");
        assert_eq!(fired[0].binding, "Player");
        assert_eq!(fired[0].director, "Cutscene");
        assert_eq!(fired[0].timeline, "intro");
        assert!(fired[0].payload.is_empty());
        assert!(world.timeline_signals().is_empty());

        world.tick_timelines(0.2);
        assert!(
            signal_names(&mut world).is_empty(),
            "staying before the next marker does not re-fire Begin"
        );

        world.tick_timelines(0.4);
        let fired = world.take_timeline_signals();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].signal, "IntroDone");
        assert_eq!(fired[0].payload, "go");
        assert!((fired[0].time - 0.5).abs() < 1e-4);

        world.tick_timelines(0.2);
        assert!(
            signal_names(&mut world).is_empty(),
            "holding past the marker does not double-fire"
        );

        world.set_timeline_time(dir, 1.2);
        assert!(
            signal_names(&mut world).is_empty(),
            "forward scrub past an already-fired marker does not fire again"
        );
        world.set_timeline_time(dir, 0.2);
        assert!(
            signal_names(&mut world).is_empty(),
            "scrub back re-arms without firing"
        );
        world.set_timeline_time(dir, 0.8);
        let fired = world.take_timeline_signals();
        assert_eq!(fired.len(), 1, "next forward cross fires once");
        assert_eq!(fired[0].signal, "IntroDone");

        world.set_timeline_time(dir, 0.1);
        assert!(signal_names(&mut world).is_empty());
        world.tick_timelines(2.0);
        let _ = world.take_timeline_signals();
        {
            let d = world.director(dir).unwrap();
            assert!(d.finished);
            assert!(!d.playing);
        }
        assert!(world.play_timeline(dir));
        assert!(
            signal_names(&mut world).is_empty(),
            "play() seeks without firing; the next tick crosses markers"
        );
        world.tick_timelines(0.05);
        assert_eq!(signal_names(&mut world), vec!["Begin".to_string()]);
    }

    #[test]
    fn signal_loop_wrap_refires_and_backward_scrub_does_not() {
        let mut world = World::new();
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 1.0, true, true);
        d.tracks.push(track(
            "Cues",
            TimelineTrackKind::Signal,
            "",
            vec![clip_signal(0.25, "Ping", "")],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();

        world.tick_timelines(0.1);
        assert!(signal_names(&mut world).is_empty());
        world.tick_timelines(0.2);
        assert_eq!(signal_names(&mut world), vec!["Ping".to_string()]);
        world.tick_timelines(1.0);
        let d = world.director(dir).unwrap();
        assert!(d.playing && d.time < 0.5, "wrapped {}", d.time);
        assert_eq!(
            signal_names(&mut world),
            vec!["Ping".to_string()],
            "loop wrap clears the fired set and the marker fires again"
        );

        world.set_timeline_time(dir, 0.9);
        let _ = world.take_timeline_signals();
        world.set_timeline_time(dir, 0.05);
        assert!(
            signal_names(&mut world).is_empty(),
            "a backward scrub across the marker disarms and does not fire"
        );
    }

    #[test]
    fn signal_jump_fires_each_marker_once() {
        let mut world = World::new();
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 3.0, false, false);
        d.tracks.push(track(
            "Cues",
            TimelineTrackKind::Signal,
            "Player",
            vec![clip_signal(0.4, "A", ""), clip_signal(0.8, "B", "x")],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.set_timeline_time(dir, 1.0);
        assert_eq!(
            signal_names(&mut world),
            vec!["A".to_string(), "B".to_string()]
        );
        world.set_timeline_time(dir, 2.0);
        assert!(signal_names(&mut world).is_empty());
    }

    #[test]
    fn control_activates_and_drives_nested_director() {
        let mut world = World::new();
        let ghost = world.spawn_named("IntroGhost", Transform::default());
        let child = world.spawn_named("Child", Transform::default());
        let mut sub = PlayableDirector::new("sub", 1.0, false, false);
        sub.tracks.push(track(
            "Cue",
            TimelineTrackKind::Signal,
            "Child",
            vec![clip_signal(0.0, "SubStart", "")],
        ));
        world.set_director(child, Some(sub));
        let parent = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 3.0, true, false);
        d.tracks.push(track(
            "Nest",
            TimelineTrackKind::Control,
            "IntroGhost",
            vec![clip_control(0.5, 1.5)],
        ));
        d.tracks.push(track(
            "Sub",
            TimelineTrackKind::Control,
            "Child",
            vec![clip_control(0.5, 1.5)],
        ));
        world.set_director(parent, Some(d));
        world.capture_timeline_baselines();
        world.apply_playing_timelines();
        assert!(!world.is_active(ghost), "appear: off before the clip");
        assert!(!world.is_active(child));
        assert!(!world.director(child).unwrap().playing);

        world.tick_timelines(0.6);
        assert!(world.is_active(ghost));
        assert!(world.is_active(child));
        let child_d = world.director(child).unwrap();
        assert!(
            (child_d.time - 0.1).abs() < 1e-3,
            "local time from clip start, got {}",
            child_d.time
        );
        assert!(child_d.playing);
        assert_eq!(
            signal_names(&mut world),
            vec!["SubStart".to_string()],
            "nested director fires its marker when the control clip enters"
        );

        world.tick_timelines(0.2);
        assert!(
            (world.director(child).unwrap().time - 0.3).abs() < 1e-3,
            "child follows the parent clock, not a second dt"
        );
        assert!(signal_names(&mut world).is_empty());

        world.tick_timelines(1.0);
        assert!(!world.is_active(ghost), "off after the clip");
        assert!(!world.is_active(child));
        let child_d = world.director(child).unwrap();
        assert!(!child_d.playing);
        assert!(
            child_d.time.abs() < 1e-4,
            "stop snaps the nested director to 0"
        );

        world.set_timeline_time(parent, 1.0);
        assert!(world.is_active(child));
        assert!((world.director(child).unwrap().time - 0.5).abs() < 1e-3);
        world.stop_timeline(parent);
        assert!(!world.is_active(ghost));
        assert!(!world.is_active(child));
        assert!(!world.director(child).unwrap().playing);
        assert!(world.director(child).unwrap().time.abs() < 1e-4);
    }

    #[test]
    fn control_ignores_self_and_cycles() {
        let mut world = World::new();
        let a = world.spawn_named("A", Transform::default());
        let b = world.spawn_named("B", Transform::default());
        let mut da = PlayableDirector::new("loop-a", 2.0, true, false);
        da.tracks.push(track(
            "Self",
            TimelineTrackKind::Control,
            "A",
            vec![clip_control(0.0, 2.0)],
        ));
        da.tracks.push(track(
            "ToB",
            TimelineTrackKind::Control,
            "B",
            vec![clip_control(0.0, 2.0)],
        ));
        world.set_director(a, Some(da));
        let mut db = PlayableDirector::new("loop-b", 2.0, true, false);
        db.tracks.push(track(
            "ToA",
            TimelineTrackKind::Control,
            "A",
            vec![clip_control(0.0, 2.0)],
        ));
        world.set_director(b, Some(db));
        world.capture_timeline_baselines();
        world.tick_timelines(0.25);
        world.tick_timelines(0.25);
        assert!(
            world.is_active(a),
            "self-control does not deactivate the director"
        );
        assert!(world.is_active(b));
        let ta = world.director(a).unwrap().time;
        let tb = world.director(b).unwrap().time;
        assert!((ta - 0.5).abs() < 1e-3, "A keeps its own clock, got {ta}");
        assert!(
            (tb - 0.5).abs() < 1e-3,
            "B is synced to A's clip and does not also tick, got {tb}"
        );
        assert!(ta.is_finite() && tb.is_finite());
    }

    fn key(t: f32, v: f32, interp: CurveInterp) -> CurveKey {
        CurveKey { t, v, interp }
    }

    #[test]
    fn curve_sample_interp_edges_and_sort() {
        let linear = Curve::from_keys(vec![
            key(0.0, 0.0, CurveInterp::Linear),
            key(2.0, 10.0, CurveInterp::Linear),
        ]);
        assert!((linear.sample(-1.0).unwrap() - 0.0).abs() < 1e-4);
        assert!((linear.sample(0.0).unwrap() - 0.0).abs() < 1e-4);
        assert!((linear.sample(1.0).unwrap() - 5.0).abs() < 1e-4);
        assert!((linear.sample(3.0).unwrap() - 10.0).abs() < 1e-4);

        let constant = Curve::from_keys(vec![
            key(0.0, 1.0, CurveInterp::Constant),
            key(1.0, 9.0, CurveInterp::Linear),
        ]);
        assert!((constant.sample(0.0).unwrap() - 1.0).abs() < 1e-4);
        assert!((constant.sample(0.99).unwrap() - 1.0).abs() < 1e-4);
        assert!((constant.sample(1.0).unwrap() - 9.0).abs() < 1e-4);

        let ease = Curve::from_keys(vec![
            key(0.0, 0.0, CurveInterp::Ease),
            key(2.0, 10.0, CurveInterp::Linear),
        ]);
        let mid = ease.sample(1.0).unwrap();
        assert!(
            (mid - 5.0).abs() < 1e-3,
            "smoothstep midpoint is linear, got {mid}"
        );
        let quarter = ease.sample(0.5).unwrap();
        let expect = 10.0 * (0.25 * 0.25 * (3.0 - 2.0 * 0.25));
        assert!(
            (quarter - expect).abs() < 1e-3,
            "ease at u=0.25 should be {expect}, got {quarter}"
        );
        assert!((quarter - 2.5).abs() > 0.5, "ease is not linear at u=0.25");

        let single = Curve::from_keys(vec![key(0.4, 3.5, CurveInterp::Linear)]);
        assert!((single.sample(0.0).unwrap() - 3.5).abs() < 1e-4);
        assert!((single.sample(9.0).unwrap() - 3.5).abs() < 1e-4);

        assert!(Curve::default().sample(0.0).is_none());

        let unsorted = Curve::from_keys(vec![
            key(2.0, 10.0, CurveInterp::Linear),
            key(0.0, 0.0, CurveInterp::Linear),
        ]);
        assert!((unsorted.keys[0].t - 0.0).abs() < 1e-6);
        assert!((unsorted.sample(1.0).unwrap() - 5.0).abs() < 1e-3);
    }

    #[test]
    fn transform_curve_puts_entity_at_expected_xy() {
        let mut world = World::new();
        let cam = world.spawn_named("MainCamera", Transform::from_xy(10.0, 10.0));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut clip = clip_xform(0.0, 2.0, [0.0, 0.0], [10.0, 100.0]);
        clip.curve_x = Some(Curve::from_keys(vec![
            key(0.0, 0.0, CurveInterp::Linear),
            key(2.0, 10.0, CurveInterp::Linear),
        ]));
        clip.curve_y = Some(Curve::from_keys(vec![
            key(0.0, 0.0, CurveInterp::Ease),
            key(2.0, 10.0, CurveInterp::Linear),
        ]));
        let mut d = PlayableDirector::new("intro", 4.0, true, false);
        d.tracks.push(track(
            "Slide",
            TimelineTrackKind::Transform,
            "MainCamera",
            vec![clip],
        ));
        world.set_director(dir, Some(d));
        world.capture_timeline_baselines();
        world.set_timeline_time(dir, 0.5);
        let xf = world.transform(cam).unwrap();
        assert!(
            (xf.translation.x - 2.5).abs() < 1e-3,
            "linear x, got {}",
            xf.translation.x
        );
        let y_ease = 10.0 * (0.25 * 0.25 * (3.0 - 2.0 * 0.25));
        assert!(
            (xf.translation.y - y_ease).abs() < 1e-3,
            "ease y, got {} want {y_ease}",
            xf.translation.y
        );
        world.set_timeline_time(dir, 3.0);
        let xf = world.transform(cam).unwrap();
        assert!((xf.translation.x - 10.0).abs() < 1e-3, "hold last x key");
        assert!((xf.translation.y - 10.0).abs() < 1e-3, "hold last y key");
        world.stop_timeline(dir);
        let xf = world.transform(cam).unwrap();
        assert!((xf.translation.x - 0.0).abs() < 1e-3);
        assert!((xf.translation.y - 0.0).abs() < 1e-3);
    }

    #[test]
    fn transform_curve_on_one_axis_keeps_from_to_on_the_other() {
        let mut world = World::new();
        let cam = world.spawn_named("MainCamera", Transform::from_xy(1.0, 1.0));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut clip = clip_xform(0.0, 2.0, [0.0, 0.0], [10.0, 80.0]);
        clip.curve_x = Some(Curve::from_keys(vec![
            key(0.0, 4.0, CurveInterp::Constant),
            key(2.0, 6.0, CurveInterp::Linear),
        ]));
        let mut d = PlayableDirector::new("intro", 4.0, true, false);
        d.tracks.push(track(
            "Slide",
            TimelineTrackKind::Transform,
            "MainCamera",
            vec![clip],
        ));
        world.set_director(dir, Some(d));
        world.set_timeline_time(dir, 1.0);
        let xf = world.transform(cam).unwrap();
        assert!(
            (xf.translation.x - 4.0).abs() < 1e-3,
            "constant x until t=2"
        );
        assert!((xf.translation.y - 40.0).abs() < 1e-3, "y still from→to");
    }

    #[test]
    fn legacy_from_to_without_curves_matches_lerp() {
        let mut world = World::new();
        let cam = world.spawn_named("MainCamera", Transform::from_xy(10.0, 10.0));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut d = PlayableDirector::new("intro", 4.0, true, false);
        d.tracks.push(track(
            "CamSlide",
            TimelineTrackKind::Transform,
            "MainCamera",
            vec![clip_xform(0.0, 2.0, [320.0, 240.0], [400.0, 240.0])],
        ));
        world.set_director(dir, Some(d));
        world.set_timeline_time(dir, 1.0);
        let xf = world.transform(cam).unwrap();
        assert!((xf.translation.x - 360.0).abs() < 1e-3);
        assert!((xf.translation.y - 240.0).abs() < 1e-3);
        world.set_timeline_time(dir, 3.0);
        let xf = world.transform(cam).unwrap();
        assert!((xf.translation.x - 400.0).abs() < 1e-3);
    }

    #[test]
    fn float_curve_drives_rotation_scale_and_sprite_alpha() {
        use crate::draw::TextureId;
        use crate::math::Vec2;
        use crate::world::Sprite;

        let mut world = World::new();
        let id = world.spawn_named("Orb", Transform::from_xy(0.0, 0.0));
        world.set_sprite(id, Some(Sprite::new(TextureId(0), Vec2::new(16.0, 16.0))));
        let dir = world.spawn_named("Cutscene", Transform::default());
        let mut rot = clip_base(0.0, 2.0, true, "", Vec::new(), "", [0.0, 0.0], [0.0, 0.0]);
        rot.curve_value = Some(Curve::from_keys(vec![
            key(0.0, 0.0, CurveInterp::Linear),
            key(2.0, 90.0, CurveInterp::Linear),
        ]));
        let mut sx = clip_base(0.0, 2.0, true, "", Vec::new(), "", [0.0, 0.0], [0.0, 0.0]);
        sx.curve_value = Some(Curve::from_keys(vec![
            key(0.0, 1.0, CurveInterp::Linear),
            key(2.0, 3.0, CurveInterp::Linear),
        ]));
        let mut alpha = clip_base(0.0, 2.0, true, "", Vec::new(), "", [0.0, 0.0], [0.0, 0.0]);
        alpha.curve_value = Some(Curve::from_keys(vec![
            key(0.0, 1.0, CurveInterp::Ease),
            key(2.0, 0.0, CurveInterp::Linear),
        ]));
        let mut d = PlayableDirector::new("intro", 4.0, true, false);
        d.tracks.push(TimelineTrackRuntime {
            name: "Spin".into(),
            kind: TimelineTrackKind::Float,
            binding: "Orb".into(),
            property: "Transform.rotation".into(),
            clips: vec![rot],
        });
        d.tracks.push(TimelineTrackRuntime {
            name: "Grow".into(),
            kind: TimelineTrackKind::Float,
            binding: "Orb".into(),
            property: "Transform.scale_x".into(),
            clips: vec![sx],
        });
        d.tracks.push(TimelineTrackRuntime {
            name: "Fade".into(),
            kind: TimelineTrackKind::Float,
            binding: "Orb".into(),
            property: "Sprite.alpha".into(),
            clips: vec![alpha],
        });
        world.set_director(dir, Some(d));
        world.set_timeline_time(dir, 1.0);
        let xf = world.transform(id).unwrap();
        let (z, _, _) = xf.rotation.to_euler(glam::EulerRot::ZYX);
        assert!(
            (z.to_degrees() - 45.0).abs() < 0.5,
            "rotation degrees, got {}",
            z.to_degrees()
        );
        assert!((xf.scale.x - 2.0).abs() < 1e-3, "scale_x");
        let a = world.sprite(id).unwrap().color.a;
        let u = 0.5_f32;
        let s = u * u * (3.0 - 2.0 * u);
        let expect = ((1.0 + (0.0 - 1.0) * s) * 255.0).round() as u8;
        assert_eq!(a, expect, "eased alpha");
        world.set_timeline_time(dir, 3.0);
        let (z, _, _) = world
            .transform(id)
            .unwrap()
            .rotation
            .to_euler(glam::EulerRot::ZYX);
        assert!(
            (z.to_degrees() - 90.0).abs() < 0.5,
            "hold last rotation key"
        );
        assert_eq!(world.sprite(id).unwrap().color.a, 0);
    }
}
