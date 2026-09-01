//! Dark Unity-Pro-adjacent chrome for the wiimaker editor.
//!
//! Layout mimics Unity 6 docks; colors stay dark (never the light Personal skin).
//! Geometric stand-ins only — no trademarked Unity icons.

use eframe::egui::{
    self, style::ScrollStyle, Align2, Color32, FontId, Frame, Margin, RichText, Rounding, Sense,
    Shadow, Shape, Stroke, Vec2, Visuals,
};

/// Workspace behind the Scene/Game well (Unity Pro ~#191919).
pub const BG_DEEP: Color32 = Color32::from_rgb(25, 25, 25);
/// Dock panel fill (~#383838).
pub const BG_PANEL: Color32 = Color32::from_rgb(56, 56, 56);
/// Toolbar / raised chrome (~#3c3c3c).
pub const BG_RAISED: Color32 = Color32::from_rgb(60, 60, 60);
/// Inset wells / Game view backdrop.
pub const BG_SUNKEN: Color32 = Color32::from_rgb(32, 32, 32);
/// Unselected tab strip.
pub const BG_TAB: Color32 = Color32::from_rgb(42, 42, 42);
/// Component card header strip.
pub const BG_COMP: Color32 = Color32::from_rgb(66, 66, 66);

pub const BORDER: Color32 = Color32::from_rgb(28, 28, 28);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(74, 74, 74);

pub const TEXT: Color32 = Color32::from_rgb(210, 210, 210);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(180, 180, 180);
pub const TEXT_DIM: Color32 = Color32::from_rgb(138, 138, 138);

/// Selection / play accent — Unity-ish blue, not the old seafoam.
pub const ACCENT: Color32 = Color32::from_rgb(76, 156, 222);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(44, 93, 135);

pub const SELECT_BG: Color32 = Color32::from_rgb(44, 93, 135);
pub const SELECT_STROKE: Color32 = Color32::from_rgb(109, 180, 232);

pub const DIRTY: Color32 = Color32::from_rgb(232, 168, 56);
pub const SAVED: Color32 = Color32::from_rgb(90, 168, 130);
pub const DANGER: Color32 = Color32::from_rgb(220, 96, 96);
pub const WARN_OUTLINE: Color32 = Color32::from_rgb(255, 200, 64);

/// Unity inspector axis tints (not brand marks).
pub const AXIS_X: Color32 = Color32::from_rgb(219, 62, 62);
pub const AXIS_Y: Color32 = Color32::from_rgb(90, 168, 62);
pub const AXIS_Z: Color32 = Color32::from_rgb(86, 156, 214);

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.menu_margin = Margin::same(4.0);
    style.spacing.window_margin = Margin::same(8.0);
    style.spacing.indent = 14.0;
    style.spacing.slider_width = 140.0;
    style.spacing.interact_size = Vec2::new(36.0, 20.0);
    style.spacing.scroll = ScrollStyle::solid();
    style.interaction.show_tooltips_only_when_still = false;
    ctx.set_style(style);
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.faint_bg_color = Color32::from_rgb(48, 48, 48);
    v.extreme_bg_color = BG_SUNKEN;
    v.code_bg_color = BG_RAISED;
    v.warn_fg_color = DIRTY;
    v.error_fg_color = DANGER;

    v.window_rounding = Rounding::same(2.0);
    v.window_fill = BG_RAISED;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_shadow = Shadow {
        offset: Vec2::new(0.0, 6.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(140),
    };
    v.menu_rounding = Rounding::same(2.0);
    v.panel_fill = BG_PANEL;

    v.selection.bg_fill = SELECT_BG;
    v.selection.stroke = Stroke::new(1.0, SELECT_STROKE);

    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    v.widgets.noninteractive.rounding = Rounding::same(2.0);

    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.weak_bg_fill = BG_RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.rounding = Rounding::same(2.0);

    v.widgets.hovered.bg_fill = Color32::from_rgb(74, 74, 74);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(74, 74, 74);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_rgb(235, 235, 235));
    v.widgets.hovered.rounding = Rounding::same(2.0);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.rounding = Rounding::same(2.0);

    v.widgets.open.bg_fill = BG_RAISED;
    v.widgets.open.weak_bg_fill = BG_RAISED;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);

    v.slider_trailing_fill = true;
    v.striped = false;
    v
}

pub fn menu_frame() -> Frame {
    Frame::none()
        .fill(BG_RAISED)
        .inner_margin(Margin::symmetric(8.0, 3.0))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn toolbar_frame() -> Frame {
    Frame::none()
        .fill(BG_PANEL)
        .inner_margin(Margin::symmetric(8.0, 4.0))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn side_frame() -> Frame {
    Frame::none()
        .fill(BG_PANEL)
        .inner_margin(Margin::symmetric(8.0, 6.0))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn bottom_frame() -> Frame {
    Frame::none()
        .fill(BG_PANEL)
        .inner_margin(Margin::symmetric(8.0, 6.0))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn central_frame() -> Frame {
    Frame::none()
        .fill(BG_DEEP)
        .inner_margin(Margin::symmetric(6.0, 4.0))
}

pub fn card_frame() -> Frame {
    Frame::none()
        .fill(BG_RAISED)
        .inner_margin(Margin::symmetric(8.0, 6.0))
        .rounding(Rounding::same(2.0))
        .stroke(Stroke::new(1.0_f32, BORDER))
}

/// Inspector property block: no extra card chrome (Unity draws fields on the panel).
pub fn inspector_props() -> Frame {
    Frame::none().inner_margin(Margin::symmetric(8.0, 4.0))
}

pub fn inspector_label(ui: &mut egui::Ui, text: &str) {
    ui.add_sized(
        [64.0, 18.0],
        egui::Label::new(RichText::new(text).size(12.0).color(TEXT_MUTED)).selectable(false),
    );
}

fn axis_color(axis: char) -> Color32 {
    match axis {
        'X' | 'x' => AXIS_X,
        'Y' | 'y' => AXIS_Y,
        _ => AXIS_Z,
    }
}

/// Colored axis chip + drag. Call *inside* a `horizontal` — do not nest another row
/// (nested `horizontal` expands to full width and stacks X/Y, unlike Unity).
pub fn axis_drag(ui: &mut egui::Ui, axis: char, v: &mut f32, speed: f32) -> bool {
    ui.spacing_mut().item_spacing.x = 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 18.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(2.0), axis_color(axis));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        axis.to_string(),
        egui::FontId::proportional(10.0),
        Color32::WHITE,
    );
    ui.add_sized(
        [40.0, 18.0],
        egui::DragValue::new(v).speed(speed).max_decimals(2),
    )
    .changed()
}

/// One Unity Transform line: label + X + Y on the same row.
pub fn vec2_row(ui: &mut egui::Ui, label: &str, xy: &mut [f32], speed: f32) -> bool {
    let mut changed = false;
    if xy.len() < 2 {
        return false;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        inspector_label(ui, label);
        changed |= axis_drag(ui, 'X', &mut xy[0], speed);
        changed |= axis_drag(ui, 'Y', &mut xy[1], speed);
    });
    changed
}

/// One Unity Transform line: label + X + Y + Z. `xyz` must be at least 3 long.
pub fn vec3_row(ui: &mut egui::Ui, label: &str, xyz: &mut [f32], speed: f32) -> bool {
    let mut changed = false;
    if xyz.len() < 3 {
        return false;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        inspector_label(ui, label);
        changed |= axis_drag(ui, 'X', &mut xyz[0], speed);
        changed |= axis_drag(ui, 'Y', &mut xyz[1], speed);
        changed |= axis_drag(ui, 'Z', &mut xyz[2], speed);
    });
    changed
}

/// Label + one DragValue.
pub fn labeled_drag(ui: &mut egui::Ui, label: &str, v: &mut f32, speed: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        inspector_label(ui, label);
        changed = ui
            .add(egui::DragValue::new(v).speed(speed).max_decimals(2))
            .changed();
    });
    changed
}

pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).strong().size(13.0).color(TEXT));
    ui.add_space(4.0);
}

pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(12.0).color(TEXT_MUTED));
}

pub fn meta_chip(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(label).size(11.0).color(TEXT_DIM));
        ui.label(RichText::new(value).size(11.0).strong().color(TEXT));
    });
}

/// Unity-style dock tab strip. Selected tab gets a blue underline.
pub fn dock_tabs<T: Copy + PartialEq>(ui: &mut egui::Ui, tabs: &[(&str, T)], current: T) -> T {
    let mut selected = current;
    let tab_h = 22.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.set_min_height(tab_h);
        for (label, value) in tabs {
            let on = selected == *value;
            let fill = if on { BG_PANEL } else { BG_TAB };
            let color = if on { TEXT } else { TEXT_MUTED };
            let btn = egui::Button::new(RichText::new(*label).size(12.5).color(color))
                .fill(fill)
                .rounding(Rounding::ZERO)
                .stroke(Stroke::NONE);
            let resp = ui.add(btn);
            if on {
                let r = resp.rect;
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(r.left(), r.bottom() - 2.0),
                        egui::pos2(r.right(), r.bottom()),
                    ),
                    Rounding::ZERO,
                    ACCENT,
                );
            }
            if resp.clicked() {
                selected = *value;
            }
        }
        let rest = ui.available_width();
        if rest > 0.0 {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(rest, tab_h), Sense::hover());
            ui.painter().rect_filled(rect, Rounding::ZERO, BG_TAB);
        }
    });
    let y = ui.cursor().top() - 1.0;
    ui.painter().hline(
        ui.max_rect().x_range(),
        y,
        Stroke::new(1.0, BORDER),
    );
    ui.add_space(4.0);
    selected
}

pub struct CardHeaderOut {
    pub open: bool,
    pub toggle: Option<bool>,
    pub remove: bool,
}

/// Unity Inspector component header: foldout, enable checkbox, bold title, gear/Remove.
pub fn component_card_header(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    enabled: Option<bool>,
    removable: bool,
) -> CardHeaderOut {
    let persist = ui.make_persistent_id(("comp_open", std::any::TypeId::of::<()>(), id));
    let mut open = ui.ctx().data_mut(|d| *d.get_temp_mut_or(persist, true));
    let mut toggle = None;
    let mut remove = false;

    let header = Frame::none()
        .fill(BG_COMP)
        .inner_margin(Margin::symmetric(6.0, 3.0))
        .rounding(Rounding::ZERO);
    header.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let arrow = if open { "▾" } else { "▸" };
            if ui
                .add(
                    egui::Button::new(RichText::new(arrow).size(12.0).color(TEXT_MUTED))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                )
                .clicked()
            {
                open = !open;
            }
            if let Some(en) = enabled {
                let mut en = en;
                if ui.checkbox(&mut en, "").changed() {
                    toggle = Some(en);
                }
            }
            ui.label(RichText::new(title).strong().size(13.0).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if removable {
                    ui.menu_button(RichText::new("⋮").size(14.0).color(TEXT_MUTED), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Remove Component").color(DANGER),
                                )
                                .fill(BG_SUNKEN),
                            )
                            .clicked()
                        {
                            remove = true;
                            ui.close_menu();
                        }
                    });
                }
            });
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(persist, open));
    CardHeaderOut {
        open,
        toggle,
        remove,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayBtn {
    Play,
    Pause,
    Stop,
}

/// Geometric Play / Pause / Stop — no Unity glyphs.
pub fn play_control(ui: &mut egui::Ui, kind: PlayBtn, active: bool, enabled: bool) -> egui::Response {
    let size = egui::vec2(30.0, 22.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, mut resp) = ui.allocate_exact_size(size, sense);
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let bg = if !enabled {
        BG_SUNKEN
    } else if active {
        ACCENT_DIM
    } else if resp.hovered() {
        Color32::from_rgb(80, 80, 80)
    } else {
        BG_RAISED
    };
    ui.painter()
        .rect_filled(rect, Rounding::same(3.0), bg);
    if active && enabled {
        ui.painter()
            .rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0, ACCENT));
    } else {
        ui.painter()
            .rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0, BORDER_SOFT));
    }
    let c = rect.center();
    let color = if enabled { TEXT } else { TEXT_DIM };
    match kind {
        PlayBtn::Play => {
            let pts = vec![
                egui::pos2(c.x - 5.0, c.y - 7.0),
                egui::pos2(c.x - 5.0, c.y + 7.0),
                egui::pos2(c.x + 8.0, c.y),
            ];
            ui.painter()
                .add(Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        PlayBtn::Pause => {
            ui.painter().rect_filled(
                egui::Rect::from_center_size(egui::pos2(c.x - 4.5, c.y), egui::vec2(4.0, 12.0)),
                Rounding::same(1.0),
                color,
            );
            ui.painter().rect_filled(
                egui::Rect::from_center_size(egui::pos2(c.x + 4.5, c.y), egui::vec2(4.0, 12.0)),
                Rounding::same(1.0),
                color,
            );
        }
        PlayBtn::Stop => {
            ui.painter().rect_filled(
                egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                Rounding::same(1.0),
                color,
            );
        }
    }
    resp
}
