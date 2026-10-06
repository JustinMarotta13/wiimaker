//! Timeline window + Playable Director transport (dark Pro).

use eframe::egui::{self, Color32, Rect, RichText, Sense, Stroke, Vec2};
use wiimaker_assets::{TimelineClip, TimelineMeta, TimelineTrack, TimelineTrackKind};
use wiimaker_core::world::World;

use crate::app::{EditorApp, PlayMode};
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
        if let Some(meta) = &meta {
            for track in &meta.tracks {
                let (row, _) = ui.allocate_exact_size(Vec2::new(full, row_h), Sense::hover());
                let painter = ui.painter_at(row);
                painter.rect_filled(row, 0.0, theme::BG_PANEL);
                painter.text(
                    row.left_center() + Vec2::new(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    track.name.clone(),
                    egui::FontId::proportional(12.0),
                    theme::TEXT,
                );
                let lane = Rect::from_min_size(
                    row.left_top() + Vec2::new(label_w, 1.0),
                    Vec2::new(track_w, row_h - 2.0),
                );
                painter.rect_filled(lane, 0.0, theme::BG_SUNKEN);
                if track.kind == TimelineTrackKind::Signal {
                    for clip in &track.clips {
                        let x =
                            lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
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
                        let x0 =
                            lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
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
                ui.label(
                    RichText::new(format!(
                        "{}  {}  {}  ({} clips)",
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
}

fn kind_label(kind: TimelineTrackKind) -> &'static str {
    match kind {
        TimelineTrackKind::Activation => "Activation",
        TimelineTrackKind::Animation => "Animation",
        TimelineTrackKind::Audio => "Audio",
        TimelineTrackKind::Transform => "Transform",
        TimelineTrackKind::Signal => "Signal",
        TimelineTrackKind::Control => "Control",
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
    if let Some(existing) = meta
        .tracks
        .iter_mut()
        .find(|t| t.name == track.name && t.kind == track.kind && t.binding == track.binding)
    {
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
