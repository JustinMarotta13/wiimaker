use eframe::egui::{self, RichText};
use wiimaker_scene::{add_entity, remove_entity, set_entity_parent, unique_entity_name, MutateOpts};

use crate::app::EditorApp;
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_hierarchy(&mut self, ui: &mut egui::Ui) {
        // Dock tab title is the name — search + tree only (no inner Hierarchy strip).
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
            theme::search_icon(ui, 14.0);
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
            // Virtualize: a 309-entity scene (Dots + 301 pellets) must not instantiate
            // every row or later docks (Inspector, Scene) never tessellate.
            let rows = self.hierarchy_flat_rows(ui.ctx(), &filter);
            let row_h = 22.0;
            egui::ScrollArea::vertical()
                .id_salt("hierarchy_scroll")
                .auto_shrink([false, false])
                .show_rows(ui, row_h, rows.len().max(1), |ui, range| {
                    if roots.is_empty() && filter.is_empty() {
                        theme::muted(ui, "Scene is empty");
                        return;
                    }
                    for i in range {
                        if let Some((name, depth)) = rows.get(i) {
                            self.hierarchy_row(
                                ui,
                                name,
                                *depth,
                                &filter,
                                &mut to_remove,
                                &mut to_duplicate,
                                &mut reparent,
                                &mut create_under,
                            );
                        }
                    }
                    let remain = ui.available_height().clamp(4.0, 24.0);
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width().min(220.0), remain),
                        egui::Sense::click(),
                    );
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

        let fold_id = Self::hierarchy_fold_id(name);
        let mut folded_open = ui.ctx().data_mut(|d| *d.get_temp_mut_or(fold_id, false));

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
                    if theme::foldout_button(ui, folded_open).clicked() {
                        folded_open = !folded_open;
                    }
                } else {
                    ui.add_space(14.0);
                }

                // Tiny two-face isometric cube — not Unity's logo.
                let cube_color = if selected {
                    egui::Color32::from_rgb(230, 230, 230)
                } else {
                    theme::TEXT_MUTED
                };
                theme::cube_icon(ui, 11.0, cube_color);

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
                        let grow = ui.available_width().clamp(8.0, 220.0);
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

        let _ = folded_open;
        let _ = filter;
    }

    fn hierarchy_fold_id(name: &str) -> egui::Id {
        egui::Id::new(("hier_fold", name))
    }

    /// Flattened (name, depth) rows respecting fold state. Selected nodes with
    /// children and ancestors of the selection stay open so Dot0.. is reachable.
    fn hierarchy_flat_rows(&self, ctx: &egui::Context, filter: &str) -> Vec<(String, u32)> {
        let mut out = Vec::new();
        for name in self.scene.root_names() {
            self.hierarchy_collect_flat(ctx, &name, 0, filter, &mut out);
        }
        out
    }

    fn hierarchy_collect_flat(
        &self,
        ctx: &egui::Context,
        name: &str,
        depth: u32,
        filter: &str,
        out: &mut Vec<(String, u32)>,
    ) {
        if !self.hierarchy_visible(name, filter) {
            return;
        }
        out.push((name.to_string(), depth));
        let children = self.scene.child_names(name);
        if children.is_empty() {
            return;
        }
        let fold_id = Self::hierarchy_fold_id(name);
        let stored = ctx.data(|d| d.get_temp::<bool>(fold_id).unwrap_or(false));
        let selected_here = self.is_selected(name);
        let child_selected = self
            .selected
            .iter()
            .any(|s| s != name && self.scene.is_descendant_of(s, name));
        let open = stored
            || child_selected
            || (selected_here && !children.is_empty())
            || !filter.trim().is_empty();
        if open {
            for child in children {
                self.hierarchy_collect_flat(ctx, &child, depth + 1, filter, out);
            }
        }
    }
}
