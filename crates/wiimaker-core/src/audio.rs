//! Host-first audio oneshots (Unity AudioSource analogue).
//!
//! Playback is a backend concern (`wiimaker-host`). This crate only
//! stores authored state and a pending oneshot queue. Wii ASND is not here.

use crate::world::World;

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

/// Authored clip player on an entity (clip stem = `assets/<clip>.wav`).
#[derive(Clone, Debug)]
pub struct AudioSource {
    /// Clip name or `*.wav` stem under the project `assets/` folder.
    pub clip: String,
    /// Linear gain (`0` silent, `1` full).
    pub volume: f32,
    /// Queue a oneshot once after the World is created / Play starts.
    pub play_on_awake: bool,
    /// Runtime: play-on-awake already queued for this World lifetime.
    pub awake_queued: bool,
}

impl AudioSource {
    pub fn new(clip: impl Into<String>, volume: f32, play_on_awake: bool) -> Self {
        Self {
            clip: clip.into(),
            volume: volume.clamp(0.0, 1.0),
            play_on_awake,
            awake_queued: false,
        }
    }
}

/// Pending host playback request (clip stem + volume).
#[derive(Clone, Debug, PartialEq)]
pub struct Oneshot {
    pub clip: String,
    pub volume: f32,
}

impl Oneshot {
    pub fn new(clip: impl Into<String>, volume: f32) -> Self {
        Self {
            clip: clip.into(),
            volume: volume.clamp(0.0, 1.0),
        }
    }
}

/// Queue play-on-awake [`AudioSource`] clips that have not fired yet.
pub fn queue_awake_audio(world: &mut World) {
    let jobs: Vec<(crate::world::EntityId, String, f32)> = world
        .iter_entities()
        .filter_map(|id| {
            let a = world.audio_source(id)?;
            if a.play_on_awake && !a.awake_queued && !a.clip.is_empty() {
                Some((id, a.clip.clone(), a.volume))
            } else {
                None
            }
        })
        .collect();
    for (id, clip, volume) in jobs {
        if let Some(a) = world.audio_source_mut(id) {
            a.awake_queued = true;
        }
        world.play_oneshot(clip, volume);
    }
}

impl World {
    /// See [`queue_awake_audio`].
    pub fn queue_awake_audio(&mut self) {
        queue_awake_audio(self);
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::{Transform, World};

    #[test]
    fn play_oneshot_queues_and_drains() {
        let mut world = World::new();
        world.play_oneshot("beep", 0.5);
        world.play_oneshot("hit", 2.0);
        let q = world.drain_oneshots();
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].clip, "beep");
        assert!((q[0].volume - 0.5).abs() < 1e-6);
        assert!((q[1].volume - 1.0).abs() < 1e-6);
        assert!(world.drain_oneshots().is_empty());
    }

    #[test]
    fn awake_queues_once() {
        let mut world = World::new();
        let id = world.spawn_named("Sfx", Transform::from_xy(0.0, 0.0));
        world.set_audio_source(id, Some(AudioSource::new("beep", 1.0, true)));
        world.queue_awake_audio();
        world.queue_awake_audio();
        let q = world.drain_oneshots();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].clip, "beep");
        assert!(world.audio_source(id).unwrap().awake_queued);
    }

    #[test]
    fn awake_skips_empty_clip() {
        let mut world = World::new();
        let id = world.spawn_named("Sfx", Transform::from_xy(0.0, 0.0));
        world.set_audio_source(id, Some(AudioSource::new("", 1.0, true)));
        world.queue_awake_audio();
        assert!(world.drain_oneshots().is_empty());
    }
}
