use eframe::egui;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::World;
use wiimaker_host::{flush_with_atlas, Framebuffer};
use wiimaker_scene::{
    fitted_blit_rect, pick_entity_at_with_catalog, pointer_to_scene, render_world_ex,
    scene_blit_rect, set_entity_rotation_z, set_entity_scale, set_entity_world_xy, tilemap_set_cell,
    GameViewAspect, GameViewPreset, Scene,
};

use crate::app::{CenterTab, EditTool, EditorApp, PlayMode, TilePaintDrag, ViewportDrag};
use crate::theme;

pub(crate) const VIEW_W: usize = 640;
pub(crate) const VIEW_H: usize = 480;

impl EditorApp {
    pub(crate) fn ui_viewport(&mut self, ui: &mut egui::Ui, tab: CenterTab) {
        let is_scene = tab == CenterTab::Scene;
        if is_scene {
            self.ui_scene_toolbar(ui);
        } else {
            self.ui_game_toolbar(ui);
        }
        ui.add_space(2.0);
        self.blit_framebuffer(ui, is_scene);
    }

    fn ui_scene_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let editing = self.play_mode == PlayMode::Edit;
            ui.add_enabled_ui(editing, |ui| {
                let tools = [
                    (EditTool::Translate, "Move", theme::ToolGlyph::Translate),
                    (EditTool::Scale, "Scale", theme::ToolGlyph::Scale),
                    (EditTool::Rotate, "Rotate", theme::ToolGlyph::Rotate),
                    (EditTool::Hand, "Hand (pan) · middle-drag also pans", theme::ToolGlyph::Hand),
                    (EditTool::Paint, "Paint", theme::ToolGlyph::Paint),
                    (EditTool::Erase, "Erase", theme::ToolGlyph::Erase),
                    (EditTool::Pick, "Pick", theme::ToolGlyph::Pick),
                ];
                for (tool, tip, glyph) in tools {
                    if theme::tool_toggle(ui, self.edit_tool == tool, tip, glyph).clicked() {
                        self.edit_tool = tool;
                    }
                }
            });
            // 2D is always-on (engine is 2D-only) — still show the control.
            let _ = theme::tool_toggle(ui, self.prefs.scene_view.mode_2d, "2D (always on)", theme::ToolGlyph::Mode2d);
            if theme::tool_toggle(
                ui,
                self.prefs.scene_view.grid_overlay,
                "Grid overlay",
                theme::ToolGlyph::Grid,
            )
            .clicked()
            {
                self.prefs.scene_view.grid_overlay = !self.prefs.scene_view.grid_overlay;
                self.persist_prefs();
            }
            if theme::tool_toggle(
                ui,
                self.prefs.scene_view.gizmos,
                "Gizmos",
                theme::ToolGlyph::Gizmo,
            )
            .clicked()
            {
                self.prefs.scene_view.gizmos = !self.prefs.scene_view.gizmos;
                self.persist_prefs();
            }
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
            if theme::enable_checkbox(ui, self.prefs.scene_view.snap).clicked() {
                self.prefs.scene_view.snap = !self.prefs.scene_view.snap;
                self.persist_prefs();
            }
            ui.label(egui::RichText::new("Snap").size(12.0).color(theme::TEXT));
            let mut snap = self.prefs.scene_view.snap_size;
            let snap_changed = ui
                .add(
                    egui::DragValue::new(&mut snap)
                        .range(1.0..=128.0)
                        .speed(1.0)
                        .prefix("grid "),
                )
                .changed();
            if snap_changed {
                self.prefs.scene_view.snap_size = snap;
                self.persist_prefs();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{VIEW_W}x{VIEW_H}"))
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                );
                let mut pct = (self.prefs.scene_view.zoom * 100.0).round();
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(10.0..=1600.0)
                            .speed(1.0)
                            .suffix("%")
                            .max_decimals(0),
                    )
                    .changed()
                {
                    self.prefs.scene_view.zoom = (pct / 100.0).clamp(0.1, 16.0);
                    self.persist_prefs();
                }
            });
        });
    }

    fn ui_game_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut preset = self.prefs.game_view.preset;
            let label = preset.as_str().to_string();
            egui::ComboBox::from_id_salt("game_view_preset")
                .selected_text(egui::RichText::new(label).size(12.0))
                .width(110.0)
                .show_ui(ui, |ui| {
                    for p in [
                        GameViewPreset::Free,
                        GameViewPreset::Res640x480,
                        GameViewPreset::Ratio16x9,
                        GameViewPreset::Ratio4x3,
                        GameViewPreset::Custom,
                    ] {
                        ui.selectable_value(&mut preset, p, p.as_str());
                    }
                });
            if preset != self.prefs.game_view.preset {
                let _ = wiimaker_scene::apply_game_view(
                    &mut self.prefs.game_view,
                    None,
                    None,
                    None,
                    Some(preset.as_str()),
                    None,
                );
                self.persist_prefs();
            }
            if self.prefs.game_view.preset == GameViewPreset::Custom {
                let mut w = self.prefs.game_view.width as f32;
                let mut h = self.prefs.game_view.height as f32;
                let cw = ui
                    .add(egui::DragValue::new(&mut w).range(16.0..=4096.0).prefix("W ").speed(1.0))
                    .changed();
                let ch = ui
                    .add(egui::DragValue::new(&mut h).range(16.0..=4096.0).prefix("H ").speed(1.0))
                    .changed();
                if cw || ch {
                    self.prefs.game_view.width = w as u32;
                    self.prefs.game_view.height = h as u32;
                    self.prefs.game_view.aspect = GameViewAspect::Fixed;
                    self.persist_prefs();
                }
            }
            ui.label(egui::RichText::new("Scale").size(12.0).color(theme::TEXT_MUTED));
            let mut scale = self.prefs.game_view.scale;
            if ui
                .add(egui::Slider::new(&mut scale, 0.1..=3.0).max_decimals(2))
                .changed()
            {
                self.prefs.game_view.scale = scale;
                self.persist_prefs();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let caption = if self.prefs.game_view.is_free() {
                    format!("Free · {VIEW_W}x{VIEW_H}")
                } else {
                    format!(
                        "{} · {}x{}",
                        self.prefs.game_view.preset.as_str(),
                        self.prefs.game_view.width,
                        self.prefs.game_view.height
                    )
                };
                ui.label(
                    egui::RichText::new(caption)
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

    /// Scene: zoom/pan stretch of the 640×480 fb. Game: aspect-fit + letterbox.
    fn blit_framebuffer(&mut self, ui: &mut egui::Ui, is_scene: bool) {
        let ctx = ui.ctx().clone();
        let mut draw = DrawList::new();
        // Scene tab stays world-space (camera rect gizmo). Game tab applies the active camera.
        render_world_ex(&self.world, &mut draw, self.scene.clear_rgba(), !is_scene);
        flush_with_atlas(&draw, &mut self.fb, Some(&self.atlas));

        let rgb = fb_to_rgb(&self.fb);
        debug_assert_eq!(self.fb.width, VIEW_W);
        debug_assert_eq!(self.fb.height, VIEW_H);
        debug_assert_eq!(rgb.len(), VIEW_W * VIEW_H * 3);
        let color_image = egui::ColorImage::from_rgb([VIEW_W, VIEW_H], &rgb);
        let tex_filter = egui::TextureOptions::NEAREST;
        let tex = self
            .texture_handle
            .get_or_insert_with(|| ctx.load_texture("viewport", color_image.clone(), tex_filter));
        tex.set(color_image, tex_filter);

        let well_size = ui.available_size();
        let (well, _) = ui.allocate_exact_size(well_size, egui::Sense::hover());
        ui.set_clip_rect(ui.clip_rect().intersect(well));

        let painter = ui.painter_at(well);
        painter.rect_filled(well, 0.0, theme::BG_DEEP);

        let image_rect = if is_scene {
            let sv = &self.prefs.scene_view;
            let (x, y, w, h) =
                scene_blit_rect(well.width(), well.height(), sv.zoom, sv.pan_x, sv.pan_y);
            egui::Rect::from_min_size(well.min + egui::vec2(x, y), egui::vec2(w, h))
        } else {
            let gv = &self.prefs.game_view;
            let (aw, ah) = gv.aspect_wh(well.width(), well.height());
            let (x, y, w, h) =
                fitted_blit_rect(well.width(), well.height(), aw, ah, gv.scale);
            egui::Rect::from_min_size(well.min + egui::vec2(x, y), egui::vec2(w, h))
        };

        painter.image(
            tex.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        let sense = if is_scene {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::hover()
        };
        let interact_rect = if is_scene && self.edit_tool == EditTool::Hand {
            well
        } else {
            image_rect
        };
        let response = ui.interact(interact_rect, ui.id().with("viewport_fb"), sense);
        let well_resp = ui.interact(well, ui.id().with("viewport_well"), egui::Sense::hover());

        if is_scene {
            if self.prefs.scene_view.grid_overlay {
                paint_grid_overlay(
                    ui,
                    image_rect,
                    self.prefs.scene_view.snap_size.max(1.0),
                );
            }
            if self.prefs.scene_view.gizmos {
                paint_selection_outline(ui, image_rect, &self.scene, &self.selected, &self.catalog);
                paint_collider_gizmos(ui, image_rect, &self.scene, &self.selected);
                paint_camera_gizmos(ui, image_rect, &self.world);
            }
            if self.edit_tool.is_tile_tool() {
                if let Some(name) = self.tilemap_target() {
                    paint_tilemap_overlay(ui, image_rect, &self.scene, &name);
                }
            }
            self.handle_scene_nav(&well_resp, well);
            self.handle_viewport_input(&response, image_rect);
        } else if self.play_mode == PlayMode::Edit {
            ui.painter().text(
                image_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Play to simulate  ·  WASD moves Player",
                egui::FontId::proportional(13.0),
                theme::TEXT_DIM,
            );
        }
    }

    fn handle_scene_nav(&mut self, well_resp: &egui::Response, well: egui::Rect) {
        if self.play_mode != PlayMode::Edit {
            return;
        }
        let hovered = well_resp.hovered() || well_resp.contains_pointer();
        if !hovered {
            return;
        }

        let (scroll_y, middle, delta, pointer) = well_resp.ctx.input(|i| {
            (
                i.smooth_scroll_delta.y,
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.delta(),
                i.pointer.hover_pos(),
            )
        });

        if scroll_y.abs() > 0.1 {
            let old = self.prefs.scene_view.zoom;
            let factor = if scroll_y > 0.0 { 1.1 } else { 1.0 / 1.1 };
            let new = (old * factor).clamp(0.1, 16.0);
            if (new - old).abs() > 1e-5 {
                if let Some(p) = pointer {
                    let k = new / old;
                    let pc = p - well.center();
                    let pan = egui::vec2(self.prefs.scene_view.pan_x, self.prefs.scene_view.pan_y);
                    let new_pan = pc * (1.0 - k) + pan * k;
                    self.prefs.scene_view.pan_x = new_pan.x;
                    self.prefs.scene_view.pan_y = new_pan.y;
                }
                self.prefs.scene_view.zoom = new;
                self.persist_prefs();
            }
        }

        if middle && (delta.x != 0.0 || delta.y != 0.0) {
            self.prefs.scene_view.pan_x += delta.x;
            self.prefs.scene_view.pan_y += delta.y;
            self.persist_prefs();
        }
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

        if self.edit_tool == EditTool::Hand {
            if response.dragged() {
                let d = response.drag_delta();
                if d.x != 0.0 || d.y != 0.0 {
                    self.prefs.scene_view.pan_x += d.x;
                    self.prefs.scene_view.pan_y += d.y;
                    self.persist_prefs();
                }
            }
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
                            if self.prefs.scene_view.snap && self.prefs.scene_view.snap_size > 0.0 {
                                let g = self.prefs.scene_view.snap_size;
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
                            if self.prefs.scene_view.snap {
                                let step = std::f32::consts::FRAC_PI_4; // 45°
                                radians = (radians / step).round() * step;
                            }
                            if set_entity_rotation_z(&mut self.scene, &drag.entity, radians).is_ok()
                            {
                                self.mark_dirty();
                            }
                        }
                        EditTool::Paint | EditTool::Erase | EditTool::Pick | EditTool::Hand => {}
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

fn paint_grid_overlay(ui: &egui::Ui, image_rect: egui::Rect, step: f32) {
    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let painter = ui.painter();
    let minor = egui::Color32::from_rgba_unmultiplied(70, 70, 70, 90);
    let major = egui::Color32::from_rgba_unmultiplied(110, 110, 110, 140);
    let step = step.max(1.0);
    let mut x = 0.0;
    let mut i = 0u32;
    while x <= VIEW_W as f32 + 0.01 {
        let stroke = if i % 5 == 0 {
            egui::Stroke::new(1.0_f32, major)
        } else {
            egui::Stroke::new(1.0_f32, minor)
        };
        painter.line_segment([to_screen(x, 0.0), to_screen(x, VIEW_H as f32)], stroke);
        x += step;
        i += 1;
        if i > 512 {
            break;
        }
    }
    let mut y = 0.0;
    i = 0;
    while y <= VIEW_H as f32 + 0.01 {
        let stroke = if i % 5 == 0 {
            egui::Stroke::new(1.0_f32, major)
        } else {
            egui::Stroke::new(1.0_f32, minor)
        };
        painter.line_segment([to_screen(0.0, y), to_screen(VIEW_W as f32, y)], stroke);
        y += step;
        i += 1;
        if i > 512 {
            break;
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

/// Collider gizmos: accent outline of the AABB or circle (amber when `trigger`).
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
        // Triggers draw amber/yellow; solid walls keep seafoam/accent blue.
        let color = if c.trigger {
            egui::Color32::from_rgb(220, 180, 60)
        } else if selected {
            theme::ACCENT
        } else {
            theme::ACCENT_DIM
        };
        let stroke = if selected {
            egui::Stroke::new(2.0_f32, color)
        } else {
            egui::Stroke::new(1.0_f32, color)
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

/// Unity-ish camera frustum: 640×480 rect centered on an active Camera's world pose.
fn paint_camera_gizmos(ui: &egui::Ui, image_rect: egui::Rect, world: &World) {
    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let painter = ui.painter();
    let color = egui::Color32::from_rgb(140, 210, 230);
    let stroke = egui::Stroke::new(1.5_f32, color);
    let Some((_id, xf, _cam)) = world.active_camera() else {
        return;
    };
    let cx = xf.translation.x;
    let cy = xf.translation.y;
    let hw = VIEW_W as f32 * 0.5;
    let hh = VIEW_H as f32 * 0.5;
    let r = egui::Rect::from_min_max(to_screen(cx - hw, cy - hh), to_screen(cx + hw, cy + hh));
    painter.rect_stroke(r, 0.0, stroke);
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
