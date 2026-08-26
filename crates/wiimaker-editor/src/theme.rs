//! Visual polish for the wiimaker editor shell.
//!
//! Teal-accent charcoal — distinct from default egui gray, no purple glow.

use eframe::egui::{
    self, style::ScrollStyle, Color32, Frame, Margin, RichText, Rounding, Shadow, Stroke, Vec2,
    Visuals,
};

/// Deep workspace behind the viewport well.
pub const BG_DEEP: Color32 = Color32::from_rgb(14, 17, 22);
/// Side / bottom panel fill.
pub const BG_PANEL: Color32 = Color32::from_rgb(24, 28, 36);
/// Slightly raised chrome (menu, cards).
pub const BG_RAISED: Color32 = Color32::from_rgb(32, 37, 48);
/// Inset wells (viewport backdrop).
pub const BG_SUNKEN: Color32 = Color32::from_rgb(10, 12, 16);
pub const BORDER: Color32 = Color32::from_rgb(48, 56, 70);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(38, 44, 56);

pub const TEXT: Color32 = Color32::from_rgb(220, 228, 238);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 158, 174);
pub const TEXT_DIM: Color32 = Color32::from_rgb(110, 120, 136);

/// Brand accent (seafoam — echoes orb green without competing).
pub const ACCENT: Color32 = Color32::from_rgb(61, 186, 156);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(36, 110, 96);

pub const SELECT_BG: Color32 = Color32::from_rgb(28, 72, 88);
pub const SELECT_STROKE: Color32 = Color32::from_rgb(126, 210, 230);

pub const DIRTY: Color32 = Color32::from_rgb(232, 168, 56);
pub const SAVED: Color32 = Color32::from_rgb(90, 168, 130);
pub const DANGER: Color32 = Color32::from_rgb(220, 96, 96);
pub const WARN_OUTLINE: Color32 = Color32::from_rgb(255, 200, 64);

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.menu_margin = Margin::same(6.0);
    style.spacing.window_margin = Margin::same(12.0);
    style.spacing.indent = 16.0;
    style.spacing.slider_width = 140.0;
    style.spacing.interact_size = Vec2::new(40.0, 22.0);
    style.spacing.scroll = ScrollStyle::solid();
    style.interaction.show_tooltips_only_when_still = false;
    ctx.set_style(style);
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.faint_bg_color = Color32::from_rgb(28, 32, 40);
    v.extreme_bg_color = BG_SUNKEN;
    v.code_bg_color = BG_RAISED;
    v.warn_fg_color = DIRTY;
    v.error_fg_color = DANGER;

    v.window_rounding = Rounding::same(8.0);
    v.window_fill = BG_RAISED;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_shadow = Shadow {
        offset: Vec2::new(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_black_alpha(120),
    };
    v.menu_rounding = Rounding::same(6.0);
    v.panel_fill = BG_PANEL;

    v.selection.bg_fill = SELECT_BG;
    v.selection.stroke = Stroke::new(1.0, SELECT_STROKE);

    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    v.widgets.noninteractive.rounding = Rounding::same(4.0);

    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.weak_bg_fill = BG_RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.rounding = Rounding::same(5.0);

    v.widgets.hovered.bg_fill = Color32::from_rgb(42, 50, 64);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 50, 64);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::from_rgb(235, 242, 250));
    v.widgets.hovered.rounding = Rounding::same(5.0);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    v.widgets.active.rounding = Rounding::same(5.0);

    v.widgets.open.bg_fill = BG_RAISED;
    v.widgets.open.weak_bg_fill = BG_RAISED;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);

    v.slider_trailing_fill = true;
    v.striped = true;
    v
}

pub fn menu_frame() -> Frame {
    Frame::none()
        .fill(BG_RAISED)
        .inner_margin(Margin::symmetric(12.0, 6.0))
        .stroke(Stroke::new(1.0, BORDER_SOFT))
}

pub fn side_frame() -> Frame {
    Frame::none()
        .fill(BG_PANEL)
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .stroke(Stroke::new(1.0, BORDER_SOFT))
}

pub fn bottom_frame() -> Frame {
    Frame::none()
        .fill(BG_PANEL)
        .inner_margin(Margin::symmetric(14.0, 10.0))
        .stroke(Stroke::new(1.0, BORDER_SOFT))
}

pub fn central_frame() -> Frame {
    Frame::none()
        .fill(BG_DEEP)
        .inner_margin(Margin::symmetric(12.0, 10.0))
}

pub fn card_frame() -> Frame {
    Frame::none()
        .fill(BG_RAISED)
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .rounding(Rounding::same(6.0))
        .stroke(Stroke::new(1.0, BORDER_SOFT))
}

pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).strong().size(13.0).color(TEXT));
    let y = ui.cursor().top() - 2.0;
    let rect = egui::Rect::from_min_max(
        egui::pos2(ui.max_rect().left(), y),
        egui::pos2(ui.max_rect().left() + 28.0, y + 2.0),
    );
    ui.painter()
        .rect_filled(rect, Rounding::same(1.0), ACCENT);
    ui.add_space(8.0);
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
