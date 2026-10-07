//! Timeline / cutscene sidecars (`assets/<name>.timeline.json`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Track kind. Signal and Control were added after Activation / Animation / Audio / Transform;
/// older `*.timeline.json` files omit those variants and still deserialize.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineTrackKind {
    Activation,
    Animation,
    Audio,
    Transform,
    /// Instant markers (`start` is the fire time; `end` is usually equal).
    Signal,
    /// Clips that activate a target and drive its PlayableDirector.
    Control,
    /// One authored float (`property` on the track, keys in `curves.value`).
    Float,
}

impl TimelineTrackKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "activation" => Some(Self::Activation),
            "animation" => Some(Self::Animation),
            "audio" => Some(Self::Audio),
            "transform" => Some(Self::Transform),
            "signal" => Some(Self::Signal),
            "control" => Some(Self::Control),
            "float" => Some(Self::Float),
            _ => None,
        }
    }
}

/// Blend from this key to the next. Unknown strings still load so doctor can warn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CurveInterp {
    #[default]
    Linear,
    Constant,
    Ease,
    Unknown(String),
}

impl CurveInterp {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "linear" => Self::Linear,
            "constant" => Self::Constant,
            "ease" => Self::Ease,
            "" => Self::Linear,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Linear => "linear",
            Self::Constant => "constant",
            Self::Ease => "ease",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    pub fn to_core(&self) -> wiimaker_core::CurveInterp {
        match self {
            Self::Constant => wiimaker_core::CurveInterp::Constant,
            Self::Ease => wiimaker_core::CurveInterp::Ease,
            Self::Linear | Self::Unknown(_) => wiimaker_core::CurveInterp::Linear,
        }
    }
}

impl Serialize for CurveInterp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CurveInterp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::parse(&s))
    }
}

/// One key. `t` is clip-local seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveKey {
    pub t: f32,
    pub v: f32,
    #[serde(default)]
    pub interp: CurveInterp,
}

/// Optional curves on a clip. Missing axes keep the Transform `from`→`to` lerp.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimelineCurves {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Vec<CurveKey>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Vec<CurveKey>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<CurveKey>>,
}

/// Float properties the runtime actually writes.
pub fn known_float_property(name: &str) -> bool {
    matches!(
        name,
        "Transform.rotation" | "Transform.scale_x" | "Transform.scale_y" | "Sprite.alpha"
    )
}

pub fn float_property_names() -> &'static [&'static str] {
    &[
        "Transform.rotation",
        "Transform.scale_x",
        "Transform.scale_y",
        "Sprite.alpha",
    ]
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
    /// Signal marker name. Fires when the playhead crosses [`Self::start`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    /// Optional string sent with a signal marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Keyframed curves. Omitted on older timelines (those keep `from`→`to`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curves: Option<TimelineCurves>,
}

impl Default for TimelineClip {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
            active: None,
            clip: None,
            audio: None,
            volume: None,
            from: None,
            to: None,
            signal: None,
            payload: None,
            curves: None,
        }
    }
}

impl TimelineClip {
    pub fn activation(start: f32, end: f32, active: bool) -> Self {
        Self {
            start,
            end,
            active: Some(active),
            ..Self::default()
        }
    }

    pub fn animation(start: f32, end: f32, clip: impl Into<String>) -> Self {
        Self {
            start,
            end,
            clip: Some(clip.into()),
            ..Self::default()
        }
    }

    pub fn audio(start: f32, end: f32, audio: impl Into<String>, volume: f32) -> Self {
        Self {
            start,
            end,
            audio: Some(audio.into()),
            volume: Some(volume),
            ..Self::default()
        }
    }

    pub fn transform(start: f32, end: f32, from: [f32; 2], to: [f32; 2]) -> Self {
        Self {
            start,
            end,
            from: Some(from),
            to: Some(to),
            ..Self::default()
        }
    }

    /// Instant signal marker. `time` is stored as both `start` and `end`.
    pub fn signal(time: f32, name: impl Into<String>, payload: Option<&str>) -> Self {
        let payload = payload.map(str::trim).filter(|s| !s.is_empty());
        Self {
            start: time,
            end: time,
            signal: Some(name.into()),
            payload: payload.map(str::to_string),
            ..Self::default()
        }
    }

    /// Control clip. The track binding is the target entity.
    pub fn control(start: f32, end: f32) -> Self {
        Self {
            start,
            end,
            ..Self::default()
        }
    }

    pub fn span(&self) -> f32 {
        (self.end - self.start).max(0.0)
    }
}

/// One authored track.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineTrack {
    pub name: String,
    pub kind: TimelineTrackKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    /// Float track property (`Transform.rotation`, `Transform.scale_x`, `Transform.scale_y`, `Sprite.alpha`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
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
        let mut meta: Self =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        meta.normalize_curves();
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

    /// Sort curve keys by time. Empty arrays stay so doctor can warn.
    pub fn normalize_curves(&mut self) {
        for track in &mut self.tracks {
            for clip in &mut track.clips {
                if let Some(curves) = clip.curves.as_mut() {
                    if let Some(keys) = curves.x.as_mut() {
                        sort_curve_keys(keys);
                    }
                    if let Some(keys) = curves.y.as_mut() {
                        sort_curve_keys(keys);
                    }
                    if let Some(keys) = curves.value.as_mut() {
                        sort_curve_keys(keys);
                    }
                }
            }
        }
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
                    | TimelineTrackKind::Control
                    | TimelineTrackKind::Float
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
                    TimelineTrackKind::Signal => {
                        if clip.signal.as_deref().unwrap_or("").trim().is_empty() {
                            bail!("timeline track '{}' signal marker missing name", track.name);
                        }
                    }
                    TimelineTrackKind::Activation
                    | TimelineTrackKind::Control
                    | TimelineTrackKind::Float => {}
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn sort_curve_keys(keys: &mut [CurveKey]) {
    keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
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
                    property: None,
                    clips: vec![TimelineClip::activation(0.5, 4.0, true)],
                },
                TimelineTrack {
                    name: "PlayerMove".into(),
                    kind: TimelineTrackKind::Animation,
                    binding: Some("Player".into()),
                    property: None,
                    clips: vec![TimelineClip::animation(0.0, 2.0, "chomp")],
                },
                TimelineTrack {
                    name: "Stinger".into(),
                    kind: TimelineTrackKind::Audio,
                    binding: None,
                    property: None,
                    clips: vec![TimelineClip::audio(0.0, 0.5, "beep", 1.0)],
                },
                TimelineTrack {
                    name: "CamSlide".into(),
                    kind: TimelineTrackKind::Transform,
                    binding: Some("MainCamera".into()),
                    property: None,
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

    #[test]
    fn old_timeline_json_without_signal_or_control_still_loads() {
        let old = r#"{
            "duration": 1.5,
            "tracks": [{
                "name": "GhostAppear",
                "kind": "Activation",
                "binding": "IntroGhost",
                "clips": [{ "start": 0.5, "end": 1.5, "active": true }]
            }]
        }"#;
        let meta: TimelineMeta = serde_json::from_str(old).unwrap();
        meta.validate().unwrap();
        assert_eq!(meta.tracks[0].kind, TimelineTrackKind::Activation);
        assert!(meta.tracks[0].clips[0].signal.is_none());
        assert!(meta.tracks[0].clips[0].payload.is_none());
    }

    #[test]
    fn signal_and_control_roundtrip() {
        let dir = env::temp_dir().join(format!("wiimaker-timeline-sc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let meta = TimelineMeta {
            duration: 3.0,
            tracks: vec![
                TimelineTrack {
                    name: "Cues".into(),
                    kind: TimelineTrackKind::Signal,
                    binding: Some("Player".into()),
                    property: None,
                    clips: vec![TimelineClip::signal(1.25, "IntroDone", Some("go"))],
                },
                TimelineTrack {
                    name: "Sub".into(),
                    kind: TimelineTrackKind::Control,
                    binding: Some("Child".into()),
                    property: None,
                    clips: vec![TimelineClip::control(0.5, 2.0)],
                },
            ],
        };
        let (path, _) = write_timeline(&dir, "intro", meta).unwrap();
        let loaded = TimelineMeta::load(&path).unwrap();
        assert_eq!(loaded.tracks[0].kind, TimelineTrackKind::Signal);
        assert_eq!(
            loaded.tracks[0].clips[0].signal.as_deref(),
            Some("IntroDone")
        );
        assert_eq!(loaded.tracks[0].clips[0].payload.as_deref(), Some("go"));
        assert!((loaded.tracks[0].clips[0].start - 1.25).abs() < 1e-4);
        assert_eq!(loaded.tracks[1].kind, TimelineTrackKind::Control);
        assert_eq!(loaded.tracks[1].binding.as_deref(), Some("Child"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("\"audio\""),
            "unused clip fields stay omitted"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsorted_curve_keys_sort_on_load_and_old_files_omit_curves() {
        let raw = r#"{
            "duration": 2.0,
            "tracks": [{
                "name": "Slide",
                "kind": "Transform",
                "binding": "Cam",
                "clips": [{
                    "start": 0.0,
                    "end": 2.0,
                    "from": [0.0, 0.0],
                    "to": [10.0, 0.0],
                    "curves": { "x": [
                        {"t": 2.0, "v": 10.0, "interp": "linear"},
                        {"t": 0.0, "v": 0.0, "interp": "ease"}
                    ]}
                }]
            }]
        }"#;
        let meta: TimelineMeta = serde_json::from_str(raw).unwrap();
        let mut meta = meta;
        meta.normalize_curves();
        meta.validate().unwrap();
        let keys = meta.tracks[0].clips[0]
            .curves
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap();
        assert!((keys[0].t - 0.0).abs() < 1e-6);
        assert_eq!(keys[0].interp, CurveInterp::Ease);
        assert!(meta.tracks[0].clips[0].curves.as_ref().unwrap().y.is_none());

        let old = r#"{
            "duration": 1.0,
            "tracks": [{
                "name": "Slide",
                "kind": "Transform",
                "binding": "Cam",
                "clips": [{ "start": 0.0, "end": 1.0, "from": [0.0, 0.0], "to": [1.0, 0.0] }]
            }]
        }"#;
        let old: TimelineMeta = serde_json::from_str(old).unwrap();
        assert!(old.tracks[0].clips[0].curves.is_none());
        assert!(old.tracks[0].property.is_none());
    }
}
