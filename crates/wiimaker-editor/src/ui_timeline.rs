//! Timeline window + Playable Director transport (dark Pro).

use eframe::egui::{self, Color32, PointerButton, Rect, RichText, Sense, Stroke, Vec2};
use wiimaker_assets::{
    curve_add_curve, curve_add_key, curve_move_key, curve_remove_key, curve_set_interp,
    CurveInterp, CurveKey, CurveProp, TimelineClip, TimelineMeta, TimelineTrack, TimelineTrackKind,
};
use wiimaker_core::world::World;

use crate::app::{EditorApp, PlayMode, TlCurveKeySel};
use crate::theme;

pub(crate) struct DirectorTransport {
    pub(crate) play: bool,
    pub(crate) stop: bool,
    pub(crate) scrub: Option<f32>,
}

impl EditorApp {
    pub(crate) fn apply_director_transport(&mut self, name: &str, transport: DirectorTransport) {
        if !transport.play && !transport.stop && transport.scrub.is_none() {
            return;
        }
        let edit = self.play_mode == PlayMode::Edit;
        let plugin = !edit && self.play_session.as_ref().is_some_and(|s| s.is_plugin());
        let DirectorTransport { play, stop, scrub } = transport;
        let apply = |world: &mut World| {
            let Some(id) = world.find_by_name(name) else {
                return;
            };
            if stop {
                world.stop_timeline(id);
            }
            if play {
                world.play_timeline(id);
            }
            if let Some(t) = scrub {
                world.set_timeline_time(id, t);
            }
        };
        if plugin {
            if let Some(world) = self.play_session.as_mut().and_then(|s| s.world_mut()) {
                apply(world);
            }
        } else {
            apply(&mut self.world);
        }
        if play && edit {
            self.timeline_preview = true;
        }
        if stop && edit {
            self.timeline_preview = false;
            self.rehydrate();
        }
        if scrub.is_some() && edit && !stop {
            let ids: Vec<_> = self.world.iter_entities().collect();
            for id in ids {
                wiimaker_scene::apply_animation_frame(
                    &mut self.world,
                    &self.catalog,
                    self.atlas.map(),
                    id,
                );
            }
        }
        let lines = if plugin {
            self.play_session
                .as_mut()
                .and_then(|s| s.world_mut())
                .map(take_timeline_signal_lines)
                .unwrap_or_default()
        } else {
            take_timeline_signal_lines(&mut self.world)
        };
        for line in lines {
            self.console_push(crate::app::ConsoleLevel::Info, line);
        }
    }

    pub(crate) fn ui_timeline(&mut self, ui: &mut egui::Ui) {
        let entity = self.primary_selected().map(|s| s.to_string());
        let file_stem = self.selected_file.as_ref().and_then(|rel| {
            let name = rel.to_string_lossy();
            name.strip_suffix(".timeline.json")
                .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s).to_string())
        });
        let director_stem = entity.as_ref().and_then(|name| {
            self.scene
                .find_entity(name)
                .and_then(|e| e.components.playable_director.as_ref())
                .map(|d| d.timeline.clone())
        });
        let stem = director_stem.clone().or(file_stem);
        let Some(stem) = stem.filter(|s| !s.is_empty()) else {
            theme::muted(ui, "Select a Playable Director or a .timeline.json asset");
            return;
        };
        let meta = self.timeline_catalog.lookup(&stem).cloned();
        let duration = meta.as_ref().map(|m| m.duration).unwrap_or(1.0).max(0.001);
        let live = self.live_director_time(entity.as_deref());
        let (time, playing) = live.unwrap_or((0.0, false));

        let mut transport = DirectorTransport {
            play: false,
            stop: false,
            scrub: None,
        };
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Play").color(theme::TEXT))
                        .fill(theme::BG_RAISED),
                )
                .clicked()
            {
                transport.play = true;
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Stop").color(theme::TEXT))
                        .fill(theme::BG_RAISED),
                )
                .clicked()
            {
                transport.stop = true;
            }
            ui.label(
                RichText::new(format!(
                    "{}  {:.2} / {:.2}s{}",
                    stem,
                    time,
                    duration,
                    if playing { "  playing" } else { "" }
                ))
                .size(12.0)
                .color(theme::TEXT),
            );
        });
        self.ui_curve_key_strip(ui, &stem);

        let label_w = 120.0;
        let row_h = 22.0;
        let ruler_h = 18.0;
        let full = ui.available_width();
        let track_w = (full - label_w).max(40.0);
        let (ruler, ruler_resp) =
            ui.allocate_exact_size(Vec2::new(full, ruler_h), Sense::click_and_drag());
        let painter = ui.painter_at(ruler);
        let lane = Rect::from_min_size(
            ruler.left_top() + Vec2::new(label_w, 0.0),
            Vec2::new(track_w, ruler_h),
        );
        painter.rect_filled(lane, 0.0, theme::BG_SUNKEN);
        let ticks = 4;
        for i in 0..=ticks {
            let t = duration * (i as f32 / ticks as f32);
            let x = lane.left() + lane.width() * (t / duration);
            painter.line_segment(
                [egui::pos2(x, lane.top()), egui::pos2(x, lane.bottom())],
                Stroke::new(1.0_f32, theme::BORDER_SOFT),
            );
            painter.text(
                egui::pos2(x + 2.0, lane.top()),
                egui::Align2::LEFT_TOP,
                format!("{t:.1}"),
                egui::FontId::proportional(10.0),
                theme::TEXT_DIM,
            );
        }
        let mut curve_edits = Vec::new();
        if let Some(meta) = &meta {
            let open = self.tl_curve_open.clone();
            let selected = self.tl_curve_key.clone();
            for track in &meta.tracks {
                paint_track_row(
                    ui,
                    track,
                    duration,
                    label_w,
                    track_w,
                    full,
                    row_h,
                    open.iter().any(|n| n == &track.name),
                    selected.as_ref(),
                    &mut curve_edits,
                );
            }
            self.ui_timeline_authoring(ui, &stem);
        } else {
            theme::muted(ui, &format!("missing assets/{stem}.timeline.json"));
        }

        let head_x = lane.left() + lane.width() * (time / duration).clamp(0.0, 1.0);
        let body = Rect::from_min_max(ruler.left_top(), ui.min_rect().right_bottom());
        ui.painter().line_segment(
            [
                egui::pos2(head_x, ruler.top()),
                egui::pos2(head_x, body.bottom().max(ruler.bottom())),
            ],
            Stroke::new(1.5_f32, theme::ACCENT),
        );

        if entity.is_some() && (ruler_resp.clicked() || ruler_resp.dragged()) {
            if let Some(pos) = ruler_resp.interact_pointer_pos() {
                let u = ((pos.x - lane.left()) / lane.width()).clamp(0.0, 1.0);
                transport.scrub = Some(u * duration);
            }
        }
        if let Some(name) = entity {
            self.apply_director_transport(&name, transport);
        }
        self.apply_curve_edits(&stem, curve_edits);
    }

    pub(crate) fn ui_timeline_asset_inspector(&mut self, ui: &mut egui::Ui, rel: &std::path::Path) {
        let name = rel.to_string_lossy();
        let Some(stem) = name.strip_suffix(".timeline.json") else {
            return;
        };
        let stem = stem.rsplit(['/', '\\']).next().unwrap_or(stem).to_string();
        let Some(meta) = self.timeline_catalog.lookup(&stem).cloned() else {
            theme::muted(ui, "Timeline failed to load");
            return;
        };
        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            ui.label(
                RichText::new("Timeline")
                    .strong()
                    .size(13.0)
                    .color(theme::TEXT),
            );
            self.ui_curve_key_strip(ui, &stem);
            let mut duration = meta.duration;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Duration").color(theme::TEXT_MUTED));
                if ui
                    .add(
                        egui::DragValue::new(&mut duration)
                            .speed(0.05)
                            .range(0.0..=600.0),
                    )
                    .changed()
                {
                    let mut next = meta.clone();
                    next.duration = duration;
                    let path = self.game_dir.join(rel);
                    if let Err(e) = next.save(&path) {
                        self.status = format!("timeline save: {e}");
                    } else if let Err(e) = self.reload_assets() {
                        self.status = format!("timeline reload: {e}");
                    } else {
                        self.status = format!("saved {stem}.timeline.json");
                    }
                }
            });
            for track in &meta.tracks {
                let binding = track.binding.as_deref().unwrap_or("-");
                let prop = track
                    .property
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("  {s}"))
                    .unwrap_or_default();
                ui.label(
                    RichText::new(format!(
                        "{}  {}  {}{prop}  ({} clips)",
                        track.name,
                        kind_label(track.kind),
                        binding,
                        track.clips.len()
                    ))
                    .size(12.0)
                    .color(theme::TEXT),
                );
                if track.kind == TimelineTrackKind::Signal {
                    for clip in &track.clips {
                        let payload = clip.payload.as_deref().unwrap_or("");
                        let extra = if payload.is_empty() {
                            String::new()
                        } else {
                            format!("  {payload}")
                        };
                        ui.label(
                            RichText::new(format!(
                                "    {:.2}  {}{extra}",
                                clip.start,
                                clip.signal.as_deref().unwrap_or("Signal")
                            ))
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                        );
                    }
                }
            }
            self.ui_timeline_authoring(ui, &stem);
        });
    }

    fn ui_timeline_authoring(&mut self, ui: &mut egui::Ui, stem: &str) {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Add Signal Marker")
                .size(12.0)
                .color(theme::TEXT_MUTED),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Track").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_signal_track)
                    .desired_width(72.0)
                    .hint_text("Cues"),
            );
            ui.label(RichText::new("Signal").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_signal_name)
                    .desired_width(88.0)
                    .hint_text("IntroDone"),
            );
            ui.label(RichText::new("Payload").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_signal_payload)
                    .desired_width(64.0)
                    .hint_text("optional"),
            );
            ui.label(RichText::new("On").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_signal_binding)
                    .desired_width(72.0)
                    .hint_text("entity"),
            );
            ui.label(RichText::new("Time").color(theme::TEXT_MUTED));
            ui.add(
                egui::DragValue::new(&mut self.tl_signal_time)
                    .speed(0.01)
                    .range(0.0..=600.0),
            );
        });
        if ui
            .add(
                egui::Button::new(RichText::new("Add Signal Marker").color(theme::TEXT))
                    .fill(theme::BG_RAISED),
            )
            .clicked()
        {
            self.commit_signal_marker(stem);
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("Add Control Clip")
                .size(12.0)
                .color(theme::TEXT_MUTED),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Track").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_control_track)
                    .desired_width(72.0)
                    .hint_text("Control"),
            );
            ui.label(RichText::new("Target").color(theme::TEXT_MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.tl_control_target)
                    .desired_width(88.0)
                    .hint_text("entity"),
            );
            ui.label(RichText::new("Start").color(theme::TEXT_MUTED));
            ui.add(
                egui::DragValue::new(&mut self.tl_control_start)
                    .speed(0.01)
                    .range(0.0..=600.0),
            );
            ui.label(RichText::new("End").color(theme::TEXT_MUTED));
            ui.add(
                egui::DragValue::new(&mut self.tl_control_end)
                    .speed(0.01)
                    .range(0.0..=600.0),
            );
        });
        if ui
            .add(
                egui::Button::new(RichText::new("Add Control Clip").color(theme::TEXT))
                    .fill(theme::BG_RAISED),
            )
            .clicked()
        {
            self.commit_control_clip(stem);
        }
    }

    fn commit_signal_marker(&mut self, stem: &str) {
        let track_name = self.tl_signal_track.trim().to_string();
        let signal = self.tl_signal_name.trim().to_string();
        if track_name.is_empty() || signal.is_empty() {
            self.status = "signal marker needs a track name and a signal name".into();
            return;
        }
        let time = self.tl_signal_time.max(0.0);
        let payload = {
            let p = self.tl_signal_payload.trim();
            if p.is_empty() {
                None
            } else {
                Some(p.to_string())
            }
        };
        let binding = {
            let b = self.tl_signal_binding.trim();
            if b.is_empty() || b == "-" {
                None
            } else {
                Some(b.to_string())
            }
        };
        let Some(mut meta) = self.timeline_catalog.lookup(stem).cloned() else {
            self.status = format!("missing assets/{stem}.timeline.json");
            return;
        };
        let clip = TimelineClip::signal(time, signal.clone(), payload.as_deref());
        push_clip(
            &mut meta,
            TimelineTrack {
                name: track_name,
                kind: TimelineTrackKind::Signal,
                binding,
                property: None,
                clips: vec![clip],
            },
        );
        self.save_timeline_meta(stem, meta);
    }

    fn commit_control_clip(&mut self, stem: &str) {
        let track_name = self.tl_control_track.trim().to_string();
        let target = self.tl_control_target.trim().to_string();
        if track_name.is_empty() || target.is_empty() || target == "-" {
            self.status = "control clip needs a track name and a target entity".into();
            return;
        }
        let start = self.tl_control_start.max(0.0);
        let end = self.tl_control_end.max(0.0);
        if end < start {
            self.status = "control clip end is before start".into();
            return;
        }
        let Some(mut meta) = self.timeline_catalog.lookup(stem).cloned() else {
            self.status = format!("missing assets/{stem}.timeline.json");
            return;
        };
        push_clip(
            &mut meta,
            TimelineTrack {
                name: track_name,
                kind: TimelineTrackKind::Control,
                binding: Some(target),
                property: None,
                clips: vec![TimelineClip::control(start, end)],
            },
        );
        self.save_timeline_meta(stem, meta);
    }

    fn save_timeline_meta(&mut self, stem: &str, meta: TimelineMeta) {
        let rel = std::path::PathBuf::from(format!("assets/{stem}.timeline.json"));
        let path = self.game_dir.join(&rel);
        if let Err(e) = meta.save(&path) {
            self.status = format!("timeline save: {e}");
            return;
        }
        if let Err(e) = self.reload_assets() {
            self.status = format!("timeline reload: {e}");
            return;
        }
        if self.play_mode == PlayMode::Edit {
            self.rehydrate();
        }
        self.status = format!("saved {stem}.timeline.json");
    }

    fn live_director_time(&self, name: Option<&str>) -> Option<(f32, bool)> {
        let name = name?;
        let world = if self.play_mode != PlayMode::Edit {
            self.play_session
                .as_ref()
                .and_then(|s| s.world())
                .unwrap_or(&self.world)
        } else {
            &self.world
        };
        let id = world.find_by_name(name)?;
        let d = world.director(id)?;
        Some((d.time, d.playing))
    }

    fn ui_curve_key_strip(&mut self, ui: &mut egui::Ui, stem: &str) {
        let Some(sel) = self.tl_curve_key.clone() else {
            return;
        };
        let Some(meta) = self.timeline_catalog.lookup(stem).cloned() else {
            return;
        };
        let Some(track) = meta.tracks.iter().find(|t| t.name == sel.track) else {
            return;
        };
        let Some(clip) = track.clips.get(sel.clip) else {
            return;
        };
        let Some(prop) = CurveProp::parse(&sel.prop) else {
            return;
        };
        let Some(key) = clip_keys(clip, prop)
            .and_then(|k| k.get(sel.index))
            .cloned()
        else {
            return;
        };
        let span = clip.span();
        let mut t = key.t;
        let mut v = key.v;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Key  {}  {}  {}",
                    sel.track,
                    prop.as_str(),
                    key.interp.as_str()
                ))
                .size(12.0)
                .color(theme::TEXT_MUTED),
            );
            ui.label(RichText::new("t").color(prop_color(prop)));
            if ui
                .add(
                    egui::DragValue::new(&mut t)
                        .speed(0.01)
                        .range(0.0..=span.max(0.0)),
                )
                .changed()
            {
                changed = true;
            }
            ui.label(RichText::new("v").color(prop_color(prop)));
            if ui.add(egui::DragValue::new(&mut v).speed(0.05)).changed() {
                changed = true;
            }
        });
        if changed {
            let mut meta = meta;
            let t = t.clamp(0.0, span);
            if let Ok(edit) = curve_move_key(&mut meta, &sel.track, sel.clip, prop, sel.index, t, v)
            {
                if let Some(index) = edit.index {
                    self.tl_curve_key = Some(TlCurveKeySel {
                        track: sel.track,
                        clip: sel.clip,
                        prop: sel.prop,
                        index,
                    });
                }
                self.commit_curve_meta(stem, meta);
            }
        }
    }

    fn apply_curve_edits(&mut self, stem: &str, edits: Vec<CurveUiEdit>) {
        if edits.is_empty() {
            return;
        }
        let Some(mut meta) = self.timeline_catalog.lookup(stem).cloned() else {
            return;
        };
        let mut dirty = false;
        for edit in edits {
            match edit {
                CurveUiEdit::Toggle(name) => {
                    if let Some(i) = self.tl_curve_open.iter().position(|n| n == &name) {
                        self.tl_curve_open.remove(i);
                    } else {
                        self.tl_curve_open.push(name);
                    }
                }
                CurveUiEdit::Select(sel) => self.tl_curve_key = Some(sel),
                CurveUiEdit::Move { sel, t, v } => {
                    let Some(prop) = CurveProp::parse(&sel.prop) else {
                        continue;
                    };
                    let span = meta
                        .tracks
                        .iter()
                        .find(|tr| tr.name == sel.track)
                        .and_then(|tr| tr.clips.get(sel.clip))
                        .map(|c| c.span())
                        .unwrap_or(0.0);
                    let t = t.clamp(0.0, span);
                    if let Ok(done) =
                        curve_move_key(&mut meta, &sel.track, sel.clip, prop, sel.index, t, v)
                    {
                        dirty = true;
                        if let Some(index) = done.index {
                            self.tl_curve_key = Some(TlCurveKeySel { index, ..sel });
                        }
                    }
                }
                CurveUiEdit::AddKey {
                    track,
                    clip,
                    prop,
                    t,
                    v,
                } => {
                    let v = sample_clip_prop(
                        meta.tracks
                            .iter()
                            .find(|tr| tr.name == track)
                            .and_then(|tr| tr.clips.get(clip)),
                        prop,
                        t,
                    )
                    .unwrap_or(v);
                    if curve_add_key(&mut meta, &track, clip, prop, t, v, CurveInterp::Linear)
                        .is_ok()
                    {
                        dirty = true;
                    }
                }
                CurveUiEdit::Delete {
                    track,
                    clip,
                    prop,
                    index,
                } => {
                    if curve_remove_key(&mut meta, &track, clip, prop, Some(index), None).is_ok() {
                        dirty = true;
                        if self.tl_curve_key.as_ref().is_some_and(|s| {
                            s.track == track
                                && s.clip == clip
                                && s.prop == prop.as_str()
                                && s.index == index
                        }) {
                            self.tl_curve_key = None;
                        }
                    }
                }
                CurveUiEdit::Interp {
                    track,
                    clip,
                    prop,
                    index,
                    interp,
                } => {
                    if curve_set_interp(&mut meta, &track, clip, prop, index, interp).is_ok() {
                        dirty = true;
                    }
                }
                CurveUiEdit::AddCurve { track, clip, prop } => {
                    if curve_add_curve(&mut meta, &track, clip, prop).is_ok() {
                        dirty = true;
                    }
                }
            }
        }
        if dirty {
            self.commit_curve_meta(stem, meta);
        }
    }

    fn commit_curve_meta(&mut self, stem: &str, meta: TimelineMeta) {
        let snaps = self.director_snaps();
        self.save_timeline_meta(stem, meta);
        if self.play_mode == PlayMode::Edit {
            self.restore_director_snaps(&snaps);
        }
    }

    fn director_snaps(&self) -> Vec<(String, f32, bool)> {
        let mut out = Vec::new();
        for id in self.world.iter_entities() {
            if let (Some(name), Some(d)) = (self.world.name(id), self.world.director(id)) {
                out.push((name.to_string(), d.time, d.playing));
            }
        }
        out
    }

    fn restore_director_snaps(&mut self, snaps: &[(String, f32, bool)]) {
        for (name, time, playing) in snaps {
            let Some(id) = self.world.find_by_name(name) else {
                continue;
            };
            self.world.set_timeline_time(id, *time);
            if let Some(d) = self.world.director_mut(id) {
                d.playing = *playing;
            }
        }
    }
}

fn kind_label(kind: TimelineTrackKind) -> &'static str {
    match kind {
        TimelineTrackKind::Activation => "Activation",
        TimelineTrackKind::Animation => "Animation",
        TimelineTrackKind::Audio => "Audio",
        TimelineTrackKind::Transform => "Transform",
        TimelineTrackKind::Signal => "Signal",
        TimelineTrackKind::Control => "Control",
        TimelineTrackKind::Float => "Float",
    }
}

fn track_color(kind: TimelineTrackKind) -> Color32 {
    match kind {
        TimelineTrackKind::Activation => Color32::from_rgb(76, 140, 196),
        TimelineTrackKind::Animation => Color32::from_rgb(196, 140, 64),
        TimelineTrackKind::Audio => Color32::from_rgb(72, 150, 110),
        TimelineTrackKind::Transform => Color32::from_rgb(150, 110, 186),
        TimelineTrackKind::Signal => Color32::from_rgb(220, 196, 96),
        TimelineTrackKind::Control => Color32::from_rgb(70, 158, 168),
        TimelineTrackKind::Float => Color32::from_rgb(86, 156, 214),
    }
}

fn paint_signal_marker(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let r = 5.0;
    let pts = vec![
        egui::pos2(center.x, center.y - r),
        egui::pos2(center.x + r * 0.72, center.y),
        egui::pos2(center.x, center.y + r),
        egui::pos2(center.x - r * 0.72, center.y),
    ];
    painter.add(egui::Shape::convex_polygon(
        pts,
        color,
        Stroke::new(1.0_f32, Color32::from_rgb(236, 232, 214)),
    ));
}

fn push_clip(meta: &mut TimelineMeta, track: TimelineTrack) {
    if let Some(existing) = meta.tracks.iter_mut().find(|t| {
        t.name == track.name
            && t.kind == track.kind
            && t.binding == track.binding
            && t.property == track.property
    }) {
        existing.clips.extend(track.clips);
    } else {
        meta.tracks.push(track);
    }
}

/// Drain fired signals into Console lines. Empty when nothing crossed this frame.
pub(crate) fn take_timeline_signal_lines(world: &mut World) -> Vec<String> {
    world
        .take_timeline_signals()
        .into_iter()
        .map(|s| {
            let who = if s.binding.is_empty() {
                "unbound".to_string()
            } else {
                s.binding
            };
            if s.payload.is_empty() {
                format!("signal {} @ {:.3} ({who})", s.signal, s.time)
            } else {
                format!(
                    "signal {} @ {:.3} ({who}) · {}",
                    s.signal, s.time, s.payload
                )
            }
        })
        .collect()
}

enum CurveUiEdit {
    Toggle(String),
    Select(TlCurveKeySel),
    Move {
        sel: TlCurveKeySel,
        t: f32,
        v: f32,
    },
    AddKey {
        track: String,
        clip: usize,
        prop: CurveProp,
        t: f32,
        v: f32,
    },
    Delete {
        track: String,
        clip: usize,
        prop: CurveProp,
        index: usize,
    },
    Interp {
        track: String,
        clip: usize,
        prop: CurveProp,
        index: usize,
        interp: CurveInterp,
    },
    AddCurve {
        track: String,
        clip: usize,
        prop: CurveProp,
    },
}

fn clip_keys(clip: &TimelineClip, prop: CurveProp) -> Option<&[CurveKey]> {
    let curves = clip.curves.as_ref()?;
    match prop {
        CurveProp::X => curves.x.as_deref(),
        CurveProp::Y => curves.y.as_deref(),
        CurveProp::Value => curves.value.as_deref(),
    }
}

fn prop_color(prop: CurveProp) -> Color32 {
    match prop {
        CurveProp::X => theme::AXIS_X,
        CurveProp::Y => theme::AXIS_Y,
        CurveProp::Value => theme::AXIS_Z,
    }
}

fn prop_label(track: &TimelineTrack, prop: CurveProp) -> String {
    match prop {
        CurveProp::X => "X".into(),
        CurveProp::Y => "Y".into(),
        CurveProp::Value => track
            .property
            .as_deref()
            .unwrap_or("value")
            .rsplit('.')
            .next()
            .unwrap_or("value")
            .to_string(),
    }
}

fn sample_clip_prop(clip: Option<&TimelineClip>, prop: CurveProp, local: f32) -> Option<f32> {
    let clip = clip?;
    if let Some(keys) = clip_keys(clip, prop) {
        if !keys.is_empty() {
            let curve = wiimaker_core::Curve::from_keys(
                keys.iter()
                    .map(|k| wiimaker_core::CurveKey {
                        t: k.t,
                        v: k.v,
                        interp: k.interp.to_core(),
                    })
                    .collect(),
            );
            return curve.sample(local);
        }
    }
    let span = clip.span().max(1e-6);
    let u = (local / span).clamp(0.0, 1.0);
    Some(match prop {
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
    })
}

fn paint_track_row(
    ui: &mut egui::Ui,
    track: &TimelineTrack,
    duration: f32,
    label_w: f32,
    track_w: f32,
    full: f32,
    row_h: f32,
    open: bool,
    selected: Option<&TlCurveKeySel>,
    edits: &mut Vec<CurveUiEdit>,
) {
    let curved = matches!(
        track.kind,
        TimelineTrackKind::Transform | TimelineTrackKind::Float
    );
    let (row, _) = ui.allocate_exact_size(Vec2::new(full, row_h), Sense::hover());
    let painter = ui.painter_at(row);
    painter.rect_filled(row, 0.0, theme::BG_PANEL);
    let name_x = if curved { 18.0 } else { 4.0 };
    painter.text(
        row.left_center() + Vec2::new(name_x, 0.0),
        egui::Align2::LEFT_CENTER,
        track.name.clone(),
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );
    if curved {
        let fold = Rect::from_min_size(row.left_top() + Vec2::new(1.0, 3.0), Vec2::new(16.0, 16.0));
        let mut toggled = false;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(fold), |ui| {
            if theme::foldout_button(ui, open).clicked() {
                toggled = true;
            }
        });
        if toggled {
            edits.push(CurveUiEdit::Toggle(track.name.clone()));
        }
    }
    let lane = Rect::from_min_size(
        row.left_top() + Vec2::new(label_w, 1.0),
        Vec2::new(track_w, row_h - 2.0),
    );
    painter.rect_filled(lane, 0.0, theme::BG_SUNKEN);
    if track.kind == TimelineTrackKind::Signal {
        for clip in &track.clips {
            let x = lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
            let center = egui::pos2(x, lane.center().y);
            paint_signal_marker(&painter, center, track_color(track.kind));
            let label = clip.signal.as_deref().unwrap_or("");
            if !label.is_empty() {
                painter.text(
                    center + Vec2::new(7.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    theme::TEXT,
                );
            }
        }
    } else {
        for clip in &track.clips {
            let x0 = lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
            let x1 = lane.left() + lane.width() * (clip.end / duration).clamp(0.0, 1.0);
            let block = Rect::from_min_max(
                egui::pos2(x0, lane.top() + 2.0),
                egui::pos2(x1.max(x0 + 2.0), lane.bottom() - 2.0),
            );
            painter.rect_filled(block, 2.0, track_color(track.kind));
            if track.kind == TimelineTrackKind::Control {
                let label = track
                    .binding
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Control");
                painter.with_clip_rect(block).text(
                    block.left_center() + Vec2::new(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    theme::TEXT,
                );
            }
        }
    }
    if curved && open {
        for (ci, clip) in track.clips.iter().enumerate() {
            let mut props: Vec<CurveProp> = Vec::new();
            if track.kind == TimelineTrackKind::Transform {
                if clip_keys(clip, CurveProp::X).is_some() {
                    props.push(CurveProp::X);
                }
                if clip_keys(clip, CurveProp::Y).is_some() {
                    props.push(CurveProp::Y);
                }
            } else if clip_keys(clip, CurveProp::Value).is_some() {
                props.push(CurveProp::Value);
            }
            if props.is_empty() {
                paint_curve_lane(
                    ui, track, ci, clip, None, duration, label_w, track_w, full, selected, edits,
                );
            } else {
                for prop in props {
                    paint_curve_lane(
                        ui,
                        track,
                        ci,
                        clip,
                        Some(prop),
                        duration,
                        label_w,
                        track_w,
                        full,
                        selected,
                        edits,
                    );
                }
            }
        }
    }
}

fn paint_curve_lane(
    ui: &mut egui::Ui,
    track: &TimelineTrack,
    clip_i: usize,
    clip: &TimelineClip,
    prop: Option<CurveProp>,
    duration: f32,
    label_w: f32,
    track_w: f32,
    full: f32,
    selected: Option<&TlCurveKeySel>,
    edits: &mut Vec<CurveUiEdit>,
) {
    let lane_h = 64.0;
    let (row, resp) = ui.allocate_exact_size(Vec2::new(full, lane_h), Sense::click_and_drag());
    let painter = ui.painter_at(row);
    painter.rect_filled(row, 0.0, theme::BG_PANEL);
    let label = match prop {
        Some(p) => prop_label(track, p),
        None => "curve".into(),
    };
    let color = prop.map(prop_color).unwrap_or(theme::TEXT_DIM);
    painter.text(
        row.left_center() + Vec2::new(18.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(11.0),
        color,
    );
    let lane = Rect::from_min_size(
        row.left_top() + Vec2::new(label_w, 2.0),
        Vec2::new(track_w, lane_h - 4.0),
    );
    painter.rect_filled(lane, 0.0, theme::BG_SUNKEN);
    let duration = duration.max(1e-4);
    let x0 = lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
    let x1 = lane.left() + lane.width() * (clip.end / duration).clamp(0.0, 1.0);
    let block = Rect::from_min_max(
        egui::pos2(x0, lane.top()),
        egui::pos2(x1.max(x0 + 4.0), lane.bottom()),
    );
    painter.rect_filled(
        block,
        0.0,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28),
    );
    let keys = prop.and_then(|p| clip_keys(clip, p)).unwrap_or(&[]);
    let (lo, hi) = value_range(keys);
    if keys.len() >= 2 {
        if let Some(p) = prop {
            let steps = 32;
            let mut pts = Vec::with_capacity(steps);
            for i in 0..steps {
                let u = i as f32 / (steps - 1) as f32;
                let local = clip.span() * u;
                let v = sample_clip_prop(Some(clip), p, local).unwrap_or(0.0);
                let x = block.left() + block.width() * u;
                let y = value_to_y(v, lo, hi, block);
                pts.push(egui::pos2(x, y));
            }
            painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, color)));
        }
    }
    let mut positions = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let u = if clip.span() <= 1e-6 {
            0.0
        } else {
            (key.t / clip.span()).clamp(0.0, 1.0)
        };
        let center = egui::pos2(
            block.left() + block.width() * u,
            value_to_y(key.v, lo, hi, block),
        );
        positions.push((i, center));
        let sel = selected.is_some_and(|s| {
            s.track == track.name
                && s.clip == clip_i
                && prop.is_some_and(|p| s.prop == p.as_str())
                && s.index == i
        });
        paint_key_diamond(&painter, center, color, sel);
    }
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let hit = pointer.and_then(|pos| {
        positions
            .iter()
            .find(|(_, c)| c.distance(pos) <= 7.0)
            .map(|(i, _)| *i)
    });
    if let Some(p) = prop {
        if resp.clicked_by(PointerButton::Primary) {
            if let Some(index) = hit {
                edits.push(CurveUiEdit::Select(TlCurveKeySel {
                    track: track.name.clone(),
                    clip: clip_i,
                    prop: p.as_str().to_string(),
                    index,
                }));
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            let index = hit.or_else(|| {
                selected.and_then(|s| {
                    if s.track == track.name && s.clip == clip_i && s.prop == p.as_str() {
                        Some(s.index)
                    } else {
                        None
                    }
                })
            });
            if let (Some(index), Some(pos)) = (index, resp.interact_pointer_pos()) {
                let span = clip.span();
                let u = if block.width() <= 1.0 {
                    0.0
                } else {
                    ((pos.x - block.left()) / block.width()).clamp(0.0, 1.0)
                };
                let t = (span * u).clamp(0.0, span);
                let v = y_to_value(pos.y, lo, hi, block);
                edits.push(CurveUiEdit::Select(TlCurveKeySel {
                    track: track.name.clone(),
                    clip: clip_i,
                    prop: p.as_str().to_string(),
                    index,
                }));
                edits.push(CurveUiEdit::Move {
                    sel: TlCurveKeySel {
                        track: track.name.clone(),
                        clip: clip_i,
                        prop: p.as_str().to_string(),
                        index,
                    },
                    t,
                    v,
                });
            }
        }
    }
    let track_name = track.name.clone();
    let kind = track.kind;
    resp.context_menu(|ui| {
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = theme::BG_RAISED;
        if let Some(p) = prop {
            if let Some(index) = hit {
                if ui.button("Delete Key").clicked() {
                    edits.push(CurveUiEdit::Delete {
                        track: track_name.clone(),
                        clip: clip_i,
                        prop: p,
                        index,
                    });
                    ui.close_menu();
                }
                ui.menu_button("Interpolation", |ui| {
                    for (label, interp) in [
                        ("Linear", CurveInterp::Linear),
                        ("Constant", CurveInterp::Constant),
                        ("Ease", CurveInterp::Ease),
                    ] {
                        if ui.button(label).clicked() {
                            edits.push(CurveUiEdit::Interp {
                                track: track_name.clone(),
                                clip: clip_i,
                                prop: p,
                                index,
                                interp,
                            });
                            ui.close_menu();
                        }
                    }
                });
            }
            if ui.button("Add Key").clicked() {
                let local = pointer
                    .map(|pos| {
                        let u = if block.width() <= 1.0 {
                            0.0
                        } else {
                            ((pos.x - block.left()) / block.width()).clamp(0.0, 1.0)
                        };
                        clip.span() * u
                    })
                    .unwrap_or(0.0);
                let v = sample_clip_prop(Some(clip), p, local).unwrap_or(0.0);
                edits.push(CurveUiEdit::AddKey {
                    track: track_name.clone(),
                    clip: clip_i,
                    prop: p,
                    t: local,
                    v,
                });
                ui.close_menu();
            }
        }
        if kind == TimelineTrackKind::Transform
            && clip_keys(clip, CurveProp::X).is_none()
            && ui.button("Add Curve X").clicked()
        {
            edits.push(CurveUiEdit::AddCurve {
                track: track_name.clone(),
                clip: clip_i,
                prop: CurveProp::X,
            });
            ui.close_menu();
        }
        if kind == TimelineTrackKind::Transform
            && clip_keys(clip, CurveProp::Y).is_none()
            && ui.button("Add Curve Y").clicked()
        {
            edits.push(CurveUiEdit::AddCurve {
                track: track_name.clone(),
                clip: clip_i,
                prop: CurveProp::Y,
            });
            ui.close_menu();
        }
        if kind == TimelineTrackKind::Float
            && clip_keys(clip, CurveProp::Value).is_none()
            && ui.button("Add Curve").clicked()
        {
            edits.push(CurveUiEdit::AddCurve {
                track: track_name.clone(),
                clip: clip_i,
                prop: CurveProp::Value,
            });
            ui.close_menu();
        }
    });
}

fn value_range(keys: &[CurveKey]) -> (f32, f32) {
    if keys.is_empty() {
        return (-1.0, 1.0);
    }
    let mut lo = keys[0].v;
    let mut hi = keys[0].v;
    for k in keys {
        lo = lo.min(k.v);
        hi = hi.max(k.v);
    }
    if (hi - lo).abs() < 1e-3 {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = (hi - lo) * 0.15;
    (lo - pad, hi + pad)
}

fn value_to_y(v: f32, lo: f32, hi: f32, block: Rect) -> f32 {
    let u = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    block.bottom() - u * block.height()
}

fn y_to_value(y: f32, lo: f32, hi: f32, block: Rect) -> f32 {
    let u = ((block.bottom() - y) / block.height().max(1.0)).clamp(0.0, 1.0);
    lo + (hi - lo) * u
}

fn paint_key_diamond(painter: &egui::Painter, center: egui::Pos2, color: Color32, selected: bool) {
    let r = if selected { 6.0_f32 } else { 4.5_f32 };
    let pts = vec![
        egui::pos2(center.x, center.y - r),
        egui::pos2(center.x + r, center.y),
        egui::pos2(center.x, center.y + r),
        egui::pos2(center.x - r, center.y),
    ];
    let stroke = if selected {
        Stroke::new(1.5_f32, Color32::from_rgb(236, 236, 236))
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20))
    };
    painter.add(egui::Shape::convex_polygon(pts, color, stroke));
}
