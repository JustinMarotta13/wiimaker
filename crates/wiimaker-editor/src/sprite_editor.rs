//! egui Sprite Editor — Grid By Cell Count + pivot (Unity-scoped).

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};
use wiimaker_assets::{SpriteCell, SpriteSheetMeta};

use crate::theme;

#[derive(Clone)]
pub struct SpriteEditorState {
    pub sheet_stem: String,
    pub sidecar_path: PathBuf,
    pub columns: u32,
    pub rows: u32,
    /// 0 = Center, 1 = Custom (for Slice panel default).
    pub pivot_mode: usize,
    pub custom_pivot: [f32; 2],
    pub selected: Option<usize>,
    pub meta: SpriteSheetMeta,
    pub sheet_w: u32,
    pub sheet_h: u32,
    pub texture: Option<egui::TextureHandle>,
    pub dirty: bool,
}

impl SpriteEditorState {
    pub fn open(assets_dir: &std::path::Path, stem: &str, ctx: &egui::Context) -> anyhow::Result<Self> {
        let png_path = assets_dir.join(format!("{stem}.png"));
        let sidecar_path = SpriteSheetMeta::sidecar_path(&png_path);
        let (sheet_w, sheet_h) = image::image_dimensions(&png_path)?;
        let meta = if sidecar_path.is_file() {
            SpriteSheetMeta::load(&sidecar_path)?
        } else {
            SpriteSheetMeta {
                columns: 1,
                rows: 1,
                sprites: vec![SpriteCell {
                    name: stem.to_string(),
                    rect: [0, 0, sheet_w, sheet_h],
                    pivot: [0.5, 0.5],
                }],
            }
        };

        let texture = load_sheet_texture(ctx, &png_path, stem)?;

        Ok(Self {
            sheet_stem: stem.to_string(),
            sidecar_path,
            columns: meta.columns.max(1),
            rows: meta.rows.max(1),
            pivot_mode: 0,
            custom_pivot: [0.5, 0.5],
            selected: None,
            meta,
            sheet_w,
            sheet_h,
            texture,
            dirty: false,
        })
    }

    pub fn slice(&mut self) -> anyhow::Result<Vec<String>> {
        let warnings = self.meta.slice_grid(
            self.sheet_w,
            self.sheet_h,
            self.columns,
            self.rows,
            &self.sheet_stem,
        )?;
        let default_pivot = if self.pivot_mode == 0 {
            [0.5, 0.5]
        } else {
            self.custom_pivot
        };
        for cell in &mut self.meta.sprites {
            // Only overwrite if still default center from preserve, and mode is custom —
            // actually Unity applies pivot to all on slice. Apply to cells that were just created
            // with preserved or default; if custom mode, set all to custom unless preserved?
            // Plan: Pivot dropdown applies as default for new cells; preserve keeps old.
            // slice_grid already preserves by name. For brand-new names, use dropdown default.
            if cell.pivot == [0.5, 0.5] && self.pivot_mode == 1 {
                cell.pivot = default_pivot;
            }
        }
        self.meta.save(&self.sidecar_path)?;
        self.dirty = false;
        self.selected = None;
        Ok(warnings)
    }

    pub fn save_meta(&mut self) -> anyhow::Result<()> {
        self.meta.save(&self.sidecar_path)?;
        self.dirty = false;
        Ok(())
    }
}

fn load_sheet_texture(
    ctx: &egui::Context,
    png_path: &std::path::Path,
    stem: &str,
) -> anyhow::Result<Option<egui::TextureHandle>> {
    let img = image::open(png_path)?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.into_raw();
    let color = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    Ok(Some(ctx.load_texture(
        format!("sprite_editor_{stem}"),
        color,
        egui::TextureOptions::NEAREST,
    )))
}

/// Draw the Sprite Editor window. Returns true if catalog should refresh.
pub fn show_sprite_editor(
    ctx: &egui::Context,
    state: &mut SpriteEditorState,
    open: &mut bool,
) -> bool {
    let mut refresh = false;
    let mut close = false;
    egui::Window::new(format!("Sprite Editor · {}", state.sheet_stem))
        .open(open)
        .default_size([720.0, 480.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Canvas
                ui.vertical(|ui| {
                    ui.set_min_width(420.0);
                    theme::card_frame().show(ui, |ui| {
                        let avail = ui.available_size();
                        let scale = ((avail.x - 8.0) / state.sheet_w as f32)
                            .min((avail.y - 8.0).max(120.0) / state.sheet_h as f32)
                            .clamp(2.0, 24.0);
                        let disp_w = state.sheet_w as f32 * scale;
                        let disp_h = state.sheet_h as f32 * scale;
                        let (resp, painter) =
                            ui.allocate_painter(egui::vec2(disp_w, disp_h), egui::Sense::click());
                        let rect = resp.rect;

                        if let Some(tex) = &state.texture {
                            painter.image(
                                tex.id(),
                                rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            painter.rect_filled(rect, 0.0, theme::BG_SUNKEN);
                        }

                        // Grid overlay from cols/rows preview
                        let cell_w = state.sheet_w as f32 / state.columns.max(1) as f32;
                        let cell_h = state.sheet_h as f32 / state.rows.max(1) as f32;
                        let stroke = egui::Stroke::new(1.0, Color32::from_rgb(220, 60, 60));
                        for c in 0..=state.columns {
                            let x = rect.min.x + c as f32 * cell_w * scale;
                            painter.line_segment(
                                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                                stroke,
                            );
                        }
                        for r in 0..=state.rows {
                            let y = rect.min.y + r as f32 * cell_h * scale;
                            painter.line_segment(
                                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                                stroke,
                            );
                        }

                        // Selection highlight
                        if let Some(idx) = state.selected {
                            if let Some(cell) = state.meta.sprites.get(idx) {
                                let [x, y, w, h] = cell.rect;
                                let sel = egui::Rect::from_min_size(
                                    egui::pos2(
                                        rect.min.x + x as f32 * scale,
                                        rect.min.y + y as f32 * scale,
                                    ),
                                    egui::vec2(w as f32 * scale, h as f32 * scale),
                                );
                                painter.rect_stroke(
                                    sel,
                                    0.0,
                                    egui::Stroke::new(2.0, theme::SELECT_STROKE),
                                );
                                // Pivot crosshair
                                let px = sel.min.x + cell.pivot[0] * sel.width();
                                let py = sel.min.y + cell.pivot[1] * sel.height();
                                let piv = egui::Stroke::new(1.5, Color32::from_rgb(80, 200, 255));
                                painter.line_segment(
                                    [egui::pos2(px - 6.0, py), egui::pos2(px + 6.0, py)],
                                    piv,
                                );
                                painter.line_segment(
                                    [egui::pos2(px, py - 6.0), egui::pos2(px, py + 6.0)],
                                    piv,
                                );
                            }
                        }

                        if resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                let lx = ((pos.x - rect.min.x) / scale) as u32;
                                let ly = ((pos.y - rect.min.y) / scale) as u32;
                                state.selected = state.meta.sprites.iter().position(|c| {
                                    let [x, y, w, h] = c.rect;
                                    lx >= x && lx < x + w && ly >= y && ly < y + h
                                });
                            }
                        }
                    });
                });

                ui.separator();

                // Side panel
                ui.vertical(|ui| {
                    ui.set_min_width(240.0);
                    ui.label(
                        RichText::new("Slice")
                            .strong()
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        ui.label(RichText::new("Type · Grid By Cell Count").size(12.0));
                        ui.horizontal(|ui| {
                            ui.label("C");
                            ui.add(egui::DragValue::new(&mut state.columns).range(1..=64));
                            ui.label("R");
                            ui.add(egui::DragValue::new(&mut state.rows).range(1..=64));
                        });
                        egui::ComboBox::from_label("Pivot")
                            .selected_text(if state.pivot_mode == 0 {
                                "Center"
                            } else {
                                "Custom"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut state.pivot_mode, 0, "Center");
                                ui.selectable_value(&mut state.pivot_mode, 1, "Custom");
                            });
                        if state.pivot_mode == 1 {
                            ui.horizontal(|ui| {
                                ui.label("X");
                                ui.add(
                                    egui::DragValue::new(&mut state.custom_pivot[0])
                                        .range(0.0..=1.0)
                                        .speed(0.01),
                                );
                                ui.label("Y");
                                ui.add(
                                    egui::DragValue::new(&mut state.custom_pivot[1])
                                        .range(0.0..=1.0)
                                        .speed(0.01),
                                );
                            });
                        }
                        if ui.button("Slice").clicked() {
                            match state.slice() {
                                Ok(warnings) => {
                                    refresh = true;
                                    if warnings.is_empty() {
                                        // status via refresh
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Sprite")
                            .strong()
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        if let Some(idx) = state.selected {
                            if let Some(cell) = state.meta.sprites.get_mut(idx) {
                                ui.label(format!("Name · {}", cell.name));
                                let [x, y, w, h] = cell.rect;
                                ui.label(
                                    RichText::new(format!("Pos · ({x}, {y})  Size · {w}×{h}"))
                                        .size(12.0)
                                        .color(theme::TEXT_MUTED),
                                );
                                ui.horizontal(|ui| {
                                    ui.label("Pivot X");
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut cell.pivot[0])
                                                .range(0.0..=1.0)
                                                .speed(0.01),
                                        )
                                        .changed()
                                    {
                                        state.dirty = true;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Pivot Y");
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut cell.pivot[1])
                                                .range(0.0..=1.0)
                                                .speed(0.01),
                                        )
                                        .changed()
                                    {
                                        state.dirty = true;
                                    }
                                });
                                if state.dirty && ui.button("Apply pivot").clicked() {
                                    if state.save_meta().is_ok() {
                                        refresh = true;
                                    }
                                }
                            }
                        } else {
                            theme::muted(ui, "Click a cell to edit pivot");
                        }
                    });

                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });

    if close {
        *open = false;
    }
    refresh
}
