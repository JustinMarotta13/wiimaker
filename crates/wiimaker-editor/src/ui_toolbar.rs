use eframe::egui::{self, RichText};

use crate::app::{EditorApp, PlayMode};
use crate::theme::{self, PlayBtn};

impl EditorApp {
    pub(crate) fn ui_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(theme::menu_frame())
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.label(
                        RichText::new("wiimaker")
                            .strong()
                            .size(13.0)
                            .color(theme::ACCENT),
                    );
                    ui.add_space(8.0);
                    ui.menu_button("File", |ui| {
                        if ui.button("Save scene").clicked() {
                            self.save();
                            ui.close_menu();
                        }
                        if ui.button("Doctor").clicked() {
                            self.doctor();
                            ui.close_menu();
                        }
                        if ui.button("Build Settings…").clicked() {
                            self.show_build_settings = true;
                            ui.close_menu();
                        }
                        if ui.button("Play").clicked() {
                            self.play();
                            ui.close_menu();
                        }
                        if ui.button("Stop Play").clicked() {
                            self.stop_play();
                            ui.close_menu();
                        }
                        if ui.button("Run external…").clicked() {
                            self.play_external();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Build").clicked() {
                            self.build_wii();
                            ui.close_menu();
                        }
                        if ui.button("Play in Dolphin").clicked() {
                            self.play_dolphin();
                            ui.close_menu();
                        }
                        if ui.button("Build & Run").clicked() {
                            self.build_and_run_wii();
                            ui.close_menu();
                        }
                        ui.separator();
                        let prefab_rels: Vec<_> = self
                            .project_entries
                            .iter()
                            .filter(|e| {
                                !e.is_dir && e.rel.to_string_lossy().ends_with(".prefab.json")
                            })
                            .map(|e| e.rel.clone())
                            .collect();
                        for rel in &prefab_rels {
                            let stem = rel
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("prefab")
                                .trim_end_matches(".prefab.json");
                            if ui
                                .button(format!("Instantiate {stem}"))
                                .on_hover_text("Cmd/Ctrl+I")
                                .clicked()
                            {
                                self.instantiate_prefab_rel(rel);
                                ui.close_menu();
                            }
                        }
                        if prefab_rels.is_empty() {
                            ui.label(
                                RichText::new("No prefabs yet")
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(self.undo.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            self.do_undo();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(self.undo.can_redo(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            self.do_redo();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(!self.selected.is_empty(), egui::Button::new("Duplicate"))
                            .clicked()
                        {
                            self.do_duplicate();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(!self.selected.is_empty(), egui::Button::new("Copy"))
                            .clicked()
                        {
                            self.do_copy();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(self.clipboard.is_some(), egui::Button::new("Paste"))
                            .clicked()
                        {
                            self.do_paste();
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Window", |ui| {
                        use crate::dock::EditorTab;
                        if ui.button("Hierarchy").clicked() {
                            self.focus_tab(EditorTab::Hierarchy);
                            ui.close_menu();
                        }
                        if ui.button("Inspector").clicked() {
                            self.focus_tab(EditorTab::Inspector);
                            ui.close_menu();
                        }
                        if ui.button("Scene").clicked() {
                            self.focus_tab(EditorTab::Scene);
                            ui.close_menu();
                        }
                        if ui.button("Game").clicked() {
                            self.focus_tab(EditorTab::Game);
                            ui.close_menu();
                        }
                        if ui.button("Project").clicked() {
                            self.focus_tab(EditorTab::Project);
                            ui.close_menu();
                        }
                        if ui.button("Console").clicked() {
                            self.focus_tab(EditorTab::Console);
                            ui.close_menu();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&self.status)
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                });
            });

        egui::TopBottomPanel::top("play_toolbar")
            .exact_height(36.0)
            .frame(theme::toolbar_frame())
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.set_clip_rect(full);
                let _ = ui.allocate_rect(full, egui::Sense::hover());
                let play_w = 120.0;
                let mid = egui::Rect::from_center_size(
                    full.center(),
                    egui::vec2(play_w, full.height()),
                );
                let left = egui::Rect::from_min_max(
                    full.left_top(),
                    egui::pos2(mid.left() - 4.0, full.bottom()),
                );
                let right = egui::Rect::from_min_max(
                    egui::pos2(mid.right() + 4.0, full.top()),
                    full.right_bottom(),
                );

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if ui
                            .add_enabled(self.dirty, egui::Button::new("Save"))
                            .on_hover_text("Cmd/Ctrl+S")
                            .clicked()
                        {
                            self.save();
                        }
                        if ui.button("Build").on_hover_text("Wii .dol").clicked() {
                            self.build_wii();
                        }
                        if ui
                            .button("Play in Dolphin")
                            .on_hover_text("Launch existing boot.dol")
                            .clicked()
                        {
                            self.play_dolphin();
                        }
                        if ui
                            .button("Build & Run")
                            .on_hover_text("Build then Dolphin")
                            .clicked()
                        {
                            self.build_and_run_wii();
                        }
                        theme::icon_menu_button(ui, "toolbar_overflow", theme::MenuIcon::Ellipsis, |ui| {
                            if ui.button("Cook assets...").clicked() {
                                self.cook();
                            }
                            if ui.button("Doctor").clicked() {
                                self.doctor();
                            }
                            if ui.button("Refresh assets").clicked() {
                                if let Err(e) = self.reload_assets() {
                                    self.status = format!("refresh failed: {e}");
                                } else {
                                    self.rehydrate();
                                    self.status = "assets refreshed".into();
                                }
                            }
                        });
                    });
                });

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(mid), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let playing = self.play_mode == PlayMode::Playing;
                        let paused = self.play_mode == PlayMode::Paused;
                        let in_play = self.play_mode != PlayMode::Edit;
                        if theme::play_control(ui, PlayBtn::Play, playing, true)
                            .on_hover_text("Play")
                            .clicked()
                            && self.play_mode != PlayMode::Playing
                        {
                            self.play();
                        }
                        if theme::play_control(ui, PlayBtn::Pause, paused, in_play)
                            .on_hover_text("Pause")
                            .clicked()
                        {
                            self.play();
                        }
                        if theme::play_control(ui, PlayBtn::Stop, false, in_play)
                            .on_hover_text("Stop")
                            .clicked()
                        {
                            self.stop_play();
                        }
                    });
                });

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.dirty {
                            ui.label(
                                RichText::new("unsaved")
                                    .strong()
                                    .size(11.0)
                                    .color(theme::DIRTY),
                            );
                        } else {
                            ui.label(RichText::new("saved").size(11.0).color(theme::SAVED));
                        }
                    });
                });
            });
    }
}
