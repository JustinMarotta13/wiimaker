use eframe::egui::{self, RichText};
use wiimaker_scene::{add_entity, remove_entity, set_entity_parent, unique_entity_name, MutateOpts};

use crate::app::EditorApp;
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_hierarchy(&mut self, ui: &mut egui::Ui) {
        let _ = theme::dock_tabs(ui, &[("Hierarchy", ())], ());

        // Unity 6 Hierarchy toolbar: + then search (see MEMORY/durable/unity-chrome/hierarchy.png).
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let plus = ui
                .add_sized(
                    [22.0, 18.0],
                    egui::Button::new(RichText::new("+").size(14.0)).fill(theme::BG_RAISED),
                )
                .on_hover_text("Create Empty");
            if plus.clicked() {
                self.hierarchy_create_empty(None);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.hierarchy_filter)
                    .desired_width(ui.available_width())
                    .hint_text("Search"),
            );
        });
        ui.add_space(4.0);

        let mut to_remove = None;
        let mut to_duplicate = None;
        let mut reparent: Option<(String, Option<String>)> = None;
        let mut create_under: Option<Option<String>> = None;

        let filter = self.hierarchy_filter.clone();
        let roots = self.scene.root_names();

        let drop_frame = egui::Frame::none();
        let (scroll_out, dropped_empty) = ui.dnd_drop_zone(drop_frame, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("hierarchy_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if roots.is_empty() && filter.is_empty() {
                        theme::muted(ui, "Scene is empty");
                    } else {
                        for name in &roots {
                            if self.hierarchy_visible(name, &filter) {
                                self.hierarchy_row(
                                    ui,
                                    name,
                                    0,
                                    &filter,
                                    &mut to_remove,
                                    &mut to_duplicate,
                                    &mut reparent,
                                    &mut create_under,
                                );
                            }
                        }
                    }
                    let remain = ui.available_height().max(24.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(ui.available_width(), remain), egui::Sense::click());
                    let _ = rect;
                    resp.context_menu(|ui| {
                        if ui.button("Create Empty").clicked() {
                            create_under = Some(None);
                            ui.close_menu();
                        }
                    });
                });
        });
        let _ = scroll_out;
        if let Some(child) = dropped_empty {
            let child: std::sync::Arc<String> = child;
            reparent = Some(((*child).clone(), None));
        }

        if let Some(parent) = create_under {
            self.hierarchy_create_empty(parent);
        }
        if let Some((child, parent)) = reparent {
            if self.scene.find_entity(&child).map(|e| e.parent.clone()) != Some(parent.clone()) {
                self.push_undo();
                if set_entity_parent(&mut self.scene, &child, parent.as_deref()).is_ok() {
                    self.select(Some(child));
                    self.sync_baseline();
                    self.mark_dirty();
                } else {
                    let _ = self.undo.undo(&mut self.scene);
                }
            }
        }
        if let Some(name) = to_duplicate {
            self.select(Some(name));
            self.do_duplicate();
        }
        if let Some(name) = to_remove {
            let mut kill = if self.is_selected(&name) {
                self.selected.clone()
            } else {
                vec![name]
            };
            kill.sort();
            kill.dedup();
            self.push_undo();
            for n in &kill {
                let _ = remove_entity(&mut self.scene, n);
            }
            self.prune_selection();
            self.sync_baseline();
            self.mark_dirty();
        }
    }

    fn hierarchy_visible(&self, name: &str, filter: &str) -> bool {
        if filter.trim().is_empty() {
            return true;
        }
        let f = filter.to_lowercase();
        if name.to_lowercase().contains(&f) {
            return true;
        }
        self.scene
            .child_names(name)
            .iter()
            .any(|c| self.hierarchy_visible(c, filter))
    }

    fn hierarchy_create_empty(&mut self, parent: Option<String>) {
        let name = unique_entity_name(&self.scene, "GameObject");
        self.push_undo();
        if add_entity(
            &mut self.scene,
            &name,
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                ..Default::default()
            },
        )
        .is_ok()
        {
            if let Some(p) = parent.as_deref() {
                if set_entity_parent(&mut self.scene, &name, Some(p)).is_err() {
                    let _ = self.undo.undo(&mut self.scene);
                    return;
                }
            }
            self.select(Some(name));
            self.sync_baseline();
            self.mark_dirty();
        } else {
            let _ = self.undo.undo(&mut self.scene);
        }
    }

    fn hierarchy_row(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        depth: u32,
        filter: &str,
        to_remove: &mut Option<String>,
        to_duplicate: &mut Option<String>,
        reparent: &mut Option<(String, Option<String>)>,
        create_under: &mut Option<Option<String>>,
    ) {
        let selected = self.is_selected(name);
        let children = self.scene.child_names(name);
        let has_kids = !children.is_empty();
        let indent = 6.0 + depth as f32 * 14.0;

        let fold_id = ui.make_persistent_id(("hier_fold", name.to_string()));
        let mut folded_open = ui.ctx().data_mut(|d| *d.get_temp_mut_or(fold_id, true));

        let fill = if selected {
            theme::SELECT_BG
        } else {
            egui::Color32::TRANSPARENT
        };
        let row_frame = egui::Frame::none()
            .fill(fill)
            .rounding(egui::Rounding::ZERO)
            .inner_margin(egui::Margin {
                left: indent,
                right: 4.0,
                top: 2.0,
                bottom: 2.0,
            });

        let (inner, dropped) = ui.dnd_drop_zone(row_frame, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if has_kids {
                    let arrow = if folded_open { "▾" } else { "▸" };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(arrow).size(11.0).color(theme::TEXT_MUTED))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .min_size(egui::vec2(14.0, 16.0)),
                        )
                        .clicked()
                    {
                        folded_open = !folded_open;
                    }
                } else {
                    ui.add_space(14.0);
                }

                // Tiny geometric cube — not Unity's logo.
                let (icon, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_stroke(
                    icon,
                    1.0,
                    egui::Stroke::new(1.0_f32, theme::TEXT_DIM),
                );

                let drag_id = egui::Id::new(("hier-drag", name));
                let name_color = if selected {
                    egui::Color32::from_rgb(235, 235, 235)
                } else {
                    theme::TEXT
                };
                let label = RichText::new(name).size(13.0).color(name_color);
                if ui.ctx().is_being_dragged(drag_id) {
                    egui::DragAndDrop::set_payload(ui.ctx(), name.to_string());
                    let layer_id = egui::LayerId::new(egui::Order::Tooltip, drag_id);
                    let response = ui
                        .with_layer_id(layer_id, |ui| {
                            ui.add(egui::Label::new(label).truncate().selectable(false));
                        })
                        .response;
                    if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                        let delta = pointer_pos - response.rect.center();
                        ui.ctx().transform_layer_shapes(
                            layer_id,
                            egui::emath::TSTransform::from_translation(delta),
                        );
                    }
                } else {
                    let body = ui.scope(|ui| {
                        let grow = ui.available_width().max(8.0);
                        ui.add(egui::Label::new(label).truncate().selectable(false));
                        ui.allocate_exact_size(egui::vec2(grow, 16.0), egui::Sense::hover());
                    });
                    let response = ui
                        .interact(body.response.rect, drag_id, egui::Sense::click_and_drag())
                        .on_hover_cursor(egui::CursorIcon::Grab);
                    if response.hovered() && !selected {
                        ui.painter().rect_filled(
                            body.response.rect,
                            0.0,
                            egui::Color32::from_white_alpha(8),
                        );
                    }
                    if response.clicked() {
                        let cmd = ui.input(|i| i.modifiers.command);
                        if cmd {
                            self.select_toggle(name.to_string());
                        } else {
                            self.select(Some(name.to_string()));
                        }
                    }
                    response.context_menu(|ui| {
                        if ui.button("Create Empty").clicked() {
                            *create_under = Some(Some(name.to_string()));
                            ui.close_menu();
                        }
                        if ui.button("Duplicate").clicked() {
                            *to_duplicate = Some(name.to_string());
                            ui.close_menu();
                        }
                        if self.scene.find_entity(name).and_then(|e| e.parent.clone()).is_some()
                            && ui.button("Unparent").clicked()
                        {
                            *reparent = Some((name.to_string(), None));
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .add(egui::Button::new(RichText::new("Delete").color(theme::DANGER)))
                            .clicked()
                        {
                            *to_remove = Some(name.to_string());
                            ui.close_menu();
                        }
                    });
                    response.dnd_set_drag_payload(name.to_string());
                }
            });
        });
        let _ = inner;
        ui.ctx().data_mut(|d| d.insert_temp(fold_id, folded_open));
        if let Some(child) = dropped {
            let child: std::sync::Arc<String> = child;
            if child.as_str() != name {
                *reparent = Some(((*child).clone(), Some(name.to_string())));
            }
        }

        if folded_open {
            for child in children {
                if self.hierarchy_visible(&child, filter) {
                    self.hierarchy_row(
                        ui,
                        &child,
                        depth + 1,
                        filter,
                        to_remove,
                        to_duplicate,
                        reparent,
                        create_under,
                    );
                }
            }
        }
    }
}
