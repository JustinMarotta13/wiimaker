use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, Sense};

use crate::app::{EditorApp, ProjectEntry};
use crate::theme;

impl EditorApp {
    pub(crate) fn ui_project_body(&mut self, ui: &mut egui::Ui) {
                ui.horizontal(|ui| {
                    theme::meta_chip(ui, "game", &self.project.name);
                    ui.separator();
                    theme::meta_chip(ui, "scene", &self.scene.name);
                    ui.separator();
                    theme::meta_chip(ui, "files", &self.project_entries.len().to_string());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("Refresh")
                            .on_hover_text("Rescan assets / scenes")
                            .clicked()
                        {
                            self.refresh_project_tree();
                            self.refresh_scenes();
                            if let Err(e) = self.reload_assets() {
                                self.status = format!("refresh failed: {e}");
                            } else {
                                self.status = "project refreshed".into();
                            }
                        }
                    });
                });
                theme::muted(ui, "Drop PNG files anywhere to import + prepare assets");
                // Prefab quick actions stay above the scroll list so Instantiate is always visible.
                let prefabs: Vec<_> = self
                    .project_entries
                    .iter()
                    .filter(|e| {
                        !e.is_dir && e.rel.to_string_lossy().ends_with(".prefab.json")
                    })
                    .cloned()
                    .collect();
                if !prefabs.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new("Prefabs")
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        for p in &prefabs {
                            let stem = p
                                .rel
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("prefab")
                                .trim_end_matches(".prefab.json");
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(format!("Instantiate {stem}")).size(12.0),
                                    )
                                    .fill(theme::BG_SUNKEN),
                                )
                                .on_hover_text(format!("Spawn {} into the open scene", p.rel.display()))
                                .clicked()
                            {
                                self.instantiate_prefab_rel(&p.rel);
                            }
                        }
                    });
                }
                ui.add_space(4.0);

                let list_h = (ui.available_height() - 36.0).max(48.0);
                theme::card_frame().show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("project_tree")
                        .auto_shrink([false, false])
                        .max_height(list_h)
                        .show(ui, |ui| {
                            let entries = self.project_entries.clone();
                            let scene_path = self.scene_path.clone();
                            let game_dir = self.game_dir.clone();
                            if entries.is_empty() {
                                theme::muted(ui, "No project files found");
                            }
                            for entry in &entries {
                                let selected = self.selected_file.as_ref() == Some(&entry.rel);
                                let is_open_scene =
                                    !entry.is_dir && game_dir.join(&entry.rel) == scene_path;
                                let resp = project_row(ui, entry, selected, is_open_scene);
                                if resp.clicked() {
                                    self.select_file(Some(entry.rel.clone()));
                                }
                                if resp.double_clicked() && !entry.is_dir {
                                    self.open_project_entry(entry);
                                }
                                resp.context_menu(|ui| {
                                    self.project_entry_context_menu(ui, entry);
                                });
                            }
                        });
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_scene_name)
                            .desired_width(120.0)
                            .hint_text("new scene name"),
                    );
                    if ui
                        .add(egui::Button::new(
                            RichText::new("New scene").color(theme::TEXT),
                        ))
                        .clicked()
                    {
                        self.create_new_scene();
                        self.refresh_project_tree();
                    }
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Set default").color(theme::TEXT),
                        ))
                        .on_hover_text("Persist current scene as game.toml default")
                        .clicked()
                    {
                        self.set_as_default_scene();
                    }
                });
    }

    fn open_project_entry(&mut self, entry: &ProjectEntry) {
        let abs = self.game_dir.join(&entry.rel);
        let name = entry.rel.to_string_lossy();
        if name.ends_with(".scene.json") {
            self.request_open_scene(abs);
        } else if name.ends_with(".prefab.json") {
            self.instantiate_prefab_rel(&entry.rel);
        } else if entry.rel.extension().and_then(|e| e.to_str()) == Some("png") {
            if let Some(stem) = entry.rel.file_stem().and_then(|s| s.to_str()) {
                self.open_sprite_editor_stem = Some(stem.into());
            }
        } else if name.ends_with(".sprites.json") {
            if let Some(stem) = entry
                .rel
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".sprites").to_string())
            {
                self.open_sprite_editor_stem = Some(stem);
            }
        }
    }

    fn project_entry_context_menu(&mut self, ui: &mut egui::Ui, entry: &ProjectEntry) {
        if entry.rel.extension().and_then(|e| e.to_str()) == Some("png") {
            if ui.button("Edit Sprites…").clicked() {
                if let Some(stem) = entry.rel.file_stem().and_then(|s| s.to_str()) {
                    self.open_sprite_editor_stem = Some(stem.into());
                }
                ui.close_menu();
            }
        }
        if entry.rel.to_string_lossy().ends_with(".scene.json") {
            if ui.button("Open scene").clicked() {
                self.request_open_scene(self.game_dir.join(&entry.rel));
                ui.close_menu();
            }
        }
        if entry.rel.to_string_lossy().ends_with(".prefab.json") {
            if ui.button("Instantiate in scene").clicked() {
                self.instantiate_prefab_rel(&entry.rel);
                ui.close_menu();
            }
        }
        if ui.button("Reveal in Inspector").clicked() {
            self.select_file(Some(entry.rel.clone()));
            ui.close_menu();
        }
    }
}

fn project_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    selected: bool,
    is_open_scene: bool,
) -> egui::Response {
    let indent = 6.0 + entry.depth as f32 * 16.0;
    let (marker, name_color, kind) = entry_visuals(entry, is_open_scene);
    let file_name = entry
        .rel
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?");
    let name = if entry.is_dir {
        format!("{file_name}/")
    } else {
        file_name.to_string()
    };
    let name_text = if selected {
        RichText::new(name).strong().size(12.5).color(theme::SELECT_STROKE)
    } else if entry.is_dir {
        RichText::new(name).strong().size(12.5).color(name_color)
    } else {
        RichText::new(name).size(12.5).color(name_color)
    };

    let id = ui.id().with(("proj_row", entry.rel.as_os_str()));
    let row_h = ui.spacing().interact_size.y.max(22.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());

    let bg = if selected {
        theme::SELECT_BG
    } else if resp.hovered() {
        Color32::from_rgb(36, 42, 54)
    } else {
        Color32::TRANSPARENT
    };
    if bg.a() > 0 {
        ui.painter()
            .rect_filled(rect, egui::Rounding::same(4.0), bg);
    }

    let kind_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            kind.to_string(),
            egui::FontId::proportional(11.0),
            theme::TEXT_DIM,
        )
    });
    let kind_w = kind_galley.size().x + 14.0;
    let text_right = rect.right() - kind_w;

    // Marker + name in a clipped strip that stops before the kind tag.
    // Must intersect ScrollArea clip — replacing it lets scrolled-off rows paint over the header.
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top()),
        egui::pos2(text_right.max(rect.left() + 40.0), rect.bottom()),
    );
    let clip = text_rect.intersect(ui.clip_rect());
    if clip.height() > 0.5 {
        let mut text_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(text_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        text_ui.set_clip_rect(clip);
        text_ui.add_space(indent);
        text_ui.label(RichText::new(marker).size(12.0).color(name_color));
        text_ui.add(egui::Label::new(name_text).truncate());
    }

    ui.painter().galley(
        egui::pos2(
            rect.right() - kind_w + 4.0,
            rect.center().y - kind_galley.size().y * 0.5,
        ),
        kind_galley,
        theme::TEXT_DIM,
    );

    if is_open_scene && !selected {
        ui.painter().circle_filled(
            egui::pos2(rect.right() - 4.0, rect.center().y),
            2.5,
            theme::ACCENT,
        );
    }

    resp.on_hover_text(entry.rel.to_string_lossy())
}

fn entry_visuals(entry: &ProjectEntry, is_open_scene: bool) -> (&'static str, Color32, &'static str) {
    if entry.is_dir {
        return ("#", theme::ACCENT, "Folder");
    }
    let name = entry.rel.to_string_lossy();
    if name.ends_with(".scene.json") {
        let color = if is_open_scene {
            theme::SELECT_STROKE
        } else {
            theme::ACCENT
        };
        ("*", color, "Scene")
    } else if name.ends_with(".prefab.json") {
        ("P", theme::ACCENT, "Prefab")
    } else if name.ends_with(".sprites.json") {
        ("=", theme::TEXT, "Sprites")
    } else if name.ends_with(".anim.json") {
        (">", theme::ACCENT, "Anim")
    } else if name == "game.toml" {
        ("@", theme::SELECT_STROKE, "Project")
    } else {
        match entry.rel.extension().and_then(|e| e.to_str()) {
            Some("png") => ("~", theme::TEXT, "PNG"),
            Some("toml") => ("@", theme::TEXT, "TOML"),
            Some("json") => ("=", theme::TEXT, "JSON"),
            Some("wpack") => ("$", theme::TEXT, "Pack"),
            Some("wscn") => ("*", theme::TEXT, "Baked"),
            _ => ("-", theme::TEXT, "File"),
        }
    }
}

pub(crate) fn push_dir_entries(
    abs_dir: &Path,
    rel_dir: PathBuf,
    depth: u32,
    out: &mut Vec<ProjectEntry>,
) {
    let mut kids: Vec<(PathBuf, bool)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(abs_dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let is_dir = path.is_dir();
        kids.push((rel_dir.join(name), is_dir));
    }
    kids.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.cmp(&b.0),
    });
    for (rel, is_dir) in kids {
        out.push(ProjectEntry {
            rel: rel.clone(),
            is_dir,
            depth,
        });
        if is_dir {
            push_dir_entries(&abs_dir.join(rel.file_name().unwrap()), rel, depth + 1, out);
        }
    }
}

pub(crate) fn file_kind_label(rel: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "Folder";
    }
    let name = rel.to_string_lossy();
    if name.ends_with(".scene.json") {
        "Scene"
    } else if name.ends_with(".prefab.json") {
        "Prefab"
    } else if name.ends_with(".sprites.json") {
        "Sprite sheet meta"
    } else if name.ends_with(".anim.json") {
        "Animation clip"
    } else if name == "game.toml" {
        "Project"
    } else {
        match rel.extension().and_then(|e| e.to_str()) {
            Some("png") => "PNG texture",
            Some("json") => "JSON",
            Some("toml") => "TOML",
            Some("wpack") => "Asset pack",
            Some("wscn") => "Baked scene",
            _ => "File",
        }
    }
}

pub(crate) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let n = n as f64;
    if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.1} KB", n / KB)
    } else {
        format!("{n:.0} B")
    }
}
