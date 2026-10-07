//! Shared timeline curve mutations. Editor and CLI both call these.

use anyhow::{bail, Result};

use super::timeline::{
    sort_curve_keys, CurveInterp, CurveKey, TimelineClip, TimelineCurves, TimelineMeta,
    TimelineTrackKind,
};

/// Which curve array a command addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveProp {
    X,
    Y,
    Value,
}

impl CurveProp {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "value" => Some(Self::Value),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Value => "value",
        }
    }
}

/// Result of a curve edit (keys after the change, sorted).
#[derive(Clone, Debug, PartialEq)]
pub struct CurveEdit {
    pub track: String,
    pub clip: usize,
    pub prop: CurveProp,
    pub index: Option<usize>,
    pub keys: Vec<CurveKey>,
    /// `add-key` replaced a key within `|Δt| <= 1e-4` instead of inserting another.
    pub replaced: bool,
}

/// Two keys at this clip-local separation are the same time.
const KEY_T_MATCH: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSample {
    pub t: f32,
    pub v: f32,
}

fn track_index(meta: &TimelineMeta, track: &str) -> Result<usize> {
    meta.tracks
        .iter()
        .position(|t| t.name == track)
        .ok_or_else(|| anyhow::anyhow!("timeline has no track '{track}'"))
}

fn clip_span(clip: &TimelineClip) -> f32 {
    clip.span()
}

fn ensure_curves(clip: &mut TimelineClip) -> &mut TimelineCurves {
    if clip.curves.is_none() {
        clip.curves = Some(TimelineCurves::default());
    }
    clip.curves.as_mut().unwrap()
}

fn slot<'a>(curves: &'a mut TimelineCurves, prop: CurveProp) -> &'a mut Option<Vec<CurveKey>> {
    match prop {
        CurveProp::X => &mut curves.x,
        CurveProp::Y => &mut curves.y,
        CurveProp::Value => &mut curves.value,
    }
}

fn keys_ref<'a>(curves: &'a TimelineCurves, prop: CurveProp) -> Option<&'a [CurveKey]> {
    match prop {
        CurveProp::X => curves.x.as_deref(),
        CurveProp::Y => curves.y.as_deref(),
        CurveProp::Value => curves.value.as_deref(),
    }
}

fn collapse_empty_curves(clip: &mut TimelineClip) {
    let Some(curves) = clip.curves.as_ref() else {
        return;
    };
    let empty = curves.x.is_none() && curves.y.is_none() && curves.value.is_none();
    if empty {
        clip.curves = None;
    }
}

fn finish(
    meta: &TimelineMeta,
    track_i: usize,
    clip_i: usize,
    prop: CurveProp,
    index: Option<usize>,
) -> CurveEdit {
    let track = &meta.tracks[track_i];
    let keys = track.clips[clip_i]
        .curves
        .as_ref()
        .and_then(|c| keys_ref(c, prop).map(|k| k.to_vec()))
        .unwrap_or_default();
    CurveEdit {
        track: track.name.clone(),
        clip: clip_i,
        prop,
        index,
        keys,
        replaced: false,
    }
}

/// Index of the key closest to `t` when one sits within [`KEY_T_MATCH`].
fn matching_key_index(keys: &[CurveKey], t: f32) -> Option<usize> {
    keys.iter()
        .enumerate()
        .filter(|(_, k)| (k.t - t).abs() <= KEY_T_MATCH)
        .min_by(|(_, a), (_, b)| {
            (a.t - t)
                .abs()
                .partial_cmp(&(b.t - t).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Insert a key. Creates the curve array when it is missing. Keys are re-sorted.
///
/// A key already within `|Δt| <= 1e-4` is not duplicated: its value and interp
/// are replaced and [`CurveEdit::replaced`] is true. The stored time stays the
/// existing key's `t`.
pub fn curve_add_key(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
    t: f32,
    v: f32,
    interp: CurveInterp,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let curves = ensure_curves(clip);
    let list = slot(curves, prop);
    let mut replaced = false;
    match list {
        Some(keys) => {
            if let Some(i) = matching_key_index(keys, t) {
                keys[i].v = v;
                keys[i].interp = interp;
                replaced = true;
            } else {
                keys.push(CurveKey { t, v, interp });
            }
        }
        None => *list = Some(vec![CurveKey { t, v, interp }]),
    }
    let keys = slot(curves, prop).as_mut().unwrap();
    sort_curve_keys(keys);
    let index = if replaced {
        matching_key_index(keys, t).unwrap_or(0)
    } else {
        keys.iter()
            .rposition(|k| (k.t - t).abs() < 1e-5 && (k.v - v).abs() < 1e-5)
            .unwrap_or(0)
    };
    let mut edit = finish(meta, track_i, clip_i, prop, Some(index));
    edit.replaced = replaced;
    Ok(edit)
}

/// Remove by index, or by clip-local time when `index` is `None`.
pub fn curve_remove_key(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
    index: Option<usize>,
    t: Option<f32>,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let curves = clip
        .curves
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no curves"))?;
    let list = slot(curves, prop)
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no {} curve", prop.as_str()))?;
    let idx = if let Some(i) = index {
        if i >= list.len() {
            bail!("curve key index {i} is out of range ({} keys)", list.len());
        }
        i
    } else if let Some(t) = t {
        list.iter()
            .position(|k| (k.t - t).abs() <= 1e-3)
            .ok_or_else(|| anyhow::anyhow!("no key at t={t}"))?
    } else {
        bail!("remove-key needs --index or --t");
    };
    list.remove(idx);
    Ok(finish(meta, track_i, clip_i, prop, None))
}

/// Move key `index` to a new time and value. Returns the index after re-sort.
pub fn curve_move_key(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
    index: usize,
    t: f32,
    v: f32,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let curves = clip
        .curves
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no curves"))?;
    let list = slot(curves, prop)
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no {} curve", prop.as_str()))?;
    if index >= list.len() {
        bail!(
            "curve key index {index} is out of range ({} keys)",
            list.len()
        );
    }
    let interp = list[index].interp.clone();
    list[index].t = t;
    list[index].v = v;
    sort_curve_keys(list);
    let new_index = list
        .iter()
        .position(|k| (k.t - t).abs() < 1e-5 && (k.v - v).abs() < 1e-5 && k.interp == interp)
        .unwrap_or(0);
    Ok(finish(meta, track_i, clip_i, prop, Some(new_index)))
}

pub fn curve_set_interp(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
    index: usize,
    interp: CurveInterp,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let curves = clip
        .curves
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no curves"))?;
    let list = slot(curves, prop)
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("clip {clip_i} has no {} curve", prop.as_str()))?;
    if index >= list.len() {
        bail!(
            "curve key index {index} is out of range ({} keys)",
            list.len()
        );
    }
    list[index].interp = interp;
    Ok(finish(meta, track_i, clip_i, prop, Some(index)))
}

/// Seed a curve from the clip `from`→`to` at local t = 0 and t = span.
pub fn curve_add_curve(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let kind = meta.tracks[track_i].kind;
    match (kind, prop) {
        (TimelineTrackKind::Transform, CurveProp::X | CurveProp::Y) => {}
        (TimelineTrackKind::Float, CurveProp::Value) => {}
        _ => bail!(
            "cannot add {} curve on {:?} track '{track}'",
            prop.as_str(),
            kind
        ),
    }
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let span = clip_span(clip);
    let (v0, v1) = match prop {
        CurveProp::X => (
            clip.from.map(|p| p[0]).unwrap_or(0.0),
            clip.to.map(|p| p[0]).unwrap_or(0.0),
        ),
        CurveProp::Y => (
            clip.from.map(|p| p[1]).unwrap_or(0.0),
            clip.to.map(|p| p[1]).unwrap_or(0.0),
        ),
        CurveProp::Value => (0.0, 0.0),
    };
    let curves = ensure_curves(clip);
    let list = slot(curves, prop);
    if list.as_ref().is_some_and(|k| !k.is_empty()) {
        bail!("clip {clip_i} already has a {} curve", prop.as_str());
    }
    *list = Some(vec![
        CurveKey {
            t: 0.0,
            v: v0,
            interp: CurveInterp::Linear,
        },
        CurveKey {
            t: span,
            v: v1,
            interp: CurveInterp::Linear,
        },
    ]);
    sort_curve_keys(slot(curves, prop).as_mut().unwrap());
    Ok(finish(meta, track_i, clip_i, prop, None))
}

pub fn curve_remove_curve(
    meta: &mut TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
) -> Result<CurveEdit> {
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get_mut(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    if let Some(curves) = clip.curves.as_mut() {
        *slot(curves, prop) = None;
    }
    collapse_empty_curves(clip);
    Ok(finish(meta, track_i, clip_i, prop, None))
}

/// Sample `prop` from clip-local `from` to `to`. `steps` is the number of samples
/// (inclusive endpoints when `steps >= 2`).
pub fn curve_sample(
    meta: &TimelineMeta,
    track: &str,
    clip_i: usize,
    prop: CurveProp,
    from: f32,
    to: f32,
    steps: usize,
) -> Result<Vec<CurveSample>> {
    if steps == 0 {
        bail!("--steps must be >= 1");
    }
    let track_i = track_index(meta, track)?;
    let clip = meta.tracks[track_i]
        .clips
        .get(clip_i)
        .ok_or_else(|| anyhow::anyhow!("track '{track}' has no clip {clip_i}"))?;
    let authored = clip.curves.as_ref().and_then(|c| keys_ref(c, prop));
    let runtime = authored.filter(|k| !k.is_empty()).map(|keys| {
        wiimaker_core::Curve::from_keys(
            keys.iter()
                .map(|k| wiimaker_core::CurveKey {
                    t: k.t,
                    v: k.v,
                    interp: k.interp.to_core(),
                })
                .collect(),
        )
    });
    let span = clip.span().max(1e-6);
    let fallback = |local: f32| -> f32 {
        let u = (local / span).clamp(0.0, 1.0);
        match prop {
            CurveProp::X => {
                let a = clip.from.map(|p| p[0]).unwrap_or(0.0);
                let b = clip.to.map(|p| p[0]).unwrap_or(0.0);
                a + (b - a) * u
            }
            CurveProp::Y => {
                let a = clip.from.map(|p| p[1]).unwrap_or(0.0);
                let b = clip.to.map(|p| p[1]).unwrap_or(0.0);
                a + (b - a) * u
            }
            CurveProp::Value => 0.0,
        }
    };
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = if steps == 1 {
            from
        } else {
            from + (to - from) * (i as f32) / ((steps - 1) as f32)
        };
        let v = runtime
            .as_ref()
            .and_then(|c| c.sample(t))
            .unwrap_or_else(|| fallback(t));
        out.push(CurveSample { t, v });
    }
    Ok(out)
}

/// Parse `x:0,0,linear;1,10,ease|y:0,0,linear` onto a clip.
pub fn apply_curve_suffix(clip: &mut TimelineClip, spec: &str) -> Result<()> {
    for part in spec.split('|') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (prop_s, body) = part.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("curve '{part}' must be prop:t,v[,interp];t,v[,interp]")
        })?;
        let prop = CurveProp::parse(prop_s)
            .ok_or_else(|| anyhow::anyhow!("unknown curve prop '{prop_s}' (x|y|value)"))?;
        let mut keys = Vec::new();
        for item in body.split(';') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let mut bits = item.split(',');
            let t_s = bits
                .next()
                .ok_or_else(|| anyhow::anyhow!("curve key '{item}' needs t,v"))?;
            let v_s = bits
                .next()
                .ok_or_else(|| anyhow::anyhow!("curve key '{item}' needs t,v"))?;
            let t: f32 = t_s
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("curve key t '{t_s}'"))?;
            let v: f32 = v_s
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("curve key v '{v_s}'"))?;
            let interp = match bits.next() {
                Some(s) if !s.trim().is_empty() => CurveInterp::parse(s),
                _ => CurveInterp::Linear,
            };
            keys.push(CurveKey { t, v, interp });
        }
        if keys.is_empty() {
            bail!("curve '{part}' has no keys");
        }
        sort_curve_keys(&mut keys);
        let curves = ensure_curves(clip);
        *slot(curves, prop) = Some(keys);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TimelineClip, TimelineTrack, TimelineTrackKind};

    fn slide() -> TimelineMeta {
        TimelineMeta {
            duration: 3.0,
            tracks: vec![TimelineTrack {
                name: "Slide".into(),
                kind: TimelineTrackKind::Transform,
                binding: Some("Cam".into()),
                property: None,
                clips: vec![TimelineClip::transform(0.0, 2.0, [0.0, 0.0], [10.0, 4.0])],
            }],
        }
    }

    #[test]
    fn add_move_interp_remove_and_sample_ease() {
        let mut meta = slide();
        let seeded = curve_add_curve(&mut meta, "Slide", 0, CurveProp::X).unwrap();
        assert_eq!(seeded.keys.len(), 2);
        assert!((seeded.keys[0].v - 0.0).abs() < 1e-6);
        assert!((seeded.keys[1].v - 10.0).abs() < 1e-6);
        assert!((seeded.keys[1].t - 2.0).abs() < 1e-6);

        let added = curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::X,
            1.0,
            1.0,
            CurveInterp::Ease,
        )
        .unwrap();
        assert_eq!(added.index, Some(1));
        assert_eq!(added.keys[1].interp, CurveInterp::Ease);

        let moved = curve_move_key(&mut meta, "Slide", 0, CurveProp::X, 1, 0.5, 2.0).unwrap();
        assert!(moved
            .keys
            .iter()
            .any(|k| (k.t - 0.5).abs() < 1e-5 && (k.v - 2.0).abs() < 1e-5));

        let idx = moved.index.unwrap();
        curve_set_interp(
            &mut meta,
            "Slide",
            0,
            CurveProp::X,
            idx,
            CurveInterp::Constant,
        )
        .unwrap();
        assert_eq!(
            meta.tracks[0].clips[0]
                .curves
                .as_ref()
                .unwrap()
                .x
                .as_ref()
                .unwrap()[idx]
                .interp,
            CurveInterp::Constant
        );

        curve_remove_key(&mut meta, "Slide", 0, CurveProp::X, Some(idx), None).unwrap();
        let n = meta.tracks[0].clips[0]
            .curves
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .len();
        assert_eq!(n, 2);

        curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::Y,
            0.0,
            0.0,
            CurveInterp::Ease,
        )
        .unwrap();
        curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::Y,
            2.0,
            10.0,
            CurveInterp::Linear,
        )
        .unwrap();
        let samples = curve_sample(&meta, "Slide", 0, CurveProp::Y, 0.0, 2.0, 5).unwrap();
        assert_eq!(samples.len(), 5);
        assert!((samples[0].v - 0.0).abs() < 1e-4);
        assert!((samples[4].v - 10.0).abs() < 1e-4);
        let quarter = samples[1].v;
        let linear = 2.5;
        assert!(
            (quarter - linear).abs() > 0.4,
            "ease sample at t=0.5 should leave the straight line, got {quarter}"
        );

        curve_remove_curve(&mut meta, "Slide", 0, CurveProp::X).unwrap();
        curve_remove_curve(&mut meta, "Slide", 0, CurveProp::Y).unwrap();
        assert!(meta.tracks[0].clips[0].curves.is_none());
    }

    #[test]
    fn sample_without_curve_matches_from_to() {
        let meta = slide();
        let samples = curve_sample(&meta, "Slide", 0, CurveProp::X, 0.0, 2.0, 3).unwrap();
        assert!((samples[0].v - 0.0).abs() < 1e-4);
        assert!((samples[1].v - 5.0).abs() < 1e-4);
        assert!((samples[2].v - 10.0).abs() < 1e-4);
        let y = curve_sample(&meta, "Slide", 0, CurveProp::Y, 1.0, 1.0, 1).unwrap();
        assert!((y[0].v - 2.0).abs() < 1e-4);
    }

    #[test]
    fn add_key_same_t_replaces_value_and_interp() {
        let mut meta = slide();
        let first = curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::X,
            0.0,
            -40.0,
            CurveInterp::Linear,
        )
        .unwrap();
        assert!(!first.replaced);
        assert_eq!(first.keys.len(), 1);
        assert!((first.keys[0].t - 0.0).abs() < 1e-6);

        let replaced = curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::X,
            1e-4,
            12.0,
            CurveInterp::Ease,
        )
        .unwrap();
        assert!(replaced.replaced);
        assert_eq!(replaced.index, Some(0));
        assert_eq!(replaced.keys.len(), 1);
        assert!(
            (replaced.keys[0].t - 0.0).abs() < 1e-6,
            "stored t stays the existing key"
        );
        assert!((replaced.keys[0].v - 12.0).abs() < 1e-6);
        assert_eq!(replaced.keys[0].interp, CurveInterp::Ease);

        let inserted = curve_add_key(
            &mut meta,
            "Slide",
            0,
            CurveProp::X,
            1e-4 + 1e-5,
            3.0,
            CurveInterp::Constant,
        )
        .unwrap();
        assert!(!inserted.replaced);
        assert_eq!(inserted.keys.len(), 2);
        assert!(inserted
            .keys
            .iter()
            .any(|k| (k.t - (1e-4 + 1e-5)).abs() < 1e-6 && (k.v - 3.0).abs() < 1e-6));
    }
}
