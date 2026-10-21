use eframe::egui::{self, Color32, RichText, Rounding, Sense, Shape, Stroke};

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

/// One Console row. `first` is the earliest source line; `count` is how many
/// lines share its level and text when Collapse is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConsoleGroup {
    pub(crate) first: usize,
    pub(crate) count: usize,
}

/// Collapse groups identical lines (same level + same text), including repeats
/// that are not adjacent. Order is first-seen. Collapse off is one group per line.
pub(crate) fn console_collapse_groups(lines: &[ConsoleLine], collapse: bool) -> Vec<ConsoleGroup> {
    if !collapse {
        return (0..lines.len())
            .map(|first| ConsoleGroup { first, count: 1 })
            .collect();
    }
    let mut groups: Vec<ConsoleGroup> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|group| {
            let prev = &lines[group.first];
            prev.level == line.level && prev.text == line.text
        }) {
            group.count += 1;
        } else {
            groups.push(ConsoleGroup { first: i, count: 1 });
        }
    }
    groups
}

/// Raw messages of `level`. Collapse and Search do not change this.
pub(crate) fn console_level_count(lines: &[ConsoleLine], level: ConsoleLevel) -> usize {
    lines.iter().filter(|line| line.level == level).count()
}

/// `3 / 12 messages` while Search or a level toggle is narrowing the list.
/// `visible` and `total` are raw message counts, not collapsed row counts.
pub(crate) fn console_count_label(visible: usize, total: usize, filtering: bool) -> String {
    if filtering {
        format!("{visible} / {total} messages")
    } else {
        format!("{total} messages")
    }
}

fn console_is_filtering(needle: &str, show_info: bool, show_warn: bool, show_error: bool) -> bool {
    !needle.trim().is_empty() || !show_info || !show_warn || !show_error
}

fn level_toggle(ui: &mut egui::Ui, on: &mut bool, tag: &str, count: usize, color: egui::Color32) {
    let fill = if *on {
        theme::BG_RAISED
    } else {
        theme::BG_SUNKEN
    };
    let text_color = if *on { color } else { theme::TEXT_DIM };
    let btn = ui.add(
        egui::Button::new(
            RichText::new(format!("{tag} {count}"))
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

fn mode_toggle(ui: &mut egui::Ui, on: bool, label: &str) -> egui::Response {
    let fill = if on {
        theme::BG_RAISED
    } else {
        theme::BG_SUNKEN
    };
    let text_color = if on { theme::TEXT } else { theme::TEXT_DIM };
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(text_color))
            .fill(fill)
            .selected(on)
            .min_size(egui::vec2(0.0, 22.0)),
    )
}

fn clear_drop_arrow(ui: &mut egui::Ui, open: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), Sense::click());
    let bg = if open || resp.hovered() {
        Color32::from_rgb(80, 80, 80)
    } else {
        theme::BG_RAISED
    };
    ui.painter().rect_filled(rect, Rounding::same(3.0), bg);
    let c = rect.center();
    let s = 3.5;
    ui.painter().add(Shape::convex_polygon(
        vec![
            egui::pos2(c.x - s, c.y - s * 0.35),
            egui::pos2(c.x + s, c.y - s * 0.35),
            egui::pos2(c.x, c.y + s * 0.7),
        ],
        theme::TEXT,
        Stroke::NONE,
    ));
    resp
}

/// Message column width after the level tag, reserving space for the count badge.
pub(crate) fn collapsed_message_max_width(
    row_right: f32,
    text_start_x: f32,
    show_badge: bool,
) -> f32 {
    let badge_reserve = if show_badge { 36.0 } else { 0.0 };
    (row_right - text_start_x - badge_reserve).max(1.0)
}

fn paint_count_badge(ui: &egui::Ui, row: egui::Rect, count: usize) {
    let brect = egui::Rect::from_center_size(
        egui::pos2(row.right() - 16.0, row.center().y),
        egui::vec2(24.0, 16.0),
    );
    ui.painter()
        .rect_filled(brect, Rounding::same(8.0), theme::BG_RAISED);
    ui.painter().text(
        brect.center(),
        egui::Align2::CENTER_CENTER,
        count.to_string(),
        egui::FontId::proportional(11.0),
        theme::TEXT,
    );
}

fn pref_check(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if theme::enable_checkbox(ui, on).clicked() {
            clicked = true;
        }
        if ui.selectable_label(false, label).clicked() {
            clicked = true;
        }
    });
    clicked
}

impl EditorApp {
    pub(crate) fn ui_console(&mut self, ui: &mut egui::Ui) {
        let filtering = console_is_filtering(
            &self.console_filter,
            self.console_show_info,
            self.console_show_warn,
            self.console_show_error,
        );
        let groups = console_collapse_groups(&self.console, self.prefs.console.collapse);
        let visible_messages: usize = groups
            .iter()
            .filter(|group| {
                console_line_visible(
                    &self.console[group.first],
                    &self.console_filter,
                    self.console_show_info,
                    self.console_show_warn,
                    self.console_show_error,
                )
            })
            .map(|group| group.count)
            .sum();
        let info_n = console_level_count(&self.console, ConsoleLevel::Info);
        let warn_n = console_level_count(&self.console, ConsoleLevel::Warn);
        let error_n = console_level_count(&self.console, ConsoleLevel::Error);

        let mut clear_on_play = None;
        let mut clear_on_build = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui
                .add(
                    egui::Button::new(RichText::new("Clear").size(12.0).color(theme::TEXT))
                        .fill(theme::BG_RAISED)
                        .min_size(egui::vec2(0.0, 22.0)),
                )
                .clicked()
            {
                self.console.clear();
            }
            let popup_id = ui.make_persistent_id("console_clear_menu");
            let menu_open = ui.memory(|mem| mem.is_popup_open(popup_id));
            let arrow = clear_drop_arrow(ui, menu_open || self.force_console_menu);
            if arrow.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }
            if self.force_console_menu {
                ui.memory_mut(|mem| mem.open_popup(popup_id));
            }
            egui::popup::popup_below_widget(
                ui,
                popup_id,
                &arrow,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(168.0);
                    if pref_check(ui, "Clear on Play", self.prefs.console.clear_on_play) {
                        clear_on_play = Some(!self.prefs.console.clear_on_play);
                    }
                    if pref_check(ui, "Clear on Build", self.prefs.console.clear_on_build) {
                        clear_on_build = Some(!self.prefs.console.clear_on_build);
                    }
                },
            );
            if mode_toggle(ui, self.prefs.console.collapse, "Collapse").clicked() {
                self.prefs.console.collapse = !self.prefs.console.collapse;
                self.persist_prefs();
            }
            if mode_toggle(ui, self.prefs.console.error_pause, "Error Pause").clicked() {
                self.prefs.console.error_pause = !self.prefs.console.error_pause;
                self.persist_prefs();
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Doctor").size(12.0).color(theme::TEXT))
                        .fill(theme::BG_RAISED)
                        .min_size(egui::vec2(0.0, 22.0)),
                )
                .clicked()
            {
                self.doctor();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                level_toggle(
                    ui,
                    &mut self.console_show_error,
                    "error",
                    error_n,
                    theme::DANGER,
                );
                level_toggle(
                    ui,
                    &mut self.console_show_warn,
                    "warn",
                    warn_n,
                    theme::DIRTY,
                );
                level_toggle(ui, &mut self.console_show_info, "info", info_n, theme::TEXT);
                ui.label(
                    RichText::new(console_count_label(
                        visible_messages,
                        self.console.len(),
                        filtering,
                    ))
                    .size(11.0)
                    .color(theme::TEXT_DIM),
                );
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    theme::search_icon(ui, 14.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.console_filter)
                            .desired_width(ui.available_width())
                            .hint_text("Search"),
                    );
                });
            });
        });
        if let Some(on) = clear_on_play {
            self.prefs.console.clear_on_play = on;
            self.persist_prefs();
        }
        if let Some(on) = clear_on_build {
            self.prefs.console.clear_on_build = on;
            self.persist_prefs();
        }

        ui.add_space(4.0);
        let collapse = self.prefs.console.collapse;
        let groups = console_collapse_groups(&self.console, collapse);
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
                for group in groups {
                    let line = &self.console[group.first];
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
                    let full = ui.available_rect_before_wrap();
                    let show_badge = collapse && group.count > 1;
                    let resp = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.label(
                            RichText::new(tag)
                                .size(11.0)
                                .strong()
                                .color(color)
                                .monospace(),
                        );
                        let text_start = ui.cursor().left();
                        let text_w =
                            collapsed_message_max_width(full.right(), text_start, show_badge);
                        ui.add_sized(
                            egui::vec2(text_w, 0.0),
                            egui::Label::new(
                                RichText::new(&line.text).size(12.0).color(theme::TEXT),
                            )
                            .wrap(),
                        );
                    });
                    if show_badge {
                        let row = egui::Rect::from_min_max(
                            egui::pos2(full.left(), resp.response.rect.top()),
                            egui::pos2(full.right(), resp.response.rect.bottom()),
                        );
                        paint_count_badge(ui, row, group.count);
                    }
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
        assert!(console_line_matches(ConsoleLevel::Info, "doctor: ok", ""));
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
        assert!(console_line_matches(ConsoleLevel::Error, "boom", "error"));
        assert!(console_line_matches(ConsoleLevel::Warn, "slow", "WARN"));
        assert!(console_line_matches(ConsoleLevel::Info, "hi", "InFo"));
        assert!(!console_line_matches(ConsoleLevel::Info, "hi", "error"));
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

    #[test]
    fn collapse_groups_repeats_and_keeps_first_seen_order() {
        let lines = vec![
            line(ConsoleLevel::Info, "started"),
            line(ConsoleLevel::Error, "play tick: boom"),
            line(ConsoleLevel::Warn, "doctor: missing png"),
            line(ConsoleLevel::Error, "play tick: boom"),
            line(ConsoleLevel::Info, "started"),
            line(ConsoleLevel::Error, "play tick: other"),
            line(ConsoleLevel::Error, "play tick: boom"),
        ];
        let open = console_collapse_groups(&lines, false);
        assert_eq!(open.len(), lines.len());
        assert!(open.iter().all(|g| g.count == 1));

        let groups = console_collapse_groups(&lines, true);
        assert_eq!(
            groups,
            vec![
                ConsoleGroup { first: 0, count: 2 },
                ConsoleGroup { first: 1, count: 3 },
                ConsoleGroup { first: 2, count: 1 },
                ConsoleGroup { first: 5, count: 1 },
            ]
        );
        let visible: usize = groups
            .iter()
            .filter(|g| console_line_visible(&lines[g.first], "boom", true, true, true))
            .map(|g| g.count)
            .sum();
        assert_eq!(visible, 3);
        assert_eq!(console_level_count(&lines, ConsoleLevel::Error), 4);
        assert_eq!(console_level_count(&lines, ConsoleLevel::Info), 2);
    }

    #[test]
    fn collapse_does_not_merge_different_levels() {
        let lines = vec![
            line(ConsoleLevel::Info, "same"),
            line(ConsoleLevel::Error, "same"),
        ];
        let groups = console_collapse_groups(&lines, true);
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|g| g.count == 1));
    }

    #[test]
    fn collapsed_message_width_reserves_level_tag_and_badge() {
        let row_left = 10.0;
        let row_right = 410.0;
        let tag_w = 36.0;
        let text_start = row_left + tag_w;
        let w = collapsed_message_max_width(row_right, text_start, true);
        assert_eq!(w, row_right - text_start - 36.0);
        assert_eq!(collapsed_message_max_width(row_right, text_start, false), 364.0);
        assert_eq!(collapsed_message_max_width(row_right, row_right, true), 1.0);
    }
}
