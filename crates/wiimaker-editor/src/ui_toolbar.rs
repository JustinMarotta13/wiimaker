use eframe::egui::{self, RichText};

use crate::app::{EditorApp, PlayMode};
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(theme::menu_frame())
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.label(
                        RichText::new("wiimaker")
                            .strong()
                            .size(14.0)
                            .color(theme::ACCENT),
                    );
                    ui.add_space(6.0);
                    ui.menu_button("File", |ui| {
                        if ui.button("Save scene").clicked() {
                            self.save();
                            ui.close_menu();
                        }
                        if ui.button("Doctor").clicked() {
                            self.doctor();
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

                    ui.separator();
                    if ui
                        .add_enabled(self.dirty, egui::Button::new("Save"))
                        .on_hover_text("Cmd/Ctrl+S")
                        .clicked()
                    {
                        self.save();
                    }
                    match self.play_mode {
                        PlayMode::Edit => {
                            if ui
                                .button("Play")
                                .on_hover_text("In-editor Play Mode (WASD)")
                                .clicked()
                            {
                                self.play();
                            }
                        }
                        PlayMode::Playing => {
                            if ui.button("Pause").clicked() {
                                self.play();
                            }
                            if ui.button("Stop").clicked() {
                                self.stop_play();
                            }
                        }
                        PlayMode::Paused => {
                            if ui.button("Resume").clicked() {
                                self.play();
                            }
                            if ui.button("Stop").clicked() {
                                self.stop_play();
                            }
                        }
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
                    {
                        let prefab_rels: Vec<_> = self
                            .project_entries
                            .iter()
                            .filter(|e| {
                                !e.is_dir && e.rel.to_string_lossy().ends_with(".prefab.json")
                            })
                            .map(|e| e.rel.clone())
                            .collect();
                        if prefab_rels.len() == 1 {
                            let rel = &prefab_rels[0];
                            let stem = rel
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("prefab")
                                .trim_end_matches(".prefab.json");
                            if ui
                                .button(format!("Instantiate {stem}"))
                                .on_hover_text(rel.display().to_string())
                                .clicked()
                            {
                                self.instantiate_prefab_rel(rel);
                            }
                        } else {
                            ui.menu_button("Prefab", |ui| {
                                if prefab_rels.is_empty() {
                                    ui.label(
                                        RichText::new(
                                            "No prefabs yet — Save as Prefab… on an entity",
                                        )
                                        .size(12.0)
                                        .color(theme::TEXT_MUTED),
                                    );
                                }
                                for rel in &prefab_rels {
                                    let stem = rel
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("prefab")
                                        .trim_end_matches(".prefab.json");
                                    if ui
                                        .button(format!("Instantiate {stem}"))
                                        .on_hover_text(rel.display().to_string())
                                        .clicked()
                                    {
                                        self.instantiate_prefab_rel(rel);
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    }
                    ui.menu_button("⋯", |ui| {
                        if ui.button("Cook assets…").clicked() {
                            self.cook();
                            ui.close_menu();
                        }
                        if ui.button("Doctor").clicked() {
                            self.doctor();
                            ui.close_menu();
                        }
                        if ui.button("Refresh assets").clicked() {
                            if let Err(e) = self.reload_assets() {
                                self.status = format!("refresh failed: {e}");
                            } else {
                                self.rehydrate();
                                self.status = "assets refreshed".into();
                            }
                            ui.close_menu();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&self.status).size(12.0).color(theme::TEXT_MUTED));
                        ui.separator();
                        if self.dirty {
                            ui.label(
                                RichText::new("unsaved")
                                    .strong()
                                    .size(12.0)
                                    .color(theme::DIRTY),
                            );
                        } else {
                            ui.label(
                                RichText::new("saved")
                                    .size(12.0)
                                    .color(theme::SAVED),
                            );
                        }
                    });
                });
            });
    }
}
