use eframe::egui::{self, RichText};

use crate::app::{ConsoleLevel, EditorApp};
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_console(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} messages", self.console.len()))
                    .size(11.0)
                    .color(theme::TEXT_DIM),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.console.clear();
                }
                if ui.small_button("Doctor").clicked() {
                    self.doctor();
                }
            });
        });
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("console_scroll")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.console.is_empty() {
                    theme::muted(ui, "doctor warnings and Play logs appear here");
                    return;
                }
                for line in &self.console {
                    let (tag, color) = match line.level {
                        ConsoleLevel::Info => ("info", theme::TEXT),
                        ConsoleLevel::Warn => ("warn", theme::DIRTY),
                        ConsoleLevel::Error => ("error", theme::DANGER),
                    };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.label(
                            RichText::new(tag)
                                .size(11.0)
                                .strong()
                                .color(color)
                                .monospace(),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(&line.text).size(12.0).color(theme::TEXT),
                            )
                            .wrap(),
                        );
                    });
                }
            });
    }
}
