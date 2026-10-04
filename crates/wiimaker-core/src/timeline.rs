//! Runtime PlayableDirector (Unity Timeline analogue). Host/editor only — not WSCN.

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

/// Track kind. Mirrors `assets/<name>.timeline.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineTrackKind {
    Activation,
    Animation,
    Audio,
    Transform,
}

/// One clip baked onto a runtime track.
#[derive(Clone, Debug)]
pub struct TimelineClipRuntime {
    pub start: f32,
    pub end: f32,
    /// Activation: desired active flag while inside the clip.
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
        self.apply_director(id, -1.0, t, false);
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
        self.apply_director(id, -1.0, 0.0, false);
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
        self.apply_director(id, prev, t, true);
        true
    }

    /// Advance playing directors and apply every track.
    pub fn tick_timelines(&mut self, dt: f32) {
        let ids: Vec<_> = self.iter_entities().collect();
        for id in ids {
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
                self.apply_director(id, prev, 0.0, true);
                continue;
            }
            let t = prev + dt.max(0.0);
            if t >= duration {
                if loop_ {
                    self.apply_director(id, prev, duration, true);
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
                    self.apply_director(id, -1.0, wrapped, true);
                } else {
                    if let Some(d) = self.director_mut(id) {
                        d.time = duration;
                        d.playing = false;
                        d.finished = true;
                    }
                    self.apply_director(id, prev, duration, true);
                }
            } else {
                if let Some(d) = self.director_mut(id) {
                    d.time = t;
                }
                self.apply_director(id, prev, t, true);
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
        }
        if let Some(d) = self.director_mut(id) {
            if !d.baselines_ready {
                d.xy_base = xy;
                d.anim_base = anims;
                d.baselines_ready = true;
            }
        }
    }

    fn apply_director(&mut self, id: EntityId, prev: f32, time: f32, fire_audio: bool) {
        let Some(tracks) = self.director(id).map(|d| d.tracks.clone()) else {
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

        self.apply_activation(&tracks, time);
        self.apply_transform(&tracks, time, &xy_base);
        self.apply_animation(id, &tracks, time, &anim_base);
        if fire_audio {
            self.apply_audio(id, &tracks, prev, time);
        }
    }

    fn apply_activation(&mut self, tracks: &[TimelineTrackRuntime], time: f32) {
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
            for track in tracks {
                if track.kind != TimelineTrackKind::Activation || track.binding != name {
                    continue;
                }
                for clip in &track.clips {
                    if clip.contains(time) {
                        inside = Some(clip.active);
                    }
                }
            }
            if let Some(eid) = self.find_by_name(&name) {
                self.set_active(eid, inside.unwrap_or(false));
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
                        let t = clip.local_t(time);
                        pose = Some([
                            clip.from[0] + (clip.to[0] - clip.from[0]) * t,
                            clip.from[1] + (clip.to[1] - clip.from[1]) * t,
                        ]);
                    } else if time > clip.end && clip.end >= last_end {
                        last_end = clip.end;
                        last_to = Some(clip.to);
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
                let crossed = time >= clip.start
                    && (prev < clip.start || (prev == clip.start && time > prev));
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
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::Transform;

    fn clip_xform(start: f32, end: f32, from: [f32; 2], to: [f32; 2]) -> TimelineClipRuntime {
        TimelineClipRuntime {
            start,
            end,
            active: true,
            clip: String::new(),
            cells: Vec::new(),
            fps: 10.0,
            loop_clip: true,
            audio: String::new(),
            volume: 1.0,
            from,
            to,
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
}
