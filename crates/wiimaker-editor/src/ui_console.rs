use eframe::egui::{self, RichText};

use crate::app::{ConsoleLevel, ConsoleLine, EditorApp};
use crate::theme;

/// Case-insensitive substring match on line text **or** the level tag
/// (`info` / `warn` / `error`). Empty / whitespace-only needle matches all.
pub(crate) fn console_line_matches(level: ConsoleLevel, text: &str, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    let n = needle.to_lowercase();
    text.to_lowercase().contains(&n) || level.as_tag().contains(n.as_str())
}

pub(crate) fn console_level_enabled(
    level: ConsoleLevel,
    show_info: bool,
    show_warn: bool,
    show_error: bool,
) -> bool {
    match level {
        ConsoleLevel::Info => show_info,
        ConsoleLevel::Warn => show_warn,
        ConsoleLevel::Error => show_error,
    }
}

/// Visible when the level toggle is on **and** Search matches text or tag.
pub(crate) fn console_line_visible(
    line: &ConsoleLine,
    needle: &str,
    show_info: bool,
    show_warn: bool,
    show_error: bool,
) -> bool {
    console_level_enabled(line.level, show_info, show_warn, show_error)
        && console_line_matches(line.level, &line.text, needle)
}

/// `3 / 12 messages` while Search or a level toggle is narrowing the list.
pub(crate) fn console_count_label(visible: usize, total: usize, filtering: bool) -> String {
    if filtering {
        format!("{visible} / {total} messages")
    } else {
        format!("{total} messages")
    }
}

fn console_is_filtering(
    needle: &str,
    show_info: bool,
    show_warn: bool,
    show_error: bool,
) -> bool {
    !needle.trim().is_empty() || !show_info || !show_warn || !show_error
}

fn level_toggle(ui: &mut egui::Ui, on: &mut bool, tag: &str, color: egui::Color32) {
    let fill = if *on { theme::BG_RAISED } else { theme::BG_SUNKEN };
    let text_color = if *on { color } else { theme::TEXT_DIM };
    let btn = ui.add(
        egui::Button::new(
            RichText::new(tag)
                .size(11.0)
                .strong()
                .monospace()
                .color(text_color),
        )
        .fill(fill)
        .selected(*on),
    );
    if btn.clicked() {
        *on = !*on;
    }
}

impl EditorApp {
    pub(crate) fn ui_console(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            theme::search_icon(ui, 14.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.console_filter)
                    .desired_width(ui.available_width())
                    .hint_text("Search"),
            );
        });
        ui.horizontal(|ui| {
            let filtering = console_is_filtering(
                &self.console_filter,
                self.console_show_info,
                self.console_show_warn,
                self.console_show_error,
            );
            let visible = self
                .console
                .iter()
                .filter(|line| {
                    console_line_visible(
                        line,
                        &self.console_filter,
                        self.console_show_info,
                        self.console_show_warn,
                        self.console_show_error,
                    )
                })
                .count();
            ui.label(
                RichText::new(console_count_label(visible, self.console.len(), filtering))
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
                ui.add_space(4.0);
                level_toggle(ui, &mut self.console_show_error, "error", theme::DANGER);
                level_toggle(ui, &mut self.console_show_warn, "warn", theme::DIRTY);
                level_toggle(ui, &mut self.console_show_info, "info", theme::TEXT);
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
                let mut any = false;
                for line in &self.console {
                    if !console_line_visible(
                        line,
                        &self.console_filter,
                        self.console_show_info,
                        self.console_show_warn,
                        self.console_show_error,
                    ) {
                        continue;
                    }
                    any = true;
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
                if !any {
                    theme::muted(ui, "No matching messages");
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ConsoleLine;

    fn line(level: ConsoleLevel, text: &str) -> ConsoleLine {
        ConsoleLine {
            level,
            text: text.into(),
        }
    }

    #[test]
    fn empty_filter_matches_all_text() {
        assert!(console_line_matches(
            ConsoleLevel::Info,
            "doctor: ok",
            ""
        ));
        assert!(console_line_matches(
            ConsoleLevel::Error,
            "missing clip",
            "   "
        ));
    }

    #[test]
    fn text_match_is_case_insensitive() {
        assert!(console_line_matches(
            ConsoleLevel::Warn,
            "Doctor: Missing PNG",
            "missing"
        ));
        assert!(console_line_matches(
            ConsoleLevel::Info,
            "Play started",
            "PLAY"
        ));
        assert!(!console_line_matches(
            ConsoleLevel::Info,
            "Play started",
            "clip"
        ));
    }

    #[test]
    fn level_tag_match_is_case_insensitive() {
        assert!(console_line_matches(
            ConsoleLevel::Error,
            "boom",
            "error"
        ));
        assert!(console_line_matches(ConsoleLevel::Warn, "slow", "WARN"));
        assert!(console_line_matches(ConsoleLevel::Info, "hi", "InFo"));
        assert!(!console_line_matches(
            ConsoleLevel::Info,
            "hi",
            "error"
        ));
    }

    #[test]
    fn level_toggles_hide_rows_even_when_text_matches() {
        let warn = line(ConsoleLevel::Warn, "doctor: missing png");
        assert!(console_line_visible(&warn, "missing", true, true, true));
        assert!(!console_line_visible(&warn, "missing", true, false, true));
        assert!(console_line_visible(&warn, "", true, true, false));
        assert!(!console_line_visible(
            &line(ConsoleLevel::Error, "fail"),
            "",
            true,
            true,
            false
        ));
    }

    #[test]
    fn count_label_shows_filtered_over_total() {
        assert_eq!(console_count_label(12, 12, false), "12 messages");
        assert_eq!(console_count_label(3, 12, true), "3 / 12 messages");
        assert_eq!(console_count_label(0, 5, true), "0 / 5 messages");
    }

    #[test]
    fn filtering_flag_covers_text_and_level_toggles() {
        assert!(!console_is_filtering("", true, true, true));
        assert!(console_is_filtering("err", true, true, true));
        assert!(console_is_filtering("", true, false, true));
    }
}
