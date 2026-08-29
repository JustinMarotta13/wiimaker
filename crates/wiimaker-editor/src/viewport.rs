use eframe::egui;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::draw::DrawList;
use wiimaker_host::{flush_with_atlas, Framebuffer};
use wiimaker_scene::{
    pick_entity_at_with_catalog, pointer_to_scene, render_world, set_entity_rotation_z,
    set_entity_scale, set_entity_world_xy, tilemap_set_cell, Scene,
};

use crate::app::{CenterTab, EditTool, EditorApp, PlayMode, TilePaintDrag, ViewportDrag};
use crate::theme;

pub(crate) const VIEW_W: usize = 640;
pub(crate) const VIEW_H: usize = 480;

impl EditorApp {
    pub(crate) fn ui_viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::central_frame())
            .show(ctx, |ui| {
                self.center_tab = theme::dock_tabs(
                    ui,
                    &[("Scene", CenterTab::Scene), ("Game", CenterTab::Game)],
                    self.center_tab,
                );

                let is_scene = self.center_tab == CenterTab::Scene;
                if is_scene {
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(self.play_mode == PlayMode::Edit, |ui| {
                            ui.selectable_value(&mut self.edit_tool, EditTool::Translate, "Move");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Scale, "Scale");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Rotate, "Rotate");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Paint, "Paint");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Erase, "Erase");
                            ui.selectable_value(&mut self.edit_tool, EditTool::Pick, "Pick");
                        });
                        if self.edit_tool.is_tile_tool() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "brush {}{}",
                                    self.tile_brush_id,
                                    if self.tile_brush_solid { " solid" } else { "" }
                                ))
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                            );
                        }
                        ui.checkbox(&mut self.snap_enabled, "Snap");
                        ui.add(
                            egui::DragValue::new(&mut self.snap_size)
                                .range(1.0..=128.0)
                                .speed(1.0)
                                .prefix("grid "),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{VIEW_W}×{VIEW_H}"))
                                    .size(11.0)
                                    .color(theme::TEXT_DIM),
                            );
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Game")
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{VIEW_W}×{VIEW_H}"))
                                    .size(11.0)
                                    .color(theme::TEXT_DIM),
                            );
                            if self.play_mode != PlayMode::Edit {
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
                        });
                    });
                }
                ui.add_space(4.0);

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

                let well = ui.available_rect_before_wrap();
                ui.painter()
                    .rect_filled(well, egui::Rounding::same(2.0), theme::BG_SUNKEN);
                ui.painter().rect_stroke(
                    well,
                    egui::Rounding::same(2.0),
                    egui::Stroke::new(1.0, theme::BORDER),
                );

                let pad_x = ((avail.x - size.x) * 0.5).max(0.0);
                let pad_y = ((avail.y - size.y) * 0.5).max(0.0);
                ui.add_space(pad_y);

                let mut viewport_response = None;
                ui.horizontal(|ui| {
                    ui.add_space(pad_x);
                    let frame_pad = 2.0;
                    let outer = egui::vec2(size.x + frame_pad * 2.0, size.y + frame_pad * 2.0);
                    let (outer_rect, _) = ui.allocate_exact_size(outer, egui::Sense::hover());
                    ui.painter().rect(
                        outer_rect,
                        egui::Rounding::same(2.0),
                        theme::BG_RAISED,
                        egui::Stroke::new(1.0, theme::BORDER),
                    );
                    let image_rect = egui::Rect::from_center_size(outer_rect.center(), size);
                    let sense = if is_scene {
                        egui::Sense::click_and_drag()
                    } else {
                        egui::Sense::hover()
                    };
                    let image = egui::Image::new((tex_id, size)).sense(sense);
                    let response = ui.put(image_rect, image);
                    if is_scene {
                        paint_selection_outline(
                            ui,
                            response.rect,
                            &self.scene,
                            &self.selected,
                            &self.catalog,
                        );
                        paint_collider_gizmos(ui, response.rect, &self.scene, &self.selected);
                        if self.edit_tool.is_tile_tool() {
                            if let Some(name) = self.tilemap_target() {
                                paint_tilemap_overlay(ui, response.rect, &self.scene, &name);
                            }
                        }
                        viewport_response = Some((response.clone(), response.rect));
                    } else if self.play_mode == PlayMode::Edit {
                        ui.painter().text(
                            response.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Play to simulate  ·  WASD moves Player",
                            egui::FontId::proportional(13.0),
                            theme::TEXT_DIM,
                        );
                    }
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
            let name = pick_entity_at_with_catalog(&app.scene, pos[0], pos[1], Some(&app.catalog))?;
            let world = app.scene.world_transform(&name)?;
            let grab_offset = [pos[0] - world.translation[0], pos[1] - world.translation[1]];
            Some((name, grab_offset))
        };

        // Block authoring picks while playing.
        if self.play_mode != PlayMode::Edit {
            return;
        }

        if self.edit_tool.is_tile_tool() {
            self.handle_tile_paint(response, to_scene);
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
                        let rot_z_start =
                            (2.0 * rot[2] * rot[3]).atan2(rot[3] * rot[3] - rot[2] * rot[2]);
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
                        EditTool::Paint | EditTool::Erase | EditTool::Pick => {}
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

    fn handle_tile_paint(
        &mut self,
        response: &egui::Response,
        to_scene: impl Fn(egui::Pos2) -> Option<[f32; 2]>,
    ) {
        let scene_pos = response.interact_pointer_pos().and_then(&to_scene);

        let target_at = |app: &Self, pos: [f32; 2]| -> Option<String> {
            if let Some(name) = app.tilemap_target() {
                if let Some(ent) = app.scene.find_entity(&name) {
                    if let Some(tm) = &ent.components.tilemap {
                        let world = app
                            .scene
                            .world_transform(&name)
                            .unwrap_or_else(|| ent.transform.clone());
                        let (cx, cy) = tm.world_to_cell(&world, pos[0], pos[1]);
                        if tm.in_bounds(cx, cy) {
                            return Some(name);
                        }
                    }
                }
            }
            pick_entity_at_with_catalog(&app.scene, pos[0], pos[1], Some(&app.catalog)).and_then(
                |name| {
                    app.scene
                        .find_entity(&name)
                        .and_then(|e| e.components.tilemap.as_ref())
                        .map(|_| name)
                },
            )
        };

        if response.clicked() && self.edit_tool == EditTool::Pick {
            if let Some(pos) = scene_pos {
                if let Some(name) = target_at(self, pos) {
                    let picked = self.scene.find_entity(&name).and_then(|ent| {
                        let tm = ent.components.tilemap.as_ref()?;
                        let world = self
                            .scene
                            .world_transform(&name)
                            .unwrap_or_else(|| ent.transform.clone());
                        let (cx, cy) = tm.world_to_cell(&world, pos[0], pos[1]);
                        let (id, solid) = tm.get(cx, cy);
                        Some((cx, cy, id, solid))
                    });
                    if let Some((cx, cy, id, solid)) = picked {
                        self.tile_brush_id = if id == 0 { 1 } else { id };
                        self.tile_brush_solid = if id == 0 { true } else { solid };
                        self.select(Some(name));
                        self.status = format!("picked tile {id} solid={solid} @ ({cx},{cy})");
                    }
                }
            }
            return;
        }

        let erase =
            self.edit_tool == EditTool::Erase || response.ctx.input(|i| i.pointer.secondary_down());
        let painting = self.edit_tool == EditTool::Paint || self.edit_tool == EditTool::Erase;

        if painting && (response.drag_started() || response.clicked()) {
            if let Some(pos) = scene_pos {
                if let Some(name) = target_at(self, pos) {
                    if !self.is_selected(&name) {
                        self.select(Some(name.clone()));
                    }
                    self.push_undo();
                    self.tile_paint = Some(TilePaintDrag {
                        entity: name,
                        last: None,
                    });
                }
            }
        }

        if painting {
            if let Some(drag) = self.tile_paint.clone() {
                if response.dragged() || response.clicked() {
                    if let Some(pos) = scene_pos {
                        let cell = self.scene.find_entity(&drag.entity).and_then(|ent| {
                            let tm = ent.components.tilemap.as_ref()?;
                            let world = self
                                .scene
                                .world_transform(&drag.entity)
                                .unwrap_or_else(|| ent.transform.clone());
                            let (cx, cy) = tm.world_to_cell(&world, pos[0], pos[1]);
                            if tm.in_bounds(cx, cy) {
                                Some((cx, cy))
                            } else {
                                None
                            }
                        });
                        if let Some((cx, cy)) = cell {
                            if Some((cx, cy)) != drag.last {
                                let id = if erase { 0 } else { self.tile_brush_id };
                                let solid = if erase { false } else { self.tile_brush_solid };
                                if tilemap_set_cell(
                                    &mut self.scene,
                                    &drag.entity,
                                    cx,
                                    cy,
                                    id,
                                    solid,
                                )
                                .is_ok()
                                {
                                    if let Some(d) = self.tile_paint.as_mut() {
                                        d.last = Some((cx, cy));
                                    }
                                    self.mark_dirty();
                                }
                            }
                        }
                    }
                }
            }
        }

        if response.drag_stopped() || (response.clicked() && self.tile_paint.is_some()) {
            if self.tile_paint.is_some() {
                self.sync_baseline();
            }
            self.tile_paint = None;
        }
    }
}

fn paint_tilemap_overlay(ui: &egui::Ui, image_rect: egui::Rect, scene: &Scene, name: &str) {
    let Some(ent) = scene.find_entity(name) else {
        return;
    };
    let Some(tm) = &ent.components.tilemap else {
        return;
    };
    let world = scene
        .world_transform(name)
        .unwrap_or_else(|| ent.transform.clone());
    let (origin, size) = tm.world_rect(&world);
    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let r = egui::Rect::from_min_max(
        to_screen(origin[0], origin[1]),
        to_screen(origin[0] + size[0], origin[1] + size[1]),
    );
    ui.painter()
        .rect_stroke(r, 0.0, egui::Stroke::new(1.0, theme::ACCENT));
}

/// Collider gizmos: seafoam outline of the AABB or circle.
/// All enabled colliders get a 1px stroke; the selection is 2px so walls read in screenshots.
fn paint_collider_gizmos(
    ui: &egui::Ui,
    image_rect: egui::Rect,
    scene: &Scene,
    selected: &[String],
) {
    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let painter = ui.painter();
    for ent in &scene.entities {
        let Some(c) = &ent.components.collider else {
            continue;
        };
        if !c.enabled {
            continue;
        }
        let world = scene
            .world_transform(&ent.name)
            .unwrap_or_else(|| ent.transform.clone());
        let selected = selected.iter().any(|n| n == &ent.name);
        let stroke = if selected {
            egui::Stroke::new(2.0_f32, theme::ACCENT)
        } else {
            egui::Stroke::new(1.0_f32, theme::ACCENT_DIM)
        };
        match c.kind {
            wiimaker_scene::SceneColliderKind::Aabb => {
                let (min, max) = c.world_aabb(&world);
                let r =
                    egui::Rect::from_min_max(to_screen(min[0], min[1]), to_screen(max[0], max[1]));
                painter.rect_stroke(r, 0.0, stroke);
            }
            wiimaker_scene::SceneColliderKind::Circle => {
                let center = c.world_center(&world);
                let radius = c.world_radius(&world).unwrap_or(0.0);
                let radius_px = radius / VIEW_W as f32 * image_rect.width();
                painter.circle_stroke(to_screen(center[0], center[1]), radius_px, stroke);
            }
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
    if let Some(tm) = &ent.components.tilemap {
        if tm.enabled {
            let (origin, size) = tm.world_rect(&world);
            let r = egui::Rect::from_min_max(
                to_screen(origin[0], origin[1]),
                to_screen(origin[0] + size[0], origin[1] + size[1]),
            );
            painter.rect_stroke(r, 0.0, stroke);
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
