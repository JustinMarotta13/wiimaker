use eframe::egui;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::draw::DrawList;
use wiimaker_host::{flush_with_atlas, Framebuffer};
use wiimaker_scene::{
    pick_entity_at_with_catalog, pointer_to_scene, render_world, set_entity_rotation_z,
    set_entity_scale, set_entity_world_xy, Scene,
};

use crate::app::{EditTool, EditorApp, PlayMode, ViewportDrag};
use crate::theme;

pub(crate) const VIEW_W: usize = 640;
pub(crate) const VIEW_H: usize = 480;

impl EditorApp {
    pub(crate) fn ui_viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::central_frame())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Scene")
                            .strong()
                            .size(13.0)
                            .color(theme::TEXT),
                    );
                    let accent = egui::Rect::from_min_size(
                        egui::pos2(ui.cursor().left(), ui.cursor().center().y - 1.0),
                        egui::vec2(20.0, 2.0),
                    );
                    ui.painter()
                        .rect_filled(accent, egui::Rounding::same(1.0), theme::ACCENT);
                    ui.add_space(28.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{VIEW_W}×{VIEW_H}"))
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                        if self.play_mode != PlayMode::Edit {
                            ui.add_space(8.0);
                            let label = match self.play_mode {
                                PlayMode::Playing => "PLAYING",
                                PlayMode::Paused => "PAUSED",
                                PlayMode::Edit => "",
                            };
                            ui.label(
                                egui::RichText::new(label)
                                    .strong()
                                    .size(11.0)
                                    .color(theme::ACCENT),
                            );
                        }
                        ui.add_space(8.0);
                        ui.add(
                            egui::DragValue::new(&mut self.snap_size)
                                .range(1.0..=128.0)
                                .speed(1.0)
                                .prefix("grid "),
                        );
                        ui.checkbox(&mut self.snap_enabled, "Snap");
                        ui.add_space(8.0);
                        ui.add_enabled_ui(self.play_mode == PlayMode::Edit, |ui| {
                            ui.selectable_value(&mut self.edit_tool, EditTool::Rotate, "Rotate");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Scale, "Scale");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Translate, "Move");
                        });
                    });
                });
                ui.add_space(6.0);

                let mut draw = DrawList::new();
                render_world(&self.world, &mut draw, self.scene.clear_rgba());
                flush_with_atlas(&draw, &mut self.fb, Some(&self.atlas));

                let color_image =
                    egui::ColorImage::from_rgb([VIEW_W, VIEW_H], &fb_to_rgb(&self.fb));
                let tex = self.texture_handle.get_or_insert_with(|| {
                    ctx.load_texture("viewport", color_image.clone(), Default::default())
                });
                tex.set(color_image, Default::default());
                let tex_id = tex.id();

                let avail = ui.available_size();
                let scale = (avail.x / VIEW_W as f32)
                    .min(avail.y / VIEW_H as f32)
                    .min(1.0);
                let size = egui::vec2(VIEW_W as f32 * scale, VIEW_H as f32 * scale);

                // Sunken well + centered framed viewport.
                let well = ui.available_rect_before_wrap();
                ui.painter()
                    .rect_filled(well, egui::Rounding::same(6.0), theme::BG_SUNKEN);
                ui.painter().rect_stroke(
                    well,
                    egui::Rounding::same(6.0),
                    egui::Stroke::new(1.0, theme::BORDER_SOFT),
                );

                let pad_x = ((avail.x - size.x) * 0.5).max(0.0);
                let pad_y = ((avail.y - size.y) * 0.5).max(0.0);
                ui.add_space(pad_y);

                let mut viewport_response = None;
                ui.horizontal(|ui| {
                    ui.add_space(pad_x);
                    let frame_pad = 3.0;
                    let outer = egui::vec2(size.x + frame_pad * 2.0, size.y + frame_pad * 2.0);
                    let (outer_rect, _) = ui.allocate_exact_size(outer, egui::Sense::hover());
                    ui.painter().rect(
                        outer_rect,
                        egui::Rounding::same(4.0),
                        theme::BG_RAISED,
                        egui::Stroke::new(1.0, theme::BORDER),
                    );
                    let image_rect = egui::Rect::from_center_size(outer_rect.center(), size);
                    let image =
                        egui::Image::new((tex_id, size)).sense(egui::Sense::click_and_drag());
                    let response = ui.put(image_rect, image);
                    paint_selection_outline(
                        ui,
                        response.rect,
                        &self.scene,
                        &self.selected,
                        &self.catalog,
                    );
                    viewport_response = Some((response.clone(), response.rect));
                });
                if let Some((response, rect)) = viewport_response {
                    self.handle_viewport_input(&response, rect);
                }
            });
    }

    fn handle_viewport_input(&mut self, response: &egui::Response, rect: egui::Rect) {
        let to_scene = |pos: egui::Pos2| -> Option<[f32; 2]> {
            pointer_to_scene(
                [pos.x, pos.y],
                [rect.min.x, rect.min.y],
                [rect.width(), rect.height()],
                VIEW_W as f32,
                VIEW_H as f32,
            )
        };

        let pick_at = |app: &Self, pos: [f32; 2]| -> Option<(String, [f32; 2])> {
            let name =
                pick_entity_at_with_catalog(&app.scene, pos[0], pos[1], Some(&app.catalog))?;
            let world = app.scene.world_transform(&name)?;
            let grab_offset = [
                pos[0] - world.translation[0],
                pos[1] - world.translation[1],
            ];
            Some((name, grab_offset))
        };

        // Block authoring picks while playing.
        if self.play_mode != PlayMode::Edit {
            return;
        }

        // Click (no drag): select or clear (Cmd toggles).
        if response.clicked() {
            let cmd = response.ctx.input(|i| i.modifiers.command);
            if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                match pick_at(self, pos) {
                    Some((name, _)) if cmd => self.select_toggle(name),
                    Some((name, _)) => self.select(Some(name)),
                    None if !cmd => self.select(None),
                    None => {}
                }
            }
        }

        // Drag start: select hit entity and begin tool gesture.
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                match pick_at(self, pos) {
                    Some((name, grab_offset)) => {
                        if !self.is_selected(&name) {
                            self.select(Some(name.clone()));
                        }
                        let world = self
                            .scene
                            .world_transform(&name)
                            .or_else(|| self.scene.find_entity(&name).map(|e| e.transform.clone()));
                        let primary_start = world
                            .as_ref()
                            .map(|w| [w.translation[0], w.translation[1]])
                            .unwrap_or([pos[0], pos[1]]);
                        let scale_start = self
                            .scene
                            .find_entity(&name)
                            .map(|e| [e.transform.scale[0], e.transform.scale[1]])
                            .unwrap_or([1.0, 1.0]);
                        let dx0 = pos[0] - primary_start[0];
                        let dy0 = pos[1] - primary_start[1];
                        let dist_start = (dx0 * dx0 + dy0 * dy0).sqrt().max(1.0);
                        let angle_start = dy0.atan2(dx0);
                        let rot = self
                            .scene
                            .find_entity(&name)
                            .map(|e| e.transform.rotation)
                            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                        // quat z,w → angle (2D): atan2(2*z*w, w²-z²) simplified for x=y=0
                        let rot_z_start = (2.0 * rot[2] * rot[3])
                            .atan2(rot[3] * rot[3] - rot[2] * rot[2]);
                        self.push_undo();
                        let others_start: Vec<(String, [f32; 2])> = self
                            .selected
                            .iter()
                            .filter(|n| *n != &name)
                            .filter_map(|n| {
                                let w = self.scene.world_transform(n)?;
                                Some((n.clone(), [w.translation[0], w.translation[1]]))
                            })
                            .collect();
                        self.viewport_drag = Some(ViewportDrag {
                            entity: name,
                            grab_offset,
                            primary_start,
                            others_start,
                            scale_start,
                            dist_start,
                            angle_start,
                            rot_z_start,
                        });
                    }
                    None => {
                        self.select(None);
                        self.viewport_drag = None;
                    }
                }
            }
        }

        if let Some(drag) = self.viewport_drag.clone() {
            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                    match self.edit_tool {
                        EditTool::Translate => {
                            let mut x = pos[0] - drag.grab_offset[0];
                            let mut y = pos[1] - drag.grab_offset[1];
                            if self.snap_enabled && self.snap_size > 0.0 {
                                let g = self.snap_size;
                                x = (x / g).round() * g;
                                y = (y / g).round() * g;
                            }
                            let dx = x - drag.primary_start[0];
                            let dy = y - drag.primary_start[1];
                            let mut ok =
                                set_entity_world_xy(&mut self.scene, &drag.entity, x, y).is_ok();
                            for (other, start) in &drag.others_start {
                                if set_entity_world_xy(
                                    &mut self.scene,
                                    other,
                                    start[0] + dx,
                                    start[1] + dy,
                                )
                                .is_err()
                                {
                                    ok = false;
                                }
                            }
                            if ok {
                                self.mark_dirty();
                            }
                        }
                        EditTool::Scale => {
                            let dx = pos[0] - drag.primary_start[0];
                            let dy = pos[1] - drag.primary_start[1];
                            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                            let factor = (dist / drag.dist_start).clamp(0.05, 32.0);
                            let sx = (drag.scale_start[0] * factor).clamp(0.05, 32.0);
                            let sy = (drag.scale_start[1] * factor).clamp(0.05, 32.0);
                            if set_entity_scale(&mut self.scene, &drag.entity, sx, sy).is_ok() {
                                self.mark_dirty();
                            }
                        }
                        EditTool::Rotate => {
                            let dx = pos[0] - drag.primary_start[0];
                            let dy = pos[1] - drag.primary_start[1];
                            let angle = dy.atan2(dx);
                            let mut radians = drag.rot_z_start + (angle - drag.angle_start);
                            if self.snap_enabled {
                                let step = std::f32::consts::FRAC_PI_4; // 45°
                                radians = (radians / step).round() * step;
                            }
                            if set_entity_rotation_z(&mut self.scene, &drag.entity, radians).is_ok()
                            {
                                self.mark_dirty();
                            }
                        }
                    }
                }
            }
        }

        if response.drag_stopped() {
            if self.viewport_drag.is_some() {
                self.sync_baseline();
            }
            self.viewport_drag = None;
        }
    }
}

fn paint_selection_outline(
    ui: &egui::Ui,
    image_rect: egui::Rect,
    scene: &Scene,
    selected: &[String],
    catalog: &SpriteCatalog,
) {
    for name in selected {
        paint_one_outline(ui, image_rect, scene, name, catalog);
    }
}

fn paint_one_outline(
    ui: &egui::Ui,
    image_rect: egui::Rect,
    scene: &Scene,
    name: &str,
    catalog: &SpriteCatalog,
) {
    let Some(ent) = scene.entities.iter().find(|e| e.name == name) else {
        return;
    };
    let world = scene
        .world_transform(name)
        .unwrap_or_else(|| ent.transform.clone());

    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let stroke = egui::Stroke::new(1.5, theme::WARN_OUTLINE);
    let painter = ui.painter();

    if let Some(sp) = &ent.components.sprite {
        if sp.enabled {
            let pivot = catalog
                .lookup(&sp.texture)
                .map(|r| r.pivot)
                .unwrap_or([0.5, 0.5]);
            let w = sp.size[0] * world.scale[0];
            let h = sp.size[1] * world.scale[1];
            let left = world.translation[0] - w * pivot[0];
            let top = world.translation[1] - h * pivot[1];
            let r = egui::Rect::from_min_max(to_screen(left, top), to_screen(left + w, top + h));
            painter.rect_stroke(r, 0.0, stroke);
        }
    }
    if let Some(d) = &ent.components.disc {
        if d.enabled {
            let cx = world.translation[0];
            let cy = world.translation[1];
            let r_scene = d.radius * world.scale[0].max(world.scale[1]);
            let center = to_screen(cx, cy);
            let radius_px = r_scene / VIEW_W as f32 * image_rect.width();
            painter.circle_stroke(center, radius_px, stroke);
        }
    }
}

fn fb_to_rgb(fb: &Framebuffer) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(fb.pixels.len() * 3);
    for p in &fb.pixels {
        rgb.push(((p >> 16) & 0xff) as u8);
        rgb.push(((p >> 8) & 0xff) as u8);
        rgb.push((p & 0xff) as u8);
    }
    rgb
}
