use eframe::egui::{self, RichText};
use wiimaker_scene::{
    add_component_animation, add_component_audio_source, add_component_camera, add_component_collider,
    add_component_disc, add_component_grid_mover, add_component_sprite, add_component_text,
    add_component_tilemap, display_sorting_layer, remove_component_animation,
    remove_component_audio_source, remove_component_camera, remove_component_collider,
    remove_component_disc, remove_component_grid_mover, remove_component_sprite,
    remove_component_text, remove_component_tilemap, save_project, set_component_enabled,
    tilemap_resize, SceneColliderKind, SceneDir, SceneTextAlign,
    DEFAULT_SORTING_LAYER,
};

use crate::app::EditorApp;
use crate::theme;
use crate::ui_project::{file_kind_label, format_bytes};

impl EditorApp {
    pub(crate) fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        // Keep widgets inside the pinned panel width (sliders/labels otherwise expand PanelState).
        let slider_w = (ui.available_width() - 72.0).clamp(96.0, 180.0);
        ui.spacing_mut().slider_width = slider_w;

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
            ui.add_space(8.0);
            theme::muted(ui, "None");
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
        let mut add_animation = false;
        let mut add_camera = false;
        let mut add_grid_mover = false;
        let mut add_audio_source = false;
        let mut add_text = false;
        let mut remove_sprite = false;
        let mut remove_disc = false;
        let mut remove_tilemap = false;
        let mut remove_collider = false;
        let mut remove_animation = false;
        let mut remove_camera = false;
        let mut remove_grid_mover = false;
        let mut remove_audio_source = false;
        let mut remove_text = false;
        let mut toggle_sprite: Option<bool> = None;
        let mut toggle_disc: Option<bool> = None;
        let mut toggle_tilemap: Option<bool> = None;
        let mut toggle_collider: Option<bool> = None;
        let mut toggle_animation: Option<bool> = None;
        let mut toggle_camera: Option<bool> = None;
        let mut toggle_grid_mover: Option<bool> = None;
        let mut toggle_audio_source: Option<bool> = None;
        let mut toggle_text: Option<bool> = None;
        let mut preview_audio = false;
        let mut pending_tm_resize: Option<(u32, u32)> = None;
        let mut use_brush: Option<(u16, bool)> = None;
        let mut rename_committed = false;
        let mut prefab_apply = false;
        let mut prefab_revert = false;
        let mut prefab_unpack = false;

        let prefab_link = self
            .scene
            .find_entity(&sel)
            .and_then(|e| e.prefab.clone())
            .filter(|s| !s.is_empty());
        let prefab_ov = if let Some(src) = prefab_link.as_deref() {
            self.scene
                .find_entity(&sel)
                .and_then(|ent| {
                    wiimaker_scene::resolve_prefab_asset(&self.game_dir, src)
                        .ok()
                        .and_then(|p| wiimaker_scene::load_prefab(&p).ok())
                        .map(|pf| wiimaker_scene::prefab_overrides(ent, &pf.entity))
                })
                .unwrap_or_default()
        } else {
            wiimaker_scene::PrefabOverrides::default()
        };
        let is_prefab_instance = prefab_link.is_some();

        // GameObject header — inspector.png (name row, then Tag).
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            theme::cube_icon(ui, 18.0, theme::TEXT_MUTED);
            let name_w = (ui.available_width() - 8.0).max(80.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.rename_draft).desired_width(name_w),
            );
            if resp.lost_focus() {
                rename_committed = true;
            }
        });
        ui.horizontal(|ui| {
            theme::inspector_label_ov(ui, "Tag", prefab_ov.contains("tag"));
            if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                dirty |= ui.add(egui::DragValue::new(&mut ent.tag)).changed();
            }
        });
        if is_prefab_instance {
            ui.add_space(4.0);
            theme::card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Prefab")
                            .strong()
                            .size(12.0)
                            .color(theme::PREFAB_OVERRIDE),
                    );
                    ui.label(
                        RichText::new(wiimaker_scene::prefab_stem(
                            prefab_link.as_deref().unwrap_or(""),
                        ))
                        .size(12.0)
                        .color(theme::TEXT),
                    );
                    if !prefab_ov.is_empty() {
                        ui.label(
                            RichText::new(format!("{} overrides", prefab_ov.len()))
                                .size(11.0)
                                .strong()
                                .color(theme::PREFAB_OVERRIDE),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Apply")
                        .on_hover_text("Push overrides to the prefab asset")
                        .clicked()
                    {
                        prefab_apply = true;
                    }
                    if ui
                        .button("Revert")
                        .on_hover_text("Reset instance to the prefab asset")
                        .clicked()
                    {
                        prefab_revert = true;
                    }
                    if ui
                        .button("Unpack Completely")
                        .on_hover_text("Break the prefab link; keep current values")
                        .clicked()
                    {
                        prefab_unpack = true;
                    }
                });
            });
        }
        if rename_committed {
            self.commit_rename();
        }

        let catalog_names: Vec<String> = self.catalog.names().to_vec();
        let entity_names: Vec<String> = self.scene.entities.iter().map(|e| e.name.clone()).collect();
        let layer_names: Vec<String> = self.project.effective_sorting_layers();
        let mut pending_sprite_tex: Option<(String, [f32; 2])> = None;

        if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
            let xform_label = if ent.parent.is_some() {
                "Local Transform"
            } else {
                "Transform"
            };
            ui.add_space(4.0);
            let xform_hdr = theme::component_card_header(ui, "transform", xform_label, None, false);
            if xform_hdr.open {
            theme::inspector_props().show(ui, |ui| {
                dirty |= theme::vec3_row_ov(
                    ui,
                    "Position",
                    &mut ent.transform.translation,
                    1.0,
                    prefab_ov.contains("transform.position"),
                );
                let rot = ent.transform.rotation;
                let mut euler = [
                    0.0_f32,
                    0.0_f32,
                    (2.0 * rot[2] * rot[3]).atan2(rot[3] * rot[3] - rot[2] * rot[2]).to_degrees(),
                ];
                if theme::vec3_row_ov(
                    ui,
                    "Rotation",
                    &mut euler,
                    0.5,
                    prefab_ov.contains("transform.rotation"),
                ) {
                    let half = euler[2].to_radians() * 0.5;
                    ent.transform.rotation = [0.0, 0.0, half.sin(), half.cos()];
                    dirty = true;
                }
                dirty |= theme::vec3_row_ov(
                    ui,
                    "Scale",
                    &mut ent.transform.scale,
                    0.01,
                    prefab_ov.contains("transform.scale"),
                );
            });
            }

            ui.add_space(6.0);
            if let Some(sp) = ent.components.sprite.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "sprite",
                    "Sprite",
                    Some(sp.enabled),
                    true,
                    prefab_ov.component("Sprite"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_sprite = Some(en);
                }
                if hdr.remove {
                    remove_sprite = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
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
                    dirty |= theme::vec2_row_ov(
                        ui,
                        "Size",
                        &mut sp.size,
                        0.5,
                        prefab_ov.contains("Sprite.size"),
                    );
                    dirty |= sorting_layer_fields(
                        ui,
                        "sprite_sort",
                        &layer_names,
                        &mut sp.sorting_layer,
                        &mut sp.z,
                        prefab_ov.contains("Sprite.sorting_layer"),
                        prefab_ov.contains("Sprite.z"),
                    );
                });
                }
                ui.add_space(4.0);
            }

            if let Some(a) = ent.components.animation.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "animation",
                    "Animation",
                    Some(a.enabled),
                    true,
                    prefab_ov.component("Animation"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_animation = Some(en);
                }
                if hdr.remove {
                    remove_animation = true;
                }
                // Always expand Animation when agent-selected (screenshot OCR).
                let force_open = std::env::var("WIIMAKER_EDITOR_SELECT").is_ok();
                if hdr.open || force_open {
                theme::inspector_props().show(ui, |ui| {
                    let clip_names: Vec<String> = self.anim_catalog.names().to_vec();
                    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                    let mut clip_changed = false;
                    let mut new_clip = a.clip.clone();
                    egui::ComboBox::from_id_salt("anim_clip")
                        .selected_text(if a.clip.is_empty() { "(none)" } else { &a.clip })
                        .width(combo_w)
                        .show_ui(ui, |ui| {
                            for name in &clip_names {
                                if ui.selectable_label(a.clip == *name, name).clicked() {
                                    new_clip = name.clone();
                                    clip_changed = true;
                                }
                            }
                        });
                    if clip_changed {
                        a.clip = new_clip;
                        dirty = true;
                    }
                    let mut use_override = a.fps.is_some();
                    if ui.checkbox(&mut use_override, "Override FPS").changed() {
                        if use_override {
                            let default_fps = self
                                .anim_catalog
                                .lookup(&a.clip)
                                .map(|m| m.fps)
                                .unwrap_or(10.0);
                            a.fps = Some(default_fps);
                        } else {
                            a.fps = None;
                        }
                        dirty = true;
                    }
                    if let Some(fps) = a.fps.as_mut() {
                        dirty |= theme::labeled_drag_ov(
                            ui,
                            "Fps",
                            fps,
                            0.25,
                            prefab_ov.contains("Animation.fps"),
                        );
                    } else if let Some(meta) = self.anim_catalog.lookup(&a.clip) {
                        ui.label(
                            RichText::new(format!("clip fps · {:.1}", meta.fps))
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                    dirty |= ui.checkbox(&mut a.loop_, "Loop").changed();
                    if let Some(meta) = self.anim_catalog.lookup(&a.clip) {
                        ui.label(
                            RichText::new(format!("{} cells · {}", meta.cells.len(), meta.cells.join(", ")))
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                    }
                });
                }
                ui.add_space(4.0);
            }

            if let Some(d) = ent.components.disc.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "disc",
                    "Disc",
                    Some(d.enabled),
                    true,
                    prefab_ov.component("Disc"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_disc = Some(en);
                }
                if hdr.remove {
                    remove_disc = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Radius",
                        &mut d.radius,
                        0.5,
                        prefab_ov.contains("Disc.radius"),
                    );
                    dirty |= sorting_layer_fields(
                        ui,
                        "disc_sort",
                        &layer_names,
                        &mut d.sorting_layer,
                        &mut d.z,
                        prefab_ov.contains("Disc.sorting_layer"),
                        prefab_ov.contains("Disc.z"),
                    );
                });
                }
            }

            if let Some(tm) = ent.components.tilemap.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "tilemap",
                    "Tilemap",
                    Some(tm.enabled),
                    true,
                    prefab_ov.component("Tilemap"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_tilemap = Some(en);
                }
                if hdr.remove {
                    remove_tilemap = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
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
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Cell",
                        &mut tm.cell,
                        0.25,
                        prefab_ov.contains("Tilemap.cell"),
                    );
                    dirty |= theme::vec2_row_ov(
                        ui,
                        "Origin",
                        &mut tm.origin,
                        1.0,
                        prefab_ov.contains("Tilemap.origin"),
                    );
                    dirty |= sorting_layer_fields(
                        ui,
                        "tilemap_sort",
                        &layer_names,
                        &mut tm.sorting_layer,
                        &mut tm.z,
                        prefab_ov.contains("Tilemap.sorting_layer"),
                        prefab_ov.contains("Tilemap.z"),
                    );
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
                }
            }

            if let Some(c) = ent.components.collider.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "collider",
                    "Collider",
                    Some(c.enabled),
                    true,
                    prefab_ov.component("Collider"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_collider = Some(en);
                }
                if hdr.remove {
                    remove_collider = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
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
                            dirty |= theme::vec2_row_ov(
                                ui,
                                "Size",
                                &mut c.size,
                                0.5,
                                prefab_ov.contains("Collider.size"),
                            );
                        }
                        SceneColliderKind::Circle => {
                            dirty |= theme::labeled_drag_ov(
                                ui,
                                "Radius",
                                &mut c.radius,
                                0.5,
                                prefab_ov.contains("Collider.radius"),
                            );
                        }
                    }
                    dirty |= ui.checkbox(&mut c.solid, "solid").changed();
                    // trigger is independent of solid (Unity): trigger=true never blocks via overlap_solid.
                    dirty |= ui.checkbox(&mut c.trigger, "Is Trigger").changed();
                    ui.horizontal(|ui| {
                        ui.label("Filter Tag");
                        let r = ui.add(egui::DragValue::new(&mut c.filter_tag));
                        r.clone().on_hover_text("0 = any");
                        dirty |= r.changed();
                    });
                    dirty |= theme::vec2_row_ov(
                        ui,
                        "Offset",
                        &mut c.offset,
                        0.5,
                        prefab_ov.contains("Collider.offset"),
                    );
                });
                }
            }

            if let Some(cam) = ent.components.camera.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "camera",
                    "Camera",
                    Some(cam.active),
                    true,
                    prefab_ov.component("Camera"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_camera = Some(en);
                }
                if hdr.remove {
                    remove_camera = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
                    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                    let current = cam.follow.clone().unwrap_or_default();
                    let label = if current.is_empty() {
                        "(none)"
                    } else {
                        current.as_str()
                    };
                    let mut new_follow = cam.follow.clone();
                    let mut follow_changed = false;
                    egui::ComboBox::from_id_salt("camera_follow")
                        .selected_text(label)
                        .width(combo_w)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(current.is_empty(), "(none)")
                                .clicked()
                            {
                                new_follow = None;
                                follow_changed = true;
                            }
                            for name in &entity_names {
                                if name == &sel {
                                    continue;
                                }
                                if ui
                                    .selectable_label(current.as_str() == name, name)
                                    .clicked()
                                {
                                    new_follow = Some(name.clone());
                                    follow_changed = true;
                                }
                            }
                        });
                    if follow_changed {
                        cam.follow = new_follow;
                        dirty = true;
                    }
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Lerp",
                        &mut cam.lerp,
                        0.01,
                        prefab_ov.contains("Camera.lerp"),
                    );
                    if cam.lerp < 0.0 {
                        cam.lerp = 0.0;
                    }
                    if cam.lerp > 1.0 {
                        cam.lerp = 1.0;
                    }
                });
                }
                ui.add_space(4.0);
            }

            if let Some(g) = ent.components.grid_mover.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "gridmover",
                    "GridMover",
                    Some(g.enabled),
                    true,
                    prefab_ov.component("GridMover"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_grid_mover = Some(en);
                }
                if hdr.remove {
                    remove_grid_mover = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Cell",
                        &mut g.cell,
                        0.25,
                        prefab_ov.contains("GridMover.cell"),
                    );
                    if g.cell < 0.01 {
                        g.cell = 0.01;
                    }
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Speed",
                        &mut g.speed,
                        0.5,
                        prefab_ov.contains("GridMover.speed"),
                    );
                    if g.speed < 0.0 {
                        g.speed = 0.0;
                    }
                    let current = g.queued_dir;
                    let label = match current {
                        None => "(none)",
                        Some(SceneDir::Up) => "Up",
                        Some(SceneDir::Down) => "Down",
                        Some(SceneDir::Left) => "Left",
                        Some(SceneDir::Right) => "Right",
                    };
                    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                    let mut new_q = current;
                    let mut q_changed = false;
                    egui::ComboBox::from_id_salt("grid_queued_dir")
                        .selected_text(label)
                        .width(combo_w)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current.is_none(), "(none)").clicked() {
                                new_q = None;
                                q_changed = true;
                            }
                            for (d, name) in [
                                (SceneDir::Up, "Up"),
                                (SceneDir::Down, "Down"),
                                (SceneDir::Left, "Left"),
                                (SceneDir::Right, "Right"),
                            ] {
                                if ui.selectable_label(current == Some(d), name).clicked() {
                                    new_q = Some(d);
                                    q_changed = true;
                                }
                            }
                        });
                    if q_changed {
                        g.queued_dir = new_q;
                        dirty = true;
                    }
                    ui.label(
                        RichText::new("4-way only · diagonals use horizontal · reverse is immediate")
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                });
                }
                ui.add_space(4.0);
            }

            if let Some(a) = ent.components.audio_source.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "audiosource",
                    "AudioSource",
                    Some(a.enabled),
                    true,
                    prefab_ov.component("AudioSource"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_audio_source = Some(en);
                }
                if hdr.remove {
                    remove_audio_source = true;
                }
                if hdr.open {
                theme::inspector_props().show(ui, |ui| {
                    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                    let mut clip_changed = false;
                    let mut new_clip = a.clip.clone();
                    egui::ComboBox::from_id_salt("audio_clip")
                        .selected_text(if a.clip.is_empty() { "(none)" } else { &a.clip })
                        .width(combo_w)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(a.clip.is_empty(), "(none)")
                                .clicked()
                            {
                                new_clip.clear();
                                clip_changed = true;
                            }
                            for name in &self.wav_names {
                                if ui.selectable_label(a.clip == *name, name).clicked() {
                                    new_clip = name.clone();
                                    clip_changed = true;
                                }
                            }
                        });
                    if clip_changed {
                        a.clip = new_clip;
                        dirty = true;
                    }
                    dirty |= theme::labeled_drag_ov(
                        ui,
                        "Volume",
                        &mut a.volume,
                        0.01,
                        prefab_ov.contains("AudioSource.volume"),
                    );
                    if a.volume < 0.0 {
                        a.volume = 0.0;
                    }
                    if a.volume > 1.0 {
                        a.volume = 1.0;
                    }
                    dirty |= ui.checkbox(&mut a.play_on_awake, "Play On Awake").changed();
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Play").color(theme::TEXT))
                                .fill(theme::ACCENT_DIM),
                        )
                        .on_hover_text("Host oneshot preview")
                        .clicked()
                    {
                        preview_audio = true;
                    }
                    ui.label(
                        RichText::new("Host PCM16 WAV · Wii ASND not wired")
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                });
                }
                ui.add_space(4.0);
            }

            if let Some(t) = ent.components.text.as_mut() {
                let hdr = theme::component_card_header_ov(
                    ui,
                    "text",
                    "Text",
                    Some(t.enabled),
                    true,
                    prefab_ov.component("Text"),
                );
                if let Some(en) = hdr.toggle {
                    toggle_text = Some(en);
                }
                if hdr.remove {
                    remove_text = true;
                }
                if hdr.open {
                    theme::inspector_props().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            theme::inspector_label_ov(
                                ui,
                                "Text",
                                prefab_ov.contains("Text.text"),
                            );
                            let edit_w = (ui.available_width() - 8.0).clamp(80.0, 220.0);
                            if ui
                                .add(
                                    egui::TextEdit::multiline(&mut t.text)
                                        .desired_rows(2)
                                        .desired_width(edit_w),
                                )
                                .changed()
                            {
                                dirty = true;
                            }
                        });
                        dirty |= theme::labeled_drag_ov(
                            ui,
                            "Size",
                            &mut t.size,
                            0.5,
                            prefab_ov.contains("Text.size"),
                        );
                        if t.size < 1.0 {
                            t.size = 1.0;
                        }
                        ui.horizontal(|ui| {
                            theme::inspector_label_ov(
                                ui,
                                "Color",
                                prefab_ov.contains("Text.color"),
                            );
                            let mut col = egui::Color32::from_rgba_unmultiplied(
                                t.color[0],
                                t.color[1],
                                t.color[2],
                                t.color[3],
                            );
                            if ui.color_edit_button_srgba(&mut col).changed() {
                                t.color = [col.r(), col.g(), col.b(), col.a()];
                                dirty = true;
                            }
                        });
                        let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
                        let current = t.align;
                        let mut picked = current;
                        ui.horizontal(|ui| {
                            theme::inspector_label_ov(
                                ui,
                                "Align",
                                prefab_ov.contains("Text.align"),
                            );
                            egui::ComboBox::from_id_salt("text_align")
                                .selected_text(current.to_runtime().as_str())
                                .width(combo_w)
                                .show_ui(ui, |ui| {
                                    for a in [
                                        SceneTextAlign::Left,
                                        SceneTextAlign::Center,
                                        SceneTextAlign::Right,
                                    ] {
                                        if ui
                                            .selectable_label(
                                                current == a,
                                                a.to_runtime().as_str(),
                                            )
                                            .clicked()
                                        {
                                            picked = a;
                                        }
                                    }
                                });
                        });
                        if picked != current {
                            t.align = picked;
                            dirty = true;
                        }
                        dirty |= sorting_layer_fields(
                            ui,
                            "text_sort",
                            &layer_names,
                            &mut t.sorting_layer,
                            &mut t.z,
                            prefab_ov.contains("Text.sorting_layer"),
                            prefab_ov.contains("Text.z"),
                        );
                        ui.label(
                            RichText::new("Host bitmap HUD · Wii GX skip")
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                    });
                }
                ui.add_space(4.0);
            }

            let missing_sprite = ent.components.sprite.is_none();
            let missing_disc = ent.components.disc.is_none();
            let missing_tilemap = ent.components.tilemap.is_none();
            let missing_collider = ent.components.collider.is_none();
            let missing_animation = ent.components.animation.is_none();
            let missing_camera = ent.components.camera.is_none();
            let missing_grid_mover = ent.components.grid_mover.is_none();
            let missing_audio_source = ent.components.audio_source.is_none();
            let missing_text = ent.components.text.is_none();
            ui.add_space(12.0);
            let add_w = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(add_w, 28.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
            ui.set_min_width(add_w);
            ui.menu_button(
                RichText::new("Add Component").strong().color(theme::TEXT),
                |ui| {
                    if missing_sprite && ui.button("Sprite").clicked() {
                        add_sprite = true;
                        ui.close_menu();
                    }
                    if missing_disc && ui.button("Disc").clicked() {
                        add_disc = true;
                        ui.close_menu();
                    }
                    if missing_tilemap && ui.button("Tilemap").clicked() {
                        add_tilemap = true;
                        ui.close_menu();
                    }
                    if missing_collider && ui.button("Collider").clicked() {
                        add_collider = true;
                        ui.close_menu();
                    }
                    if missing_animation && ui.button("Animation").clicked() {
                        add_animation = true;
                        ui.close_menu();
                    }
                    if missing_camera && ui.button("Camera").clicked() {
                        add_camera = true;
                        ui.close_menu();
                    }
                    if missing_grid_mover && ui.button("GridMover").clicked() {
                        add_grid_mover = true;
                        ui.close_menu();
                    }
                    if missing_audio_source && ui.button("AudioSource").clicked() {
                        add_audio_source = true;
                        ui.close_menu();
                    }
                    if missing_text && ui.button("Text").clicked() {
                        add_text = true;
                        ui.close_menu();
                    }
                    if !missing_sprite
                        && !missing_disc
                        && !missing_tilemap
                        && !missing_collider
                        && !missing_animation
                        && !missing_camera
                        && !missing_grid_mover
                        && !missing_audio_source
                        && !missing_text
                    {
                        ui.label(
                            RichText::new("All components present")
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                },
            );
                },
            );
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
            let _ = add_component_collider(&mut self.scene, &sel, kind, size, radius, true, false, 0);
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
        if add_animation {
            let clip = self
                .anim_catalog
                .names()
                .first()
                .cloned()
                .unwrap_or_else(|| "chomp".into());
            self.push_undo();
            let _ = add_component_animation(&mut self.scene, &sel, &clip, None, true);
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_animation {
            self.push_undo();
            if remove_component_animation(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_animation {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "animation", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_camera {
            self.push_undo();
            let _ = add_component_camera(&mut self.scene, &sel, true);
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_camera {
            self.push_undo();
            if remove_component_camera(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_camera {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "camera", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_grid_mover {
            self.push_undo();
            let _ = add_component_grid_mover(&mut self.scene, &sel, 16.0, 120.0);
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_grid_mover {
            self.push_undo();
            if remove_component_grid_mover(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_grid_mover {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "gridmover", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_audio_source {
            let clip = self.wav_names.first().cloned().unwrap_or_default();
            self.push_undo();
            let _ = add_component_audio_source(&mut self.scene, &sel, &clip, 1.0, false);
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_audio_source {
            self.push_undo();
            if remove_component_audio_source(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_audio_source {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "audiosource", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if add_text {
            self.push_undo();
            let _ = add_component_text(
                &mut self.scene,
                &sel,
                "Text",
                16.0,
                [255, 255, 255, 255],
                SceneTextAlign::Left,
            );
            self.sync_baseline();
            self.mark_dirty();
        }
        if remove_text {
            self.push_undo();
            if remove_component_text(&mut self.scene, &sel).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if let Some(en) = toggle_text {
            self.push_undo();
            if set_component_enabled(&mut self.scene, &sel, "text", en).is_ok() {
                self.sync_baseline();
                self.mark_dirty();
            } else {
                let _ = self.undo.undo(&mut self.scene);
            }
        }
        if preview_audio {
            if let Some(a) = self
                .scene
                .find_entity(&sel)
                .and_then(|e| e.components.audio_source.as_ref())
            {
                let clip = a.clip.clone();
                let vol = a.volume;
                self.preview_wav_clip(&clip, vol);
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
        if is_prefab_instance {
            if prefab_apply {
                self.apply_selected_prefab(&sel);
            }
            if prefab_revert {
                self.revert_selected_prefab(&sel);
            }
            if prefab_unpack {
                self.unpack_selected_prefab(&sel);
            }
        } else {
            theme::card_frame().show(ui, |ui| {
                if ui.button("Save as Prefab…").clicked() {
                    self.save_entity_as_prefab(&sel);
                }
                theme::muted(ui, "Writes assets/prefabs/<name>.prefab.json");
            });
        }
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
                theme::muted(ui, "Project settings — Scenes in Build · Sorting Layers");
                ui.add_space(6.0);
                self.ui_build_settings_body(ui);
                ui.add_space(8.0);
                self.ui_sorting_layers_body(ui);
                ui.add_space(6.0);
                if ui.button("Open Build Settings…").clicked() {
                    self.show_build_settings = true;
                }
            } else if ext == "wav" {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Play").strong()).fill(theme::ACCENT_DIM),
                    )
                    .on_hover_text("Preview this WAV on the host")
                    .clicked()
                {
                    if let Some(stem) = rel.file_stem().and_then(|s| s.to_str()) {
                        self.preview_wav_clip(stem, 1.0);
                    }
                }
                theme::muted(ui, "PCM16 mono/stereo oneshot · not packed into .wpack yet");
            } else if abs.is_dir() {
                theme::muted(ui, "Folder — select a file for actions");
            } else {
                theme::muted(ui, "No editor actions for this file type yet");
            }
        });
    }

    fn ui_sorting_layers_body(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Sorting Layers")
                .size(13.0)
                .strong()
                .color(theme::TEXT),
        );
        theme::muted(ui, "Ordered list on game.toml. Back = drawn first.");
        ui.add_space(4.0);
        let layers = self.project.effective_sorting_layers();
        let mut move_to: Option<(String, usize)> = None;
        let mut remove: Option<String> = None;
        theme::card_frame().show(ui, |ui| {
            for (i, name) in layers.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{i}"))
                            .size(11.0)
                            .color(theme::TEXT_DIM)
                            .monospace(),
                    );
                    let selected = self.sorting_layer_pick == *name;
                    let resp = ui.selectable_label(selected, name);
                    if resp.clicked() {
                        self.sorting_layer_pick = name.clone();
                    }
                    if ui
                        .add_enabled(i > 0, egui::Button::new("↑").small())
                        .on_hover_text("Move earlier (draw behind)")
                        .clicked()
                    {
                        move_to = Some((name.clone(), i - 1));
                    }
                    if ui
                        .add_enabled(i + 1 < layers.len(), egui::Button::new("↓").small())
                        .on_hover_text("Move later (draw in front)")
                        .clicked()
                    {
                        move_to = Some((name.clone(), i + 1));
                    }
                    let can_remove = !name.eq_ignore_ascii_case(DEFAULT_SORTING_LAYER)
                        && layers.len() > 1;
                    if ui
                        .add_enabled(can_remove, egui::Button::new("–").small())
                        .on_hover_text("Remove (assignments → Default)")
                        .clicked()
                    {
                        remove = Some(name.clone());
                    }
                });
            }
        });
        let mut pending_add: Option<String> = None;
        let mut pending_rename: Option<(String, String)> = None;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.sorting_layer_draft)
                    .hint_text("New layer")
                    .desired_width(120.0),
            );
            let draft = self.sorting_layer_draft.trim().to_string();
            if ui
                .add_enabled(!draft.is_empty(), egui::Button::new("+ Add"))
                .clicked()
            {
                pending_add = Some(draft.clone());
            }
            let pick = self.sorting_layer_pick.clone();
            if ui
                .add_enabled(
                    !draft.is_empty() && !pick.is_empty() && pick != draft,
                    egui::Button::new("Rename"),
                )
                .on_hover_text("Rename the selected layer to the draft name")
                .clicked()
            {
                pending_rename = Some((pick, draft));
            }
        });
        if let Some((name, idx)) = move_to {
            self.move_project_sorting_layer(&name, idx);
        }
        if let Some(name) = remove {
            self.remove_project_sorting_layer(&name);
        }
        if let Some(name) = pending_add {
            self.add_project_sorting_layer(&name);
        }
        if let Some((from, to)) = pending_rename {
            self.rename_project_sorting_layer(&from, &to);
            self.sorting_layer_pick = to;
        }
    }
}

fn sorting_layer_fields(
    ui: &mut egui::Ui,
    id_salt: &str,
    layers: &[String],
    layer: &mut String,
    z: &mut f32,
    ov_layer: bool,
    ov_order: bool,
) -> bool {
    let mut dirty = false;
    let current = display_sorting_layer(layer).to_string();
    let combo_w = (ui.available_width() - 8.0).clamp(100.0, 220.0);
    ui.horizontal(|ui| {
        theme::inspector_label_ov(ui, "Sorting Layer", ov_layer);
        let mut picked = current.clone();
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(&current)
            .width(combo_w)
            .show_ui(ui, |ui| {
                for name in layers {
                    if ui.selectable_label(current == *name, name).clicked() {
                        picked = name.clone();
                    }
                }
            });
        if picked != current {
            *layer = if picked.eq_ignore_ascii_case(DEFAULT_SORTING_LAYER) {
                String::new()
            } else {
                picked
            };
            dirty = true;
        }
    });
    dirty |= theme::labeled_drag_ov(ui, "Order in Layer", z, 0.05, ov_order);
    dirty
}
