//! Timeline / cutscene sidecars (`assets/<name>.timeline.json`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Track kind for v1 timelines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineTrackKind {
    Activation,
    Animation,
    Audio,
    Transform,
}

impl TimelineTrackKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "activation" => Some(Self::Activation),
            "animation" => Some(Self::Animation),
            "audio" => Some(Self::Audio),
            "transform" => Some(Self::Transform),
            _ => None,
        }
    }
}

/// One clip on a timeline track. Unused fields stay omitted in JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineClip {
    pub start: f32,
    pub end: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[f32; 2]>,
}

impl TimelineClip {
    pub fn activation(start: f32, end: f32, active: bool) -> Self {
        Self {
            start,
            end,
            active: Some(active),
            clip: None,
            audio: None,
            volume: None,
            from: None,
            to: None,
        }
    }

    pub fn animation(start: f32, end: f32, clip: impl Into<String>) -> Self {
        Self {
            start,
            end,
            active: None,
            clip: Some(clip.into()),
            audio: None,
            volume: None,
            from: None,
            to: None,
        }
    }

    pub fn audio(start: f32, end: f32, audio: impl Into<String>, volume: f32) -> Self {
        Self {
            start,
            end,
            active: None,
            clip: None,
            audio: Some(audio.into()),
            volume: Some(volume),
            from: None,
            to: None,
        }
    }

    pub fn transform(start: f32, end: f32, from: [f32; 2], to: [f32; 2]) -> Self {
        Self {
            start,
            end,
            active: None,
            clip: None,
            audio: None,
            volume: None,
            from: Some(from),
            to: Some(to),
        }
    }
}

/// One authored track.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineTrack {
    pub name: String,
    pub kind: TimelineTrackKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(default)]
    pub clips: Vec<TimelineClip>,
}

/// Authoring timeline (`assets/<name>.timeline.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineMeta {
    pub duration: f32,
    #[serde(default)]
    pub tracks: Vec<TimelineTrack>,
}

impl TimelineMeta {
    pub fn path(assets_dir: impl AsRef<Path>, name: &str) -> PathBuf {
        assets_dir.as_ref().join(format!("{name}.timeline.json"))
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

    pub fn validate(&self) -> Result<()> {
        if self.duration < 0.0 {
            bail!("timeline duration must be >= 0");
        }
        for track in &self.tracks {
            if track.name.trim().is_empty() {
                bail!("timeline track name must not be empty");
            }
            let needs_binding = matches!(
                track.kind,
                TimelineTrackKind::Activation
                    | TimelineTrackKind::Animation
                    | TimelineTrackKind::Transform
            );
            if needs_binding
                && track
                    .binding
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            {
                bail!(
                    "timeline track '{}' ({:?}) needs a binding",
                    track.name,
                    track.kind
                );
            }
            for clip in &track.clips {
                if clip.end < clip.start {
                    bail!(
                        "timeline track '{}' clip end {} is before start {}",
                        track.name,
                        clip.end,
                        clip.start
                    );
                }
                match track.kind {
                    TimelineTrackKind::Animation => {
                        if clip.clip.as_deref().unwrap_or("").trim().is_empty() {
                            bail!(
                                "timeline track '{}' animation clip missing stem",
                                track.name
                            );
                        }
                    }
                    TimelineTrackKind::Audio => {
                        if clip.audio.as_deref().unwrap_or("").trim().is_empty() {
                            bail!("timeline track '{}' audio clip missing stem", track.name);
                        }
                    }
                    TimelineTrackKind::Transform => {
                        if clip.from.is_none() || clip.to.is_none() {
                            bail!(
                                "timeline track '{}' transform clip needs from and to",
                                track.name
                            );
                        }
                    }
                    TimelineTrackKind::Activation => {}
                }
            }
        }
        Ok(())
    }
}

/// Write / overwrite `assets/<name>.timeline.json`.
pub fn write_timeline(
    assets_dir: &Path,
    name: &str,
    meta: TimelineMeta,
) -> Result<(PathBuf, TimelineMeta)> {
    if name.is_empty() {
        bail!("timeline name must not be empty");
    }
    meta.validate()?;
    let path = TimelineMeta::path(assets_dir, name);
    meta.save(&path)?;
    Ok((path, meta))
}

/// Stem names of every `*.timeline.json` under `assets_dir`.
pub fn list_timelines(assets_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !assets_dir.is_dir() {
        return Ok(names);
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(stem) = fname.strip_suffix(".timeline.json") {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Lookup table of timelines by stem.
#[derive(Clone, Debug, Default)]
pub struct TimelineCatalog {
    by_name: HashMap<String, TimelineMeta>,
    names: Vec<String>,
}

impl TimelineCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn lookup(&self, name: &str) -> Option<&TimelineMeta> {
        self.by_name.get(name)
    }

    pub fn load_dir(assets_dir: &Path) -> Result<Self> {
        let mut cat = Self::empty();
        if !assets_dir.is_dir() {
            return Ok(cat);
        }
        for name in list_timelines(assets_dir)? {
            let path = TimelineMeta::path(assets_dir, &name);
            let meta = TimelineMeta::load(&path)?;
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

    fn sample() -> TimelineMeta {
        TimelineMeta {
            duration: 4.0,
            tracks: vec![
                TimelineTrack {
                    name: "GhostAppear".into(),
                    kind: TimelineTrackKind::Activation,
                    binding: Some("IntroGhost".into()),
                    clips: vec![TimelineClip::activation(0.5, 4.0, true)],
                },
                TimelineTrack {
                    name: "PlayerMove".into(),
                    kind: TimelineTrackKind::Animation,
                    binding: Some("Player".into()),
                    clips: vec![TimelineClip::animation(0.0, 2.0, "chomp")],
                },
                TimelineTrack {
                    name: "Stinger".into(),
                    kind: TimelineTrackKind::Audio,
                    binding: None,
                    clips: vec![TimelineClip::audio(0.0, 0.5, "beep", 1.0)],
                },
                TimelineTrack {
                    name: "CamSlide".into(),
                    kind: TimelineTrackKind::Transform,
                    binding: Some("MainCamera".into()),
                    clips: vec![TimelineClip::transform(
                        0.0,
                        2.0,
                        [320.0, 240.0],
                        [400.0, 240.0],
                    )],
                },
            ],
        }
    }

    #[test]
    fn roundtrip_timeline_json() {
        let dir = env::temp_dir().join(format!("wiimaker-timeline-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (path, _) = write_timeline(&dir, "intro", sample()).unwrap();
        assert!(path.ends_with("intro.timeline.json"));
        let loaded = TimelineMeta::load(&path).unwrap();
        assert!((loaded.duration - 4.0).abs() < 1e-4);
        assert_eq!(loaded.tracks.len(), 4);
        assert_eq!(loaded.tracks[0].kind, TimelineTrackKind::Activation);
        assert_eq!(loaded.tracks[2].binding, None);
        assert_eq!(loaded.tracks[3].clips[0].from, Some([320.0, 240.0]));
        let names = list_timelines(&dir).unwrap();
        assert_eq!(names, vec!["intro"]);
        let cat = TimelineCatalog::load_dir(&dir).unwrap();
        assert!(cat.lookup("intro").is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
