//! Trailing WSCN0003 tables for Animator + PlayableDirector (after AudioSource).
//!
//! Layout (only when the scene has ≥1 enabled Animator or PlayableDirector):
//! 1. Audio table first (existing). If there is no AudioSource, write `u16` 0 so
//!    old loaders consume a zero-length audio table and ignore the rest.
//! 2. Sprite cell LUT — names resolved to wpack tex + UV/pivot/size.
//! 3. Controller blobs (params, states, blend trees, transitions, layers).
//! 4. Timeline blobs (tracks, clips, curves; audio stems → wpack index strings).
//! 5. Animator components (entity index, controller index, overrides).
//! 6. PlayableDirector components (entity index, timeline index, flags).
//!
//! Magic stays `WSCN0003`. Scenes without these components omit the extension
//! (and omit the audio count unless AudioSource is present).

use std::io::Write;

use anyhow::{bail, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use wiimaker_assets::{
    is_any_state, AnimClipCatalog, AnimClipMeta, AnimatorControllerCatalog, AnimatorControllerMeta,
    BlendDimension, BlendTreeMeta, ControllerCondition, ControllerParam, ControllerParamType,
    ControllerState, ControllerTransition, SpriteCatalog, TimelineCatalog, TimelineMeta,
    TimelineTrackKind, WPack,
};

use crate::scene::{Scene, SceneAnimator, ScenePlayableDirector};
use crate::wscn::{WSCN_AUDIO_NO_CLIP, WSCN_TILE_NO_TEX};

/// Missing controller / timeline index (same sentinel as audio / tile tex).
pub const WSCN_NO_REF: u16 = 0xFFFF;

pub struct BakeCatalogs<'a> {
    pub pack: &'a WPack,
    pub sprites: Option<&'a SpriteCatalog>,
    pub anims: Option<&'a AnimClipCatalog>,
    pub controllers: Option<&'a AnimatorControllerCatalog>,
    pub timelines: Option<&'a TimelineCatalog>,
}

fn write_str(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = s.as_bytes();
    if bytes.len() > u16::MAX as usize {
        bail!("WSCN string too long ({})", s.len());
    }
    buf.write_u16::<LittleEndian>(bytes.len() as u16)?;
    buf.write_all(bytes)?;
    Ok(())
}

fn scene_has_anim_or_director(scene: &Scene) -> bool {
    scene.entities.iter().any(|ent| {
        ent.components.animator.as_ref().is_some_and(|a| a.enabled)
            || ent
                .components
                .playable_director
                .as_ref()
                .is_some_and(|d| d.enabled)
    })
}

fn unique_push(list: &mut Vec<String>, name: &str) {
    let n = name.trim();
    if n.is_empty() {
        return;
    }
    if !list.iter().any(|s| s == n) {
        list.push(n.to_string());
    }
}

fn clip_meta<'a>(anims: Option<&'a AnimClipCatalog>, stem: &str) -> Option<&'a AnimClipMeta> {
    let stem = stem.trim();
    if stem.is_empty() {
        return None;
    }
    anims.and_then(|c| c.lookup(stem))
}

fn collect_clip_cells(cells: &mut Vec<String>, anims: Option<&AnimClipCatalog>, stem: &str) {
    if let Some(meta) = clip_meta(anims, stem) {
        for cell in &meta.cells {
            unique_push(cells, cell);
        }
    }
}

fn collect_state_cells(
    cells: &mut Vec<String>,
    anims: Option<&AnimClipCatalog>,
    s: &ControllerState,
) {
    collect_clip_cells(cells, anims, &s.clip);
    if let Some(bt) = &s.blend_tree {
        for m in &bt.motions {
            collect_clip_cells(cells, anims, &m.clip);
        }
    }
}

fn collect_controller_cells(
    cells: &mut Vec<String>,
    anims: Option<&AnimClipCatalog>,
    meta: &AnimatorControllerMeta,
) {
    for s in &meta.states {
        collect_state_cells(cells, anims, s);
    }
    for layer in &meta.layers {
        for s in &layer.states {
            collect_state_cells(cells, anims, s);
        }
    }
}

fn collect_timeline_cells(
    cells: &mut Vec<String>,
    anims: Option<&AnimClipCatalog>,
    meta: &TimelineMeta,
) {
    for track in &meta.tracks {
        for clip in &track.clips {
            if let Some(stem) = clip.clip.as_deref() {
                collect_clip_cells(cells, anims, stem);
            }
        }
    }
}

fn referenced_controllers(scene: &Scene) -> Vec<String> {
    let mut names = Vec::new();
    for ent in &scene.entities {
        if let Some(a) = ent.components.animator.as_ref().filter(|a| a.enabled) {
            unique_push(&mut names, &a.controller);
        }
    }
    names
}

fn referenced_timelines(scene: &Scene) -> Vec<String> {
    let mut names = Vec::new();
    for ent in &scene.entities {
        if let Some(d) = ent
            .components
            .playable_director
            .as_ref()
            .filter(|d| d.enabled)
        {
            unique_push(&mut names, &d.timeline);
        }
    }
    names
}

fn index_of(names: &[String], name: &str) -> u16 {
    let n = name.trim();
    names
        .iter()
        .position(|s| s == n)
        .and_then(|i| u16::try_from(i).ok())
        .unwrap_or(WSCN_NO_REF)
}

fn resolve_cell(
    name: &str,
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
) -> (u16, [f32; 4], [f32; 2], [f32; 2]) {
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(name) {
            let tex = pack
                .texture_index(&r.sheet_texture)
                .filter(|&i| i <= u16::MAX as usize)
                .map(|i| i as u16)
                .unwrap_or(WSCN_TILE_NO_TEX);
            let uv = [r.uv[0], r.uv[1], r.uv[0] + r.uv[2], r.uv[1] + r.uv[3]];
            return (tex, uv, r.pivot, r.pixel_size);
        }
    }
    let tex = pack
        .texture_index(name)
        .filter(|&i| i <= u16::MAX as usize)
        .map(|i| i as u16)
        .unwrap_or(WSCN_TILE_NO_TEX);
    (tex, [0.0, 0.0, 1.0, 1.0], [0.5, 0.5], [16.0, 16.0])
}

fn write_cell_table(
    buf: &mut Vec<u8>,
    cells: &[String],
    pack: &WPack,
    catalog: Option<&SpriteCatalog>,
) -> Result<()> {
    if cells.len() > u16::MAX as usize {
        bail!("too many sprite cells for WSCN");
    }
    buf.write_u16::<LittleEndian>(cells.len() as u16)?;
    for name in cells {
        write_str(buf, name)?;
        let (tex, uv, pivot, size) = resolve_cell(name, pack, catalog);
        buf.write_u16::<LittleEndian>(tex)?;
        for v in uv {
            buf.write_f32::<LittleEndian>(v)?;
        }
        buf.write_f32::<LittleEndian>(pivot[0])?;
        buf.write_f32::<LittleEndian>(pivot[1])?;
        buf.write_f32::<LittleEndian>(size[0])?;
        buf.write_f32::<LittleEndian>(size[1])?;
    }
    Ok(())
}

fn write_clip_cells(
    buf: &mut Vec<u8>,
    anims: Option<&AnimClipCatalog>,
    stem: &str,
) -> Result<(f32, bool)> {
    if let Some(meta) = clip_meta(anims, stem) {
        if meta.cells.len() > u16::MAX as usize {
            bail!("anim clip '{}' has too many cells", stem);
        }
        buf.write_u16::<LittleEndian>(meta.cells.len() as u16)?;
        for cell in &meta.cells {
            write_str(buf, cell)?;
        }
        let fps = if meta.fps > 0.0 { meta.fps } else { 10.0 };
        Ok((fps, meta.loop_))
    } else {
        buf.write_u16::<LittleEndian>(0)?;
        Ok((10.0, true))
    }
}

fn write_blend_tree(
    buf: &mut Vec<u8>,
    bt: &BlendTreeMeta,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    let dim = match bt.dimension {
        BlendDimension::OneD => 0u8,
        BlendDimension::TwoD => 1u8,
    };
    buf.write_u8(dim)?;
    if bt.params.len() > u16::MAX as usize {
        bail!("blend tree has too many params");
    }
    buf.write_u16::<LittleEndian>(bt.params.len() as u16)?;
    for p in &bt.params {
        write_str(buf, p)?;
    }
    if bt.motions.len() > u16::MAX as usize {
        bail!("blend tree has too many motions");
    }
    buf.write_u16::<LittleEndian>(bt.motions.len() as u16)?;
    for m in &bt.motions {
        write_str(buf, &m.clip)?;
        let (fps, loop_) = write_clip_cells(buf, anims, &m.clip)?;
        buf.write_f32::<LittleEndian>(fps)?;
        buf.write_u8(if loop_ { 1 } else { 0 })?;
        buf.write_f32::<LittleEndian>(m.threshold.unwrap_or(0.0))?;
        let pos = m.position.unwrap_or([0.0, 0.0]);
        buf.write_f32::<LittleEndian>(pos[0])?;
        buf.write_f32::<LittleEndian>(pos[1])?;
    }
    Ok(())
}

fn write_state(
    buf: &mut Vec<u8>,
    s: &ControllerState,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    write_str(buf, &s.name)?;
    write_str(buf, &s.clip)?;
    buf.write_f32::<LittleEndian>(if s.speed > 0.0 { s.speed } else { 1.0 })?;
    let (fps, loop_) = write_clip_cells(buf, anims, &s.clip)?;
    buf.write_f32::<LittleEndian>(fps)?;
    buf.write_u8(if loop_ { 1 } else { 0 })?;
    if let Some(bt) = &s.blend_tree {
        buf.write_u8(1)?;
        write_blend_tree(buf, bt, anims)?;
    } else {
        buf.write_u8(0)?;
    }
    Ok(())
}

fn write_condition(
    buf: &mut Vec<u8>,
    c: &ControllerCondition,
    params: &[ControllerParam],
) -> Result<()> {
    let kind = params.iter().find(|p| p.name == c.param).map(|p| p.kind);
    // Packed like hydrate `runtime_condition`.
    if let Some(g) = c.greater {
        buf.write_u8(2)?; // FloatGreater
        write_str(buf, &c.param)?;
        buf.write_f32::<LittleEndian>(g)?;
        return Ok(());
    }
    if let Some(l) = c.less {
        buf.write_u8(3)?; // FloatLess
        write_str(buf, &c.param)?;
        buf.write_f32::<LittleEndian>(l)?;
        return Ok(());
    }
    if let Some(eq) = &c.equals {
        if let Some(b) = eq.as_bool() {
            if kind == Some(ControllerParamType::Trigger) && b {
                buf.write_u8(4)?; // Trigger
                write_str(buf, &c.param)?;
                return Ok(());
            }
            buf.write_u8(0)?; // BoolEq
            write_str(buf, &c.param)?;
            buf.write_u8(if b { 1 } else { 0 })?;
            return Ok(());
        }
        if let Some(n) = eq.as_f64() {
            buf.write_u8(1)?; // FloatEq
            write_str(buf, &c.param)?;
            buf.write_f32::<LittleEndian>(n as f32)?;
            return Ok(());
        }
    }
    if kind == Some(ControllerParamType::Trigger) {
        buf.write_u8(4)?;
        write_str(buf, &c.param)?;
        return Ok(());
    }
    buf.write_u8(0)?;
    write_str(buf, &c.param)?;
    buf.write_u8(1)?;
    Ok(())
}

fn write_transition(
    buf: &mut Vec<u8>,
    t: &ControllerTransition,
    params: &[ControllerParam],
) -> Result<()> {
    write_str(buf, &t.from)?;
    buf.write_u8(if is_any_state(&t.from) { 1 } else { 0 })?;
    write_str(buf, &t.to)?;
    buf.write_u8(if t.has_exit_time { 1 } else { 0 })?;
    if t.conditions.len() > u16::MAX as usize {
        bail!("too many transition conditions");
    }
    buf.write_u16::<LittleEndian>(t.conditions.len() as u16)?;
    for c in &t.conditions {
        write_condition(buf, c, params)?;
    }
    Ok(())
}

fn write_machine(
    buf: &mut Vec<u8>,
    states: &[ControllerState],
    transitions: &[ControllerTransition],
    params: &[ControllerParam],
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    if states.len() > u16::MAX as usize {
        bail!("too many animator states");
    }
    buf.write_u16::<LittleEndian>(states.len() as u16)?;
    for s in states {
        write_state(buf, s, anims)?;
    }
    if transitions.len() > u16::MAX as usize {
        bail!("too many animator transitions");
    }
    buf.write_u16::<LittleEndian>(transitions.len() as u16)?;
    for t in transitions {
        write_transition(buf, t, params)?;
    }
    Ok(())
}

fn write_controller(
    buf: &mut Vec<u8>,
    name: &str,
    meta: Option<&AnimatorControllerMeta>,
    anims: Option<&AnimClipCatalog>,
) -> Result<()> {
    write_str(buf, name)?;
    let Some(meta) = meta else {
        write_str(buf, "")?;
        buf.write_f32::<LittleEndian>(1.0)?;
        buf.write_u16::<LittleEndian>(0)?;
        write_machine(buf, &[], &[], &[], anims)?;
        buf.write_u16::<LittleEndian>(0)?;
        return Ok(());
    };
    write_str(buf, &meta.default_state)?;
    buf.write_f32::<LittleEndian>(meta.weight)?;
    if meta.parameters.len() > u16::MAX as usize {
        bail!("too many animator parameters");
    }
    buf.write_u16::<LittleEndian>(meta.parameters.len() as u16)?;
    for p in &meta.parameters {
        write_str(buf, &p.name)?;
        let kind = match p.kind {
            ControllerParamType::Bool => 0u8,
            ControllerParamType::Float => 1u8,
            ControllerParamType::Trigger => 2u8,
        };
        buf.write_u8(kind)?;
        buf.write_u8(if p.default_bool() { 1 } else { 0 })?;
        buf.write_f32::<LittleEndian>(p.default_float())?;
    }
    write_machine(
        buf,
        &meta.states,
        &meta.transitions,
        &meta.parameters,
        anims,
    )?;
    if meta.layers.len() > u16::MAX as usize {
        bail!("too many animator layers");
    }
    buf.write_u16::<LittleEndian>(meta.layers.len() as u16)?;
    for layer in &meta.layers {
        write_str(buf, &layer.name)?;
        buf.write_f32::<LittleEndian>(layer.weight)?;
        write_str(buf, &layer.default_state)?;
        write_machine(
            buf,
            &layer.states,
            &layer.transitions,
            &meta.parameters,
            anims,
        )?;
    }
    Ok(())
}

fn write_curve(buf: &mut Vec<u8>, keys: Option<&[wiimaker_assets::CurveKey]>) -> Result<()> {
    let keys = keys.unwrap_or(&[]);
    if keys.len() > u16::MAX as usize {
        bail!("too many curve keys");
    }
    buf.write_u16::<LittleEndian>(keys.len() as u16)?;
    for k in keys {
        buf.write_f32::<LittleEndian>(k.t)?;
        buf.write_f32::<LittleEndian>(k.v)?;
        let interp = match k.interp {
            wiimaker_assets::CurveInterp::Linear => 0u8,
            wiimaker_assets::CurveInterp::Constant => 1u8,
            wiimaker_assets::CurveInterp::Ease => 2u8,
            wiimaker_assets::CurveInterp::Unknown(_) => 0u8,
        };
        buf.write_u8(interp)?;
    }
    Ok(())
}

fn audio_index_string(stem: &str, pack: &WPack) -> String {
    if stem.trim().is_empty() {
        return String::new();
    }
    pack.audio_index(stem)
        .filter(|&i| i <= u16::MAX as usize && i != WSCN_AUDIO_NO_CLIP as usize)
        .map(|i| i.to_string())
        .unwrap_or_default()
}

fn write_timeline(
    buf: &mut Vec<u8>,
    name: &str,
    meta: Option<&TimelineMeta>,
    anims: Option<&AnimClipCatalog>,
    pack: &WPack,
) -> Result<()> {
    write_str(buf, name)?;
    let Some(meta) = meta else {
        buf.write_f32::<LittleEndian>(0.0)?;
        buf.write_u16::<LittleEndian>(0)?;
        return Ok(());
    };
    buf.write_f32::<LittleEndian>(meta.duration.max(0.0))?;
    if meta.tracks.len() > u16::MAX as usize {
        bail!("too many timeline tracks");
    }
    buf.write_u16::<LittleEndian>(meta.tracks.len() as u16)?;
    for track in &meta.tracks {
        write_str(buf, &track.name)?;
        let kind = match track.kind {
            TimelineTrackKind::Activation => 0u8,
            TimelineTrackKind::Animation => 1u8,
            TimelineTrackKind::Audio => 2u8,
            TimelineTrackKind::Transform => 3u8,
            TimelineTrackKind::Signal => 4u8,
            TimelineTrackKind::Control => 5u8,
            TimelineTrackKind::Float => 6u8,
        };
        buf.write_u8(kind)?;
        write_str(buf, track.binding.as_deref().unwrap_or(""))?;
        write_str(buf, track.property.as_deref().unwrap_or(""))?;
        if track.clips.len() > u16::MAX as usize {
            bail!("too many timeline clips");
        }
        buf.write_u16::<LittleEndian>(track.clips.len() as u16)?;
        for c in &track.clips {
            buf.write_f32::<LittleEndian>(c.start)?;
            buf.write_f32::<LittleEndian>(c.end)?;
            buf.write_u8(if c.active.unwrap_or(true) { 1 } else { 0 })?;
            let clip = c.clip.as_deref().unwrap_or("");
            write_str(buf, clip)?;
            let (fps, loop_) = write_clip_cells(buf, anims, clip)?;
            buf.write_f32::<LittleEndian>(fps)?;
            buf.write_u8(if loop_ { 1 } else { 0 })?;
            let audio = audio_index_string(c.audio.as_deref().unwrap_or(""), pack);
            write_str(buf, &audio)?;
            buf.write_f32::<LittleEndian>(c.volume.unwrap_or(1.0))?;
            let from = c.from.unwrap_or([0.0, 0.0]);
            let to = c.to.unwrap_or([0.0, 0.0]);
            buf.write_f32::<LittleEndian>(from[0])?;
            buf.write_f32::<LittleEndian>(from[1])?;
            buf.write_f32::<LittleEndian>(to[0])?;
            buf.write_f32::<LittleEndian>(to[1])?;
            write_str(buf, c.signal.as_deref().unwrap_or(""))?;
            write_str(buf, c.payload.as_deref().unwrap_or(""))?;
            write_curve(buf, c.curves.as_ref().and_then(|k| k.x.as_deref()))?;
            write_curve(buf, c.curves.as_ref().and_then(|k| k.y.as_deref()))?;
            write_curve(buf, c.curves.as_ref().and_then(|k| k.value.as_deref()))?;
        }
    }
    Ok(())
}

fn write_animator_entry(
    buf: &mut Vec<u8>,
    entity_index: usize,
    a: &SceneAnimator,
    controllers: &[String],
) -> Result<()> {
    if entity_index > u16::MAX as usize {
        bail!("entity index overflow in animator table");
    }
    buf.write_u16::<LittleEndian>(entity_index as u16)?;
    buf.write_u16::<LittleEndian>(index_of(controllers, &a.controller))?;
    if a.parameters.len() > u16::MAX as usize {
        bail!("too many animator param overrides");
    }
    buf.write_u16::<LittleEndian>(a.parameters.len() as u16)?;
    for p in &a.parameters {
        write_str(buf, &p.name)?;
        buf.write_u8(if p.bool_value.is_some() { 1 } else { 0 })?;
        buf.write_u8(if p.bool_value.unwrap_or(false) { 1 } else { 0 })?;
        buf.write_u8(if p.float_value.is_some() { 1 } else { 0 })?;
        buf.write_f32::<LittleEndian>(p.float_value.unwrap_or(0.0))?;
    }
    if a.layers.len() > u16::MAX as usize {
        bail!("too many animator layer overrides");
    }
    buf.write_u16::<LittleEndian>(a.layers.len() as u16)?;
    for layer in &a.layers {
        write_str(buf, &layer.name)?;
        buf.write_f32::<LittleEndian>(layer.weight)?;
    }
    Ok(())
}

fn write_director_entry(
    buf: &mut Vec<u8>,
    entity_index: usize,
    d: &ScenePlayableDirector,
    timelines: &[String],
) -> Result<()> {
    if entity_index > u16::MAX as usize {
        bail!("entity index overflow in director table");
    }
    buf.write_u16::<LittleEndian>(entity_index as u16)?;
    buf.write_u16::<LittleEndian>(index_of(timelines, &d.timeline))?;
    buf.write_u8(if d.play_on_awake { 1 } else { 0 })?;
    buf.write_u8(if d.loop_ { 1 } else { 0 })?;
    Ok(())
}

/// Append Animator / PlayableDirector tables. Caller already wrote the audio table
/// (including a zero count when this extension is the only tail).
pub fn write_animator_director_tables(
    buf: &mut Vec<u8>,
    scene: &Scene,
    cats: &BakeCatalogs<'_>,
) -> Result<()> {
    if !scene_has_anim_or_director(scene) {
        return Ok(());
    }
    let controllers = referenced_controllers(scene);
    let timelines = referenced_timelines(scene);
    let mut cells = Vec::new();
    for name in &controllers {
        if let Some(meta) = cats.controllers.and_then(|c| c.lookup(name)) {
            collect_controller_cells(&mut cells, cats.anims, meta);
        }
    }
    for name in &timelines {
        if let Some(meta) = cats.timelines.and_then(|c| c.lookup(name)) {
            collect_timeline_cells(&mut cells, cats.anims, meta);
        }
    }

    write_cell_table(buf, &cells, cats.pack, cats.sprites)?;

    if controllers.len() > u16::MAX as usize {
        bail!("too many animator controllers for WSCN");
    }
    buf.write_u16::<LittleEndian>(controllers.len() as u16)?;
    for name in &controllers {
        let meta = cats.controllers.and_then(|c| c.lookup(name));
        write_controller(buf, name, meta, cats.anims)?;
    }

    if timelines.len() > u16::MAX as usize {
        bail!("too many timelines for WSCN");
    }
    buf.write_u16::<LittleEndian>(timelines.len() as u16)?;
    for name in &timelines {
        let meta = cats.timelines.and_then(|c| c.lookup(name));
        write_timeline(buf, name, meta, cats.anims, cats.pack)?;
    }

    let animators: Vec<(usize, &SceneAnimator)> = scene
        .entities
        .iter()
        .enumerate()
        .filter_map(|(i, ent)| {
            ent.components
                .animator
                .as_ref()
                .filter(|a| a.enabled)
                .map(|a| (i, a))
        })
        .collect();
    if animators.len() > u16::MAX as usize {
        bail!("too many Animator components for WSCN");
    }
    buf.write_u16::<LittleEndian>(animators.len() as u16)?;
    for (i, a) in animators {
        write_animator_entry(buf, i, a, &controllers)?;
    }

    let directors: Vec<(usize, &ScenePlayableDirector)> = scene
        .entities
        .iter()
        .enumerate()
        .filter_map(|(i, ent)| {
            ent.components
                .playable_director
                .as_ref()
                .filter(|d| d.enabled)
                .map(|d| (i, d))
        })
        .collect();
    if directors.len() > u16::MAX as usize {
        bail!("too many PlayableDirector components for WSCN");
    }
    buf.write_u16::<LittleEndian>(directors.len() as u16)?;
    for (i, d) in directors {
        write_director_entry(buf, i, d, &timelines)?;
    }
    Ok(())
}

pub fn needs_animator_director_tables(scene: &Scene) -> bool {
    scene_has_anim_or_director(scene)
}
