//! Timeline window + Playable Director transport (dark Pro).

use eframe::egui::{self, Color32, Rect, RichText, Sense, Stroke, Vec2};
use wiimaker_assets::TimelineTrackKind;
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
                for clip in &track.clips {
                    let x0 = lane.left() + lane.width() * (clip.start / duration).clamp(0.0, 1.0);
                    let x1 = lane.left() + lane.width() * (clip.end / duration).clamp(0.0, 1.0);
                    let block = Rect::from_min_max(
                        egui::pos2(x0, lane.top() + 2.0),
                        egui::pos2(x1.max(x0 + 2.0), lane.bottom() - 2.0),
                    );
                    painter.rect_filled(block, 2.0, track_color(track.kind));
                }
            }
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
                let binding = track.binding.as_deref().unwrap_or("—");
                ui.label(
                    RichText::new(format!(
                        "{}  {:?}  {}  ({} clips)",
                        track.name,
                        track.kind,
                        binding,
                        track.clips.len()
                    ))
                    .size(12.0)
                    .color(theme::TEXT),
                );
            }
        });
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

fn track_color(kind: TimelineTrackKind) -> Color32 {
    match kind {
        TimelineTrackKind::Activation => Color32::from_rgb(76, 140, 196),
        TimelineTrackKind::Animation => Color32::from_rgb(196, 140, 64),
        TimelineTrackKind::Audio => Color32::from_rgb(72, 150, 110),
        TimelineTrackKind::Transform => Color32::from_rgb(150, 110, 186),
    }
}
