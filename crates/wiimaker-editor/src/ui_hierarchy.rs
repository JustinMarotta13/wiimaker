use eframe::egui::{self, RichText};
use wiimaker_scene::{add_entity, remove_entity, set_entity_parent, unique_entity_name, MutateOpts};

use crate::app::EditorApp;
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_hierarchy(&mut self, ui: &mut egui::Ui) {
        let _ = theme::dock_tabs(ui, &[("Hierarchy", ())], ());
        if self.selected.len() > 1 {
            theme::meta_chip(
                ui,
                "multi",
                &format!("{} selected", self.selected.len()),
            );
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_entity_name)
                    .desired_width(140.0)
                    .hint_text("Entity name"),
            );
            if ui
                .add(egui::Button::new(RichText::new("+ Add").strong()))
                .clicked()
            {
                let name = unique_entity_name(&self.scene, &self.new_entity_name);
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
                    self.select(Some(name.clone()));
                    self.new_entity_name = unique_entity_name(&self.scene, "NewEntity");
                    self.sync_baseline();
                    self.mark_dirty();
                } else {
                    let _ = self.undo.undo(&mut self.scene);
                }
            }
        });
        ui.add_space(4.0);
        theme::muted(ui, "Cmd-click multi-select · drag row to parent · drop on Scene to unparent");
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let mut to_remove = None;
        let mut to_duplicate = None;
        let mut reparent: Option<(String, Option<String>)> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Root drop target — unparent.
            let root_frame = egui::Frame::none()
                .fill(theme::BG_SUNKEN)
                .rounding(egui::Rounding::same(4.0))
                .inner_margin(egui::Margin::symmetric(8.0, 4.0));
            let (_, dropped_root) = ui.dnd_drop_zone(root_frame, |ui| {
                ui.label(
                    RichText::new("Scene")
                        .strong()
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
            });
            if let Some(child) = dropped_root {
                let child: std::sync::Arc<String> = child;
                reparent = Some(((*child).clone(), None));
            }
            ui.add_space(4.0);

            let roots = self.scene.root_names();
            if roots.is_empty() {
                theme::muted(ui, "No entities yet");
            } else {
                for name in roots {
                    self.hierarchy_row(
                        ui,
                        &name,
                        0,
                        &mut to_remove,
                        &mut to_duplicate,
                        &mut reparent,
                    );
                }
            }
        });

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

    fn hierarchy_row(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        depth: u32,
        to_remove: &mut Option<String>,
        to_duplicate: &mut Option<String>,
        reparent: &mut Option<(String, Option<String>)>,
    ) {
        let selected = self.is_selected(name);
        let children = self.scene.child_names(name);
        let indent = 8.0 + depth as f32 * 14.0;

        let row_frame = egui::Frame::none()
            .fill(if selected {
                theme::SELECT_BG
            } else {
                egui::Color32::TRANSPARENT
            })
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin {
                left: indent,
                right: 4.0,
                top: 3.0,
                bottom: 3.0,
            });

        let (inner, dropped) = ui.dnd_drop_zone(row_frame, |ui| {
            ui.horizontal(|ui| {
                // Leave room for Duplicate + Delete.
                let btn_reserve = 52.0;
                let drag_w = (ui.available_width() - btn_reserve).max(48.0);
                // Do not use `dnd_drag_source`: it overlays Sense::drag() on top of the
                // row, and egui then ignores click widgets underneath (drag-only wins).
                // click_and_drag postpones drag until the pointer moves, so click selects.
                let drag_id = egui::Id::new(("hier-drag", name));
                let is_primary = self.primary_selected() == Some(name);
                let label = if selected && is_primary {
                    RichText::new(format!("> {name}"))
                        .strong()
                        .color(theme::SELECT_STROKE)
                } else if selected {
                    RichText::new(format!("+ {name}"))
                        .strong()
                        .color(theme::ACCENT)
                } else {
                    RichText::new(name).color(theme::TEXT)
                };
                if ui.ctx().is_being_dragged(drag_id) {
                    egui::DragAndDrop::set_payload(ui.ctx(), name.to_string());
                    let layer_id = egui::LayerId::new(egui::Order::Tooltip, drag_id);
                    let response = ui
                        .with_layer_id(layer_id, |ui| {
                            ui.add(
                                egui::Label::new(label)
                                    .truncate()
                                    .selectable(false),
                            );
                            let rest = (drag_w - ui.min_rect().width()).max(0.0);
                            if rest > 0.0 {
                                ui.allocate_exact_size(
                                    egui::vec2(rest, ui.spacing().interact_size.y),
                                    egui::Sense::hover(),
                                );
                            }
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
                        ui.add(
                            egui::Label::new(label)
                                .truncate()
                                .selectable(false),
                        );
                        let rest = (drag_w - ui.min_rect().width()).max(0.0);
                        if rest > 0.0 {
                            ui.allocate_exact_size(
                                egui::vec2(rest, ui.spacing().interact_size.y),
                                egui::Sense::hover(),
                            );
                        }
                    });
                    let response = ui
                        .interact(body.response.rect, drag_id, egui::Sense::click_and_drag())
                        .on_hover_cursor(egui::CursorIcon::Grab);
                    if response.clicked() {
                        let cmd = ui.input(|i| i.modifiers.command);
                        if cmd {
                            self.select_toggle(name.to_string());
                        } else {
                            self.select(Some(name.to_string()));
                        }
                    }
                    response.dnd_set_drag_payload(name.to_string());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let del = ui
                        .add_sized(
                            [22.0, 18.0],
                            egui::Button::new(RichText::new("x").size(11.0).color(theme::DANGER))
                                .fill(theme::BG_SUNKEN),
                        )
                        .on_hover_text("Delete (and children)");
                    if del.clicked() {
                        *to_remove = Some(name.to_string());
                    }
                    let dup = ui
                        .add_sized(
                            [22.0, 18.0],
                            egui::Button::new(RichText::new("D").size(11.0)).fill(theme::BG_SUNKEN),
                        )
                        .on_hover_text("Duplicate");
                    if dup.clicked() {
                        *to_duplicate = Some(name.to_string());
                    }
                });
            });
        });
        let _ = inner;
        if let Some(child) = dropped {
            let child: std::sync::Arc<String> = child;
            if child.as_str() != name {
                *reparent = Some(((*child).clone(), Some(name.to_string())));
            }
        }
        ui.add_space(2.0);

        for child in children {
            self.hierarchy_row(ui, &child, depth + 1, to_remove, to_duplicate, reparent);
        }
    }
}
