//! Desktop oneshot playback. No Wii GX/ASND.
//!
//! PCM16 WAV is validated, then `aplay`/`paplay`/`pw-play` is spawned when
//! present. (`rodio`/`cpal` could not be added on Cargo 1.83 without pulling
//! edition-2024 crates.) Missing clips error. `WIIMAKER_AUDIO=0` skips.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use wiimaker_assets::{inspect_wav, resolve_wav, spawn_wav_player};
use wiimaker_core::world::World;

/// Set `WIIMAKER_AUDIO=0` to skip launching a player (CI / headless).
pub fn audio_device_enabled() -> bool {
    match std::env::var("WIIMAKER_AUDIO") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0" || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("false"))
        }
        Err(_) => true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayOutcome {
    Played,
    SkippedNoDevice,
}

/// Result of draining a World's oneshot queue on the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OneshotDrain {
    pub drained: usize,
    pub played: usize,
    pub skipped: usize,
    /// True when the queue was dropped and no player was started.
    pub muted: bool,
    pub errors: Vec<String>,
}

/// While alive, host playback checks see `WIIMAKER_AUDIO=0`.
///
/// The editor uses this around in-editor Play so a game `cdylib` (its own
/// copy of [`audio_device_enabled`]) skips oneshots. Process-wide env is the
/// shared switch; statics are not shared across the plugin. Restores the
/// previous value on drop. Wii ASND does not read this.
pub struct SuppressAudio {
    prev: Option<String>,
    active: bool,
}

impl SuppressAudio {
    pub fn when(mute: bool) -> Self {
        if !mute {
            return Self {
                prev: None,
                active: false,
            };
        }
        let prev = std::env::var("WIIMAKER_AUDIO").ok();
        std::env::set_var("WIIMAKER_AUDIO", "0");
        Self { prev, active: true }
    }
}

impl Drop for SuppressAudio {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.prev.take() {
            Some(v) => std::env::set_var("WIIMAKER_AUDIO", v),
            None => std::env::remove_var("WIIMAKER_AUDIO"),
        }
    }
}

/// Tracks spawned oneshot player processes.
pub struct HostAudio {
    children: Vec<std::process::Child>,
}

impl HostAudio {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn has_device(&self) -> bool {
        audio_device_enabled()
    }

    fn reap(&mut self) {
        self.children
            .retain_mut(|c| !matches!(c.try_wait(), Ok(Some(_))));
    }

    pub fn play_wav_path(&mut self, path: &Path, _volume: f32) -> Result<PlayOutcome> {
        inspect_wav(path)?;
        if !audio_device_enabled() {
            return Ok(PlayOutcome::SkippedNoDevice);
        }
        self.reap();
        match spawn_wav_player(path)? {
            Some(child) => {
                self.children.push(child);
                Ok(PlayOutcome::Played)
            }
            None => Ok(PlayOutcome::SkippedNoDevice),
        }
    }

    pub fn play_clip(
        &mut self,
        assets_dir: &Path,
        clip: &str,
        volume: f32,
    ) -> Result<(PlayOutcome, std::path::PathBuf)> {
        let path = resolve_wav(assets_dir, clip)?;
        let outcome = self.play_wav_path(&path, volume)?;
        Ok((outcome, path))
    }

    pub fn play_world(&mut self, world: &mut World, assets_dir: &Path) -> Vec<String> {
        self.consume_oneshots(world, assets_dir, false).errors
    }

    /// Drain the oneshot queue. `mute` drops every clip without launching a player
    /// (host editor Play). World queue behavior is unchanged — this only consumes it.
    pub fn consume_oneshots(
        &mut self,
        world: &mut World,
        assets_dir: &Path,
        mute: bool,
    ) -> OneshotDrain {
        let queued = world.drain_oneshots();
        let drained = queued.len();
        if mute {
            return OneshotDrain {
                drained,
                played: 0,
                skipped: 0,
                muted: true,
                errors: Vec::new(),
            };
        }
        let mut errors = Vec::new();
        let mut played = 0usize;
        let mut skipped = 0usize;
        for shot in queued {
            if shot.clip.is_empty() {
                continue;
            }
            match self.play_clip(assets_dir, &shot.clip, shot.volume) {
                Ok((PlayOutcome::Played, _)) => played += 1,
                Ok((PlayOutcome::SkippedNoDevice, _)) => skipped += 1,
                Err(e) => errors.push(e.to_string()),
            }
        }
        OneshotDrain {
            drained,
            played,
            skipped,
            muted: false,
            errors,
        }
    }

    pub fn active_players(&self) -> usize {
        self.children.len()
    }

    pub fn sleep_clip(path: &Path, cap: Duration) {
        if let Ok(info) = inspect_wav(path) {
            let secs = info.duration_secs().min(cap.as_secs_f32()).max(0.0);
            if secs > 0.0 {
                std::thread::sleep(Duration::from_secs_f32(secs + 0.05));
            }
        }
    }
}

impl Default for HostAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HostAudio {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.wait();
        }
    }
}

pub fn play_oneshot_file(
    assets_dir: &Path,
    clip: &str,
    volume: f32,
    wait: bool,
) -> Result<(PlayOutcome, std::path::PathBuf)> {
    let mut audio = HostAudio::new();
    let (outcome, path) = audio.play_clip(assets_dir, clip, volume)?;
    if wait && outcome == PlayOutcome::Played {
        HostAudio::sleep_clip(&path, Duration::from_secs(5));
    }
    Ok((outcome, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_assets::write_beep_wav;

    #[test]
    fn missing_clip_is_error() {
        let dir =
            std::env::temp_dir().join(format!("wiimaker-host-audio-miss-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("WIIMAKER_AUDIO", "0");
        let mut audio = HostAudio::new();
        let err = audio.play_clip(&dir, "nope", 1.0).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing") || msg.contains("not found"),
            "{msg}"
        );
    }

    #[test]
    fn present_clip_skips_when_disabled() {
        let dir =
            std::env::temp_dir().join(format!("wiimaker-host-audio-ok-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        write_beep_wav(dir.join("beep.wav"), 0.05).unwrap();
        std::env::set_var("WIIMAKER_AUDIO", "0");
        let mut audio = HostAudio::new();
        let (out, path) = audio.play_clip(&dir, "beep", 0.8).unwrap();
        assert_eq!(out, PlayOutcome::SkippedNoDevice);
        assert!(path.ends_with("beep.wav"));
    }

    #[test]
    fn mute_drops_oneshots_and_play_on_awake_without_a_player() {
        let dir =
            std::env::temp_dir().join(format!("wiimaker-host-audio-mute-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        write_beep_wav(dir.join("beep.wav"), 0.05).unwrap();

        let mut world = World::new();
        let id = world.spawn_named("Beep", wiimaker_core::world::Transform::from_xy(0.0, 0.0));
        world.set_audio_source(id, Some(wiimaker_core::AudioSource::new("beep", 1.0, true)));
        world.queue_awake_audio();
        world.play_oneshot("beep", 0.4);

        let mut audio = HostAudio::new();
        let report = audio.consume_oneshots(&mut world, &dir, true);
        assert!(report.muted);
        assert_eq!(report.drained, 2, "play-on-awake plus the extra oneshot");
        assert_eq!(report.played, 0);
        assert!(report.errors.is_empty());
        assert_eq!(audio.active_players(), 0);
        assert!(world.drain_oneshots().is_empty());

        world.play_oneshot("beep", 1.0);
        let live = audio.consume_oneshots(&mut world, &dir, false);
        assert!(!live.muted);
        assert_eq!(live.drained, 1);
        assert!(live.played + live.skipped == 1 || !live.errors.is_empty());
        assert_eq!(audio.active_players(), if live.played == 1 { 1 } else { 0 });
    }
}
