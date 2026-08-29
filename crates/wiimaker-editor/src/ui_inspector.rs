use eframe::egui::{self, RichText};
use wiimaker_scene::{
    add_component_collider, add_component_disc, add_component_sprite, add_component_tilemap,
    remove_component_collider, remove_component_disc, remove_component_sprite,
    remove_component_tilemap, save_project, set_component_enabled, set_entity_parent,
    tilemap_resize, SceneColliderKind,
};

use crate::app::EditorApp;
use crate::theme;
use crate::ui_project::{file_kind_label, format_bytes};

impl EditorApp {
    pub(crate) fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        // Keep widgets inside the pinned panel width (sliders/labels otherwise expand PanelState).
        let slider_w = (ui.available_width() - 72.0).clamp(96.0, 180.0);
        ui.spacing_mut().slider_width = slider_w;

        theme::section_header(ui, "Inspector");
        egui::ScrollArea::vertical()
            .id_salt("inspector_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.ui_inspector_body(ui);
            });
    }

    fn ui_inspector_body(&mut self, ui: &mut egui::Ui) {
        if self.selected_file.is_some() {
            self.ui_inspector_file(ui);
            return;
        }
        let Some(sel) = self.primary_selected().map(|s| s.to_string()) else {
            theme::card_frame().show(ui, |ui| {
                theme::muted(
                    ui,
                    "Select an entity in the Hierarchy / Scene, or a file in Project",
                );
            });
            return;
        };
        if self.selected.len() > 1 {
            theme::meta_chip(
                ui,
                "selection",
                &format!("{} entities", self.selected.len()),
            );
            theme::muted(
                ui,
                &format!("Editing primary · {}", self.selected.join(", ")),
            );
            ui.add_space(4.0);
        }
        let mut dirty = false;
        let mut add_sprite = false;
        let mut add_disc = false;
        let mut add_tilemap = false;
        let mut add_collider = false;
        let mut remove_sprite = false;
        let mut remove_disc = false;
        let mut remove_tilemap = false;
        let mut remove_collider = false;
        let mut toggle_sprite: Option<bool> = None;
        let mut toggle_disc: Option<bool> = None;
        let mut toggle_tilemap: Option<bool> = None;
        let mut toggle_collider: Option<bool> = None;
        let mut pending_tm_resize: Option<(u32, u32)> = None;
        let mut use_brush: Option<(u16, bool)> = None;
        let mut rename_committed = false;
        let mut unparent = false;

        let parent_name = self.scene.find_entity(&sel).and_then(|e| e.parent.clone());

        theme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Name").size(12.0).color(theme::TEXT_MUTED));
                let name_w = (ui.available_width() - 48.0).clamp(80.0, 200.0);
                let resp = ui
                    .add(egui::TextEdit::singleline(&mut self.rename_draft).desired_width(name_w));
                if resp.lost_focus() {
                    rename_committed = true;
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Parent").size(12.0).color(theme::TEXT_MUTED));
                match &parent_name {
                    Some(p) => {
                        ui.label(RichText::new(p).color(theme::TEXT));
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Unparent").size(11.0))
                                    .fill(theme::BG_SUNKEN),
                            )
                            .clicked()
                        {
                            unparent = true;
                        }
                    }
                    None => {
                        ui.label(RichText::new("Scene (root)").color(theme::TEXT_DIM));
                    }
                }
            });
        });
        if rename_committed {
            self.commit_rename();
        }
        if unparent {
            self.push_undo();
            if set_entity_parent(&mut self.scene, &sel, None).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }

        let catalog_names: Vec<String> = self.catalog.names().to_vec();
        let mut pending_sprite_tex: Option<(String, [f32; 2])> = None;

        if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
            let xform_label = if ent.parent.is_some() {
                "Local Transform"
            } else {
                "Transform"
            };
            ui.add_space(8.0);
            ui.label(
                RichText::new(xform_label)
                    .strong()
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(4.0);
            theme::card_frame().show(ui, |ui| {
                let mut changed = false;
                changed |= ui
                    .add(
                        egui::Slider::new(&mut ent.transform.translation[0], -640.0..=640.0)
                            .text("x"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut ent.transform.translation[1], -480.0..=480.0)
                            .text("y"),
                    )
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut ent.transform.scale[0], 0.1..=8.0).text("scale x"))
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut ent.transform.scale[1], 0.1..=8.0).text("scale y"))
                    .changed();
                if changed {
                    dirty = true;
                }
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new("Components")
                    .strong()
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(4.0);
            if let Some(sp) = ent.components.sprite.as_mut() {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = sp.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            toggle_sprite = Some(en);
                        }
                        ui.label(RichText::new("Sprite").strong().color(theme::ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Remove").size(11.0).color(theme::DANGER),
                                    )
                                    .fill(theme::BG_SUNKEN),
                                )
                                .clicked()
                            {
                                remove_sprite = true;
                            }
                        });
                    });
                    let catalog_names = &catalog_names;
                    let mut tex_changed = false;
                    let mut new_tex = sp.texture.clone();
                    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                    egui::ComboBox::from_id_salt("sprite_tex")
                        .selected_text(&sp.texture)
                        .width(combo_w)
                        .show_ui(ui, |ui| {
                            for name in catalog_names {
                                if ui.selectable_label(sp.texture == *name, name).clicked() {
                                    new_tex = name.clone();
                                    tex_changed = true;
                                }
                            }
                        });
                    if tex_changed {
                        // Size looked up after borrow ends via pending.
                        pending_sprite_tex = Some((new_tex, [0.0, 0.0]));
                    }
                    dirty |= ui
                        .add(egui::Slider::new(&mut sp.size[0], 1.0..=256.0).text("w"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut sp.size[1], 1.0..=256.0).text("h"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut sp.z, -10.0..=10.0).text("z"))
                        .changed();
                });
                ui.add_space(4.0);
            } else if ui.button("+ Sprite").clicked() {
                add_sprite = true;
            }

            if let Some(d) = ent.components.disc.as_mut() {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = d.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            toggle_disc = Some(en);
                        }
                        ui.label(RichText::new("Disc").strong().color(theme::ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Remove").size(11.0).color(theme::DANGER),
                                    )
                                    .fill(theme::BG_SUNKEN),
                                )
                                .clicked()
                            {
                                remove_disc = true;
                            }
                        });
                    });
                    dirty |= ui
                        .add(egui::Slider::new(&mut d.radius, 1.0..=200.0).text("radius"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut d.z, -10.0..=10.0).text("z"))
                        .changed();
                });
            } else if ui.button("+ Disc").clicked() {
                add_disc = true;
            }

            if let Some(tm) = ent.components.tilemap.as_mut() {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = tm.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            toggle_tilemap = Some(en);
                        }
                        ui.label(RichText::new("Tilemap").strong().color(theme::ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Remove").size(11.0).color(theme::DANGER),
                                    )
                                    .fill(theme::BG_SUNKEN),
                                )
                                .clicked()
                            {
                                remove_tilemap = true;
                            }
                        });
                    });
                    let mut w = tm.width;
                    let mut h = tm.height;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("grid").size(11.0).color(theme::TEXT_MUTED));
                        let w_ch = ui.add(egui::DragValue::new(&mut w).range(1..=256).prefix("w "));
                        let h_ch = ui.add(egui::DragValue::new(&mut h).range(1..=256).prefix("h "));
                        if (w_ch.changed() || h_ch.changed()) && (w != tm.width || h != tm.height) {
                            pending_tm_resize = Some((w, h));
                        }
                    });
                    dirty |= ui
                        .add(egui::Slider::new(&mut tm.cell, 4.0..=64.0).text("cell"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut tm.origin[0], -640.0..=640.0).text("origin x"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut tm.origin[1], -480.0..=480.0).text("origin y"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut tm.z, -10.0..=10.0).text("z"))
                        .changed();
                    let occupied = tm.cells.iter().filter(|c| **c != 0).count();
                    ui.label(
                        RichText::new(format!(
                            "{occupied} occupied · {} solid",
                            tm.solid.iter().filter(|s| **s != 0).count()
                        ))
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new("Palette").size(12.0).color(theme::TEXT_MUTED));
                    let catalog_names = &catalog_names;
                    for pal in tm.palette.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut pal.id)
                                    .range(1..=32)
                                    .prefix("id "),
                            );
                            let mut col = egui::Color32::from_rgba_unmultiplied(
                                pal.color[0],
                                pal.color[1],
                                pal.color[2],
                                pal.color[3],
                            );
                            if ui.color_edit_button_srgba(&mut col).changed() {
                                pal.color = [col.r(), col.g(), col.b(), col.a()];
                                dirty = true;
                            }
                            if ui.small_button("Brush").clicked() {
                                use_brush = Some((pal.id, true));
                            }
                        });
                        let mut sprite = pal.sprite.clone().unwrap_or_default();
                        let combo_w = (ui.available_width() - 8.0).clamp(80.0, 200.0);
                        let mut tex_changed = false;
                        egui::ComboBox::from_id_salt(format!("tm_pal_{}", pal.id))
                            .selected_text(if sprite.is_empty() {
                                "(color quad)"
                            } else {
                                sprite.as_str()
                            })
                            .width(combo_w)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(sprite.is_empty(), "(color quad)")
                                    .clicked()
                                {
                                    sprite.clear();
                                    tex_changed = true;
                                }
                                for name in catalog_names {
                                    if ui.selectable_label(sprite == *name, name).clicked() {
                                        sprite = name.clone();
                                        tex_changed = true;
                                    }
                                }
                            });
                        if tex_changed {
                            pal.sprite = if sprite.is_empty() {
                                None
                            } else {
                                Some(sprite)
                            };
                            dirty = true;
                        }
                    }
                    if ui.small_button("+ palette id").clicked() {
                        let next = tm.palette.iter().map(|p| p.id).max().unwrap_or(0) + 1;
                        tm.palette.push(wiimaker_scene::SceneTilePalette {
                            id: next,
                            sprite: None,
                            color: [48, 88, 176, 255],
                        });
                        dirty = true;
                    }
                });
            } else if ui.button("+ Tilemap").clicked() {
                add_tilemap = true;
            }

            if let Some(c) = ent.components.collider.as_mut() {
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = c.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            toggle_collider = Some(en);
                        }
                        ui.label(RichText::new("Collider").strong().color(theme::ACCENT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Remove").size(11.0).color(theme::DANGER),
                                    )
                                    .fill(theme::BG_SUNKEN),
                                )
                                .clicked()
                            {
                                remove_collider = true;
                            }
                        });
                    });
                    let kind_label = match c.kind {
                        SceneColliderKind::Aabb => "Aabb",
                        SceneColliderKind::Circle => "Circle",
                    };
                    egui::ComboBox::from_id_salt("collider_kind")
                        .selected_text(kind_label)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(c.kind == SceneColliderKind::Aabb, "Aabb")
                                .clicked()
                            {
                                c.kind = SceneColliderKind::Aabb;
                                dirty = true;
                            }
                            if ui
                                .selectable_label(c.kind == SceneColliderKind::Circle, "Circle")
                                .clicked()
                            {
                                c.kind = SceneColliderKind::Circle;
                                dirty = true;
                            }
                        });
                    match c.kind {
                        SceneColliderKind::Aabb => {
                            dirty |= ui
                                .add(egui::Slider::new(&mut c.size[0], 1.0..=256.0).text("w"))
                                .changed();
                            dirty |= ui
                                .add(egui::Slider::new(&mut c.size[1], 1.0..=256.0).text("h"))
                                .changed();
                        }
                        SceneColliderKind::Circle => {
                            dirty |= ui
                                .add(egui::Slider::new(&mut c.radius, 1.0..=200.0).text("radius"))
                                .changed();
                        }
                    }
                    dirty |= ui.checkbox(&mut c.solid, "solid").changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut c.offset[0], -128.0..=128.0).text("offset x"))
                        .changed();
                    dirty |= ui
                        .add(egui::Slider::new(&mut c.offset[1], -128.0..=128.0).text("offset y"))
                        .changed();
                });
            } else if ui.button("+ Collider").clicked() {
                add_collider = true;
            }
        }

        if let Some((tex, _)) = pending_sprite_tex {
            let size = self
                .catalog
                .lookup(&tex)
                .map(|r| r.pixel_size)
                .unwrap_or([32.0, 32.0]);
            if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                if let Some(sp) = ent.components.sprite.as_mut() {
                    sp.texture = tex;
                    sp.size = size;
                    dirty = true;
                }
            }
        }

        if dirty {
            self.begin_inspector_gesture();
            self.mark_dirty();
        }
        if remove_sprite {
            self.push_undo();
            if remove_component_sprite(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if remove_disc {
            self.push_undo();
            if remove_component_disc(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_sprite {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "sprite", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_disc {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "disc", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_sprite {
            let tex = self
                .catalog
                .names()
                .first()
                .cloned()
                .or_else(|| self.asset_names.first().cloned())
                .unwrap_or_else(|| "missing".into());
            let size = self
                .catalog
                .lookup(&tex)
                .map(|r| r.pixel_size)
                .unwrap_or([32.0, 32.0]);
            self.push_undo();
            let _ = add_component_sprite(&mut self.scene, &sel, &tex, size);
            self.sync_baseline();
            self.mark_dirty();
        }
        if add_disc {
            self.push_undo();
            let _ = add_component_disc(&mut self.scene, &sel, 36.0, [72, 210, 160, 255]);
            self.sync_baseline();
            self.mark_dirty();
        }
        if add_tilemap {
            self.push_undo();
            let _ = add_component_tilemap(&mut self.scene, &sel, 32, 18, 16.0);
            self.edit_tool = crate::app::EditTool::Paint;
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_tilemap {
            self.push_undo();
            if remove_component_tilemap(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_tilemap {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "tilemap", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_collider {
            let (kind, size, radius) = self
                .scene
                .find_entity(&sel)
                .map(|e| {
                    if let Some(sp) = &e.components.sprite {
                        (SceneColliderKind::Aabb, sp.size, 16.0)
                    } else if let Some(d) = &e.components.disc {
                        (SceneColliderKind::Circle, [32.0, 32.0], d.radius)
                    } else {
                        (SceneColliderKind::Aabb, [32.0, 32.0], 16.0)
                    }
                })
                .unwrap_or((SceneColliderKind::Aabb, [32.0, 32.0], 16.0));
            self.push_undo();
            let _ = add_component_collider(&mut self.scene, &sel, kind, size, radius, true);
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_collider {
            self.push_undo();
            if remove_component_collider(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_collider {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "collider", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some((w, h)) = pending_tm_resize {
            self.begin_inspector_gesture();
            if tilemap_resize(&mut self.scene, &sel, w, h).is_ok() {
                self.mark_dirty();
            }
        }
        if let Some((id, solid)) = use_brush {
            self.tile_brush_id = id;
            self.tile_brush_solid = solid;
            self.edit_tool = crate::app::EditTool::Paint;
            self.status = format!("brush {id}");
        }

        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            if ui.button("Save as Prefab…").clicked() {
                self.save_entity_as_prefab(&sel);
            }
            theme::muted(ui, "Writes assets/prefabs/<name>.prefab.json");
        });
    }

    fn ui_inspector_file(&mut self, ui: &mut egui::Ui) {
        let Some(rel) = self.selected_file.clone() else {
            return;
        };
        let abs = self.game_dir.join(&rel);
        let name = rel
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let ext = rel
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let kind = file_kind_label(&rel, abs.is_dir());
        let size_label = abs
            .metadata()
            .ok()
            .map(|m| format_bytes(m.len()))
            .unwrap_or_else(|| "—".into());

        theme::card_frame().show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(&name).strong().size(14.0).color(theme::TEXT))
                    .truncate(),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Path").size(12.0).color(theme::TEXT_MUTED));
                ui.add(
                    egui::Label::new(RichText::new(rel.to_string_lossy()).color(theme::TEXT))
                        .truncate(),
                );
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Type").size(12.0).color(theme::TEXT_MUTED));
                ui.label(RichText::new(kind).color(theme::TEXT));
            });
            if abs.is_file() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Size").size(12.0).color(theme::TEXT_MUTED));
                    ui.label(RichText::new(size_label).color(theme::TEXT));
                });
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new("Actions")
                .strong()
                .size(12.0)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(4.0);
        theme::card_frame().show(ui, |ui| {
            let is_scene = rel.to_string_lossy().ends_with(".scene.json");
            let is_sprites = name.ends_with(".sprites.json");
            if ext == "png" {
                let stem = rel
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if ui
                    .add(
                        egui::Button::new(RichText::new("Edit Sprites…").strong())
                            .fill(theme::ACCENT_DIM),
                    )
                    .on_hover_text("Open the sprite sheet editor for this PNG")
                    .clicked()
                {
                    self.open_sprite_editor_stem = Some(stem.clone());
                }
                ui.add_space(6.0);
                let entities: Vec<String> =
                    self.scene.entities.iter().map(|e| e.name.clone()).collect();
                if entities.is_empty() {
                    theme::muted(ui, "No entities to assign this texture to");
                } else {
                    ui.label(
                        RichText::new("Assign to entity")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    for ent_name in entities {
                        if ui.button(&ent_name).clicked() {
                            self.assign_sprite_name(&ent_name, &stem);
                        }
                    }
                }
                ui.add_space(4.0);
                theme::muted(ui, "Slice cells · set pivot · writes .sprites.json");
            } else if is_scene {
                let is_open = abs == self.scene_path;
                if is_open {
                    theme::muted(ui, "This scene is already open");
                } else if ui.button("Open scene").clicked() {
                    self.request_open_scene(abs.clone());
                }
                ui.add_space(4.0);
                if ui.button("Set as default scene").clicked() {
                    if let Ok(rel_scene) = abs.strip_prefix(&self.game_dir) {
                        self.project.default_scene = rel_scene.to_string_lossy().into_owned();
                        match save_project(&self.game_dir, &self.project) {
                            Ok(()) => {
                                self.status =
                                    format!("default scene → {}", self.project.default_scene);
                            }
                            Err(e) => self.status = format!("save project failed: {e}"),
                        }
                    }
                }
            } else if is_sprites {
                let stem = name.trim_end_matches(".sprites.json").to_string();
                let cell_count = self
                    .catalog
                    .names()
                    .iter()
                    .filter(|n| n.as_str() == stem || n.starts_with(&format!("{stem}/")))
                    .count();
                ui.label(
                    RichText::new(format!("{cell_count} catalog entries for `{stem}`"))
                        .color(theme::TEXT),
                );
                ui.add_space(4.0);
                if ui.button("Edit Sprites…").clicked() {
                    self.open_sprite_editor_stem = Some(stem);
                }
            } else if name.ends_with(".prefab.json") {
                if ui.button("Instantiate in scene").clicked() {
                    self.instantiate_prefab_rel(&rel);
                }
                theme::muted(ui, "Creates a new entity from this prefab");
            } else if name == "game.toml" {
                theme::muted(ui, "Project settings — edit game.toml or use Set default");
            } else if abs.is_dir() {
                theme::muted(ui, "Folder — select a file for actions");
            } else {
                theme::muted(ui, "No editor actions for this file type yet");
            }
        });
    }
}
