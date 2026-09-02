//! Unity-shaped egui_dock layout: Hierarchy | Scene/Game | Inspector, Project/Console below.

use eframe::egui::{self, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabStyle, TabViewer};

use crate::app::{CenterTab, EditorApp};
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EditorTab {
    Hierarchy,
    Scene,
    Game,
    Inspector,
    Project,
    Console,
}

impl EditorTab {
    fn title(self) -> &'static str {
        match self {
            Self::Hierarchy => "Hierarchy",
            Self::Scene => "Scene",
            Self::Game => "Game",
            Self::Inspector => "Inspector",
            Self::Project => "Project",
            Self::Console => "Console",
        }
    }
}

/// Default Unity layout:
/// left Hierarchy (~0.18), center Scene+Game tabs, right Inspector (~0.22 of remaining),
/// bottom Project+Console (~0.22 of remaining center).
pub(crate) fn default_unity_layout() -> DockState<EditorTab> {
    let mut dock = DockState::new(vec![EditorTab::Scene, EditorTab::Game]);
    let surface = dock.main_surface_mut();
    // split_left fraction = left (new) child share (Horizontal::fraction).
    let [center, _hier] =
        surface.split_left(NodeIndex::root(), 0.18, vec![EditorTab::Hierarchy]);
    // split_right fraction = left (old) child share → inspector ~0.22 of remaining.
    let [center, _insp] = surface.split_right(center, 0.78, vec![EditorTab::Inspector]);
    // split_below fraction = top (old) child share → bottom ~0.22 of remaining center.
    let [_center, _bottom] = surface.split_below(
        center,
        0.78,
        vec![EditorTab::Project, EditorTab::Console],
    );
    if let Some(loc) = dock.find_tab(&EditorTab::Scene) {
        dock.set_active_tab(loc);
        dock.set_focused_node_and_surface((loc.0, loc.1));
    }
    dock
}

fn dock_style(ui: &egui::Ui) -> Style {
    let mut style = Style::from_egui(ui.style().as_ref());
    style.dock_area_padding = None;
    style.main_surface_border_stroke = egui::Stroke::NONE;
    style.main_surface_border_rounding = egui::Rounding::ZERO;

    style.separator.width = 1.0;
    style.separator.extra_interact_width = 4.0;
    style.separator.color_idle = theme::BORDER;
    style.separator.color_hovered = theme::ACCENT_DIM;
    style.separator.color_dragged = theme::ACCENT;

    style.tab_bar.bg_fill = theme::BG_TAB;
    style.tab_bar.height = 24.0;
    style.tab_bar.hline_color = theme::BORDER;
    style.tab_bar.rounding = egui::Rounding::ZERO;

    style.tab.hline_below_active_tab_name = true;
    let paint_tab = |bg: egui::Color32, text: egui::Color32| egui_dock::TabInteractionStyle {
        bg_fill: bg,
        text_color: text,
        outline_color: theme::BORDER,
        rounding: egui::Rounding::ZERO,
    };
    style.tab.active = paint_tab(theme::BG_PANEL, theme::TEXT);
    style.tab.focused = paint_tab(theme::BG_PANEL, theme::TEXT);
    style.tab.hovered = paint_tab(egui::Color32::from_rgb(74, 74, 74), theme::TEXT);
    style.tab.inactive = paint_tab(theme::BG_TAB, theme::TEXT_MUTED);
    style.tab.active_with_kb_focus = style.tab.active.clone();
    style.tab.focused_with_kb_focus = style.tab.focused.clone();
    style.tab.inactive_with_kb_focus = style.tab.inactive.clone();
    style.tab.tab_body.bg_fill = theme::BG_PANEL;
    style.tab.tab_body.inner_margin = egui::Margin::symmetric(8.0, 6.0);
    style.tab.tab_body.stroke = egui::Stroke::new(1.0_f32, theme::BORDER);
    style.tab.tab_body.rounding = egui::Rounding::ZERO;
    style
}

struct EditorTabViewer<'a> {
    app: &'a mut EditorApp,
}

impl TabViewer for EditorTabViewer<'_> {
    type Tab = EditorTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title().into()
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match *tab {
            EditorTab::Hierarchy => self.app.ui_hierarchy(ui),
            EditorTab::Inspector => self.app.ui_inspector(ui),
            EditorTab::Scene => self.app.ui_viewport(ui, CenterTab::Scene),
            EditorTab::Game => self.app.ui_viewport(ui, CenterTab::Game),
            EditorTab::Project => self.app.ui_project_body(ui),
            EditorTab::Console => self.app.ui_console(ui),
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }

    fn tab_style_override(&self, tab: &Self::Tab, global: &TabStyle) -> Option<TabStyle> {
        match tab {
            EditorTab::Scene | EditorTab::Game => {
                let mut s = global.clone();
                s.tab_body.inner_margin = egui::Margin::same(0.0);
                s.tab_body.bg_fill = theme::BG_DEEP;
                Some(s)
            }
            _ => None,
        }
    }
}

impl EditorApp {
    pub(crate) fn focus_tab(&mut self, tab: EditorTab) {
        self.pending_focus = Some(tab);
    }

    pub(crate) fn apply_pending_focus(&mut self) {
        let Some(tab) = self.pending_focus.take() else {
            return;
        };
        if let Some(loc) = self.dock_state.find_tab(&tab) {
            self.dock_state.set_active_tab(loc);
            self.dock_state.set_focused_node_and_surface((loc.0, loc.1));
        }
    }

    pub(crate) fn ui_dock(&mut self, ui: &mut egui::Ui) {
        let mut dock_state =
            std::mem::replace(&mut self.dock_state, DockState::new(vec![EditorTab::Scene]));
        let style = dock_style(ui);
        {
            let mut viewer = EditorTabViewer { app: self };
            DockArea::new(&mut dock_state)
                .style(style)
                .show_close_buttons(false)
                .show_window_close_buttons(false)
                .draggable_tabs(true)
                .show_inside(ui, &mut viewer);
        }
        self.dock_state = dock_state;
    }
}
