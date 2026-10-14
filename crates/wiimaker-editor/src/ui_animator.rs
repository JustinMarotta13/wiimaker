//! Animator window: layer / parameter list and a state graph (dark Pro).
//!
//! Edits go through `wiimaker-assets` controller helpers, the same ones the CLI calls.

use eframe::egui::{
    self, pos2, Align2, Color32, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use wiimaker_assets::{
    add_controller_layer, add_controller_param, add_layer_transition, arrow_head, hit_node,
    hit_transition, is_any_state, is_base_layer_name, layout_layer, move_layer_node,
    remove_controller_layer, remove_controller_param, remove_layer_state, remove_layer_transition,
    rename_controller_layer, rename_controller_param, rename_layer_state,
    set_controller_layer_weight, set_controller_param_default, set_layer_default,
    set_layer_transition, unique_label, write_layer_state, write_layer_state_blend_tree,
    write_layer_state_clip, AnimatorControllerMeta, BlendDimension, BlendTreeMeta,
    ControllerCondition, ControllerParam, ControllerParamType, ControllerTransition, GraphNode,
    GraphNodeId, BASE_LAYER_NAME, TRANSITION_SLOP,
};

use crate::app::{EditorApp, PlayMode};
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnimatorSide {
    Layers,
    Parameters,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnimSel {
    Node(String),
    Transition(usize),
}

#[derive(Clone, Debug)]
struct NodeDrag {
    key: String,
    origin: [f32; 2],
    grab: [f32; 2],
}

#[derive(Clone, Debug)]
struct MenuHit {
    graph: [f32; 2],
    node: Option<String>,
    transition: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct AnimatorView {
    pub pan: [f32; 2],
    pub zoom: f32,
    pub layer: String,
    side: AnimatorSide,
    sel: Option<AnimSel>,
    link_from: Option<String>,
    drag: Option<NodeDrag>,
    menu: Option<MenuHit>,
    rename_layer: String,
    rename_param: String,
    param_draft: String,
    rename_state: String,
    synced_layer: String,
}

impl Default for AnimatorView {
    fn default() -> Self {
        Self {
            pan: [8.0, 4.0],
            zoom: 1.0,
            layer: BASE_LAYER_NAME.into(),
            side: AnimatorSide::Layers,
            sel: None,
            link_from: None,
            drag: None,
            menu: None,
            rename_layer: BASE_LAYER_NAME.into(),
            rename_param: String::new(),
            param_draft: String::new(),
            rename_state: String::new(),
            synced_layer: String::new(),
        }
    }
}

enum AnimEdit {
    AddLayer(String),
    RemoveLayer(String),
    RenameLayer {
        from: String,
        to: String,
    },
    Weight {
        name: String,
        weight: f32,
    },
    AddParam(ControllerParamType),
    RenameParam {
        from: String,
        to: String,
    },
    RemoveParam(String),
    ParamDefault {
        name: String,
        value: serde_json::Value,
    },
    AddState {
        name: String,
        pos: [f32; 2],
        blend: bool,
    },
    RenameState {
        from: String,
        to: String,
    },
    RemoveState(String),
    SetDefault(String),
    AddTransition {
        from: String,
        to: String,
    },
    RemoveTransition(usize),
    Move {
        node: String,
        pos: [f32; 2],
    },
    SetClip {
        state: String,
        clip: String,
    },
    SetExit {
        index: usize,
        value: bool,
    },
    SetConditions {
        index: usize,
        conditions: Vec<ControllerCondition>,
    },
}

fn screen_of(origin: Pos2, pan: [f32; 2], zoom: f32, g: [f32; 2]) -> Pos2 {
    pos2(
        origin.x + pan[0] + g[0] * zoom,
        origin.y + pan[1] + g[1] * zoom,
    )
}

fn graph_of(origin: Pos2, pan: [f32; 2], zoom: f32, s: Pos2) -> [f32; 2] {
    let z = zoom.max(0.05);
    [(s.x - origin.x - pan[0]) / z, (s.y - origin.y - pan[1]) / z]
}

impl EditorApp {
    fn animator_stem(&self) -> Option<String> {
        if let Some(rel) = &self.selected_file {
            let name = rel.to_string_lossy().replace('\\', "/");
            if let Some(stem) = name.strip_suffix(".controller.json") {
                return Some(stem.rsplit('/').next().unwrap_or(stem).to_string());
            }
        }
        let entity = self.primary_selected()?;
        let ctrl = self
            .scene
            .find_entity(entity)?
            .components
            .animator
            .as_ref()?
            .controller
            .clone();
        if ctrl.is_empty() {
            None
        } else {
            Some(ctrl)
        }
    }

    fn animator_live(&self, controller: &str, layer: &str) -> Option<(String, f32)> {
        if self.play_mode == PlayMode::Edit {
            return None;
        }
        let entity = self.primary_selected()?.to_string();
        let authored = self
            .scene
            .find_entity(&entity)?
            .components
            .animator
            .as_ref()?
            .controller
            .clone();
        if authored != controller {
            return None;
        }
        let session_world = self.play_session.as_ref().and_then(|s| s.world());
        let world = session_world.unwrap_or(&self.world);
        let an = world
            .find_by_name(&entity)
            .and_then(|id| world.animator(id))?;
        let idx = an.layer_index(layer)?;
        let state = an.layer_state_name(idx)?.to_string();
        Some((state, an.layer_normalized_time(idx)))
    }

    pub(crate) fn ui_animator(&mut self, ui: &mut egui::Ui) {
        let Some(stem) = self.animator_stem() else {
            theme::muted(
                ui,
                "Select an Animator controller in Project, or an entity with an Animator.",
            );
            return;
        };
        let Some(meta) = self.controller_catalog.lookup(&stem).cloned() else {
            theme::muted(ui, &format!("missing assets/{stem}.controller.json"));
            return;
        };
        if self.animator_view.layer != BASE_LAYER_NAME
            && meta.layer(&self.animator_view.layer).is_none()
        {
            self.animator_view.layer = BASE_LAYER_NAME.into();
        }
        if self.animator_view.synced_layer != self.animator_view.layer {
            self.animator_view.synced_layer = self.animator_view.layer.clone();
            self.animator_view.rename_layer = self.animator_view.layer.clone();
        }
        let mut edits = Vec::new();
        let height = ui.available_height().max(40.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.allocate_ui_with_layout(
                Vec2::new(232.0, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.ui_animator_side(ui, &stem, &meta, &mut edits),
            );
            ui.separator();
            let w = ui.available_width().max(40.0);
            ui.allocate_ui_with_layout(
                Vec2::new(w, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.ui_animator_graph(ui, &stem, &meta, &mut edits),
            );
        });
        self.apply_animator_edits(&stem, edits);
    }

    fn ui_animator_side(
        &mut self,
        ui: &mut egui::Ui,
        stem: &str,
        meta: &AnimatorControllerMeta,
        edits: &mut Vec<AnimEdit>,
    ) {
        let _ = stem;
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.animator_view.side == AnimatorSide::Layers, "Layers")
                .clicked()
            {
                self.animator_view.side = AnimatorSide::Layers;
            }
            if ui
                .selectable_label(
                    self.animator_view.side == AnimatorSide::Parameters,
                    "Parameters",
                )
                .clicked()
            {
                self.animator_view.side = AnimatorSide::Parameters;
            }
        });
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("animator_side")
            .show(ui, |ui| match self.animator_view.side {
                AnimatorSide::Layers => self.ui_animator_layers(ui, meta, edits),
                AnimatorSide::Parameters => self.ui_animator_params(ui, meta, edits),
            });
    }

    fn ui_animator_layers(
        &mut self,
        ui: &mut egui::Ui,
        meta: &AnimatorControllerMeta,
        edits: &mut Vec<AnimEdit>,
    ) {
        let mut rows = vec![(BASE_LAYER_NAME.to_string(), meta.weight, true)];
        for layer in &meta.layers {
            rows.push((layer.name.clone(), layer.weight, false));
        }
        for (name, weight, base) in rows {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.animator_view.layer == name, &name)
                    .clicked()
                {
                    self.animator_view.layer = name.clone();
                    self.animator_view.sel = None;
                    self.animator_view.link_from = None;
                }
                let mut w = weight;
                let drag = egui::DragValue::new(&mut w)
                    .speed(0.01)
                    .max_decimals(2)
                    .prefix("w ");
                if base {
                    ui.add_enabled(false, drag)
                        .on_hover_text("Base weight does not affect playback");
                } else if ui.add(drag).changed() && w.is_finite() {
                    edits.push(AnimEdit::Weight {
                        name: name.clone(),
                        weight: w,
                    });
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button("+").on_hover_text("Add Layer").clicked() {
                let mut names: Vec<String> = meta.layers.iter().map(|l| l.name.clone()).collect();
                names.push(BASE_LAYER_NAME.into());
                edits.push(AnimEdit::AddLayer(unique_label(&names, "New Layer")));
            }
            let selected = self.animator_view.layer.clone();
            if ui
                .add_enabled(!is_base_layer_name(&selected), egui::Button::new("Remove"))
                .clicked()
            {
                edits.push(AnimEdit::RemoveLayer(selected));
            }
        });
        if !is_base_layer_name(&self.animator_view.layer) {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.animator_view.rename_layer)
                        .desired_width(120.0)
                        .hint_text("Rename"),
                );
                let to = self.animator_view.rename_layer.trim().to_string();
                let from = self.animator_view.layer.clone();
                if ui
                    .add_enabled(!to.is_empty() && to != from, egui::Button::new("Rename"))
                    .clicked()
                {
                    edits.push(AnimEdit::RenameLayer { from, to });
                }
            });
        } else {
            theme::muted(ui, "Base weight is ignored at playback.");
        }
        self.ui_animator_details(ui, meta, edits);
    }

    fn ui_animator_params(
        &mut self,
        ui: &mut egui::Ui,
        meta: &AnimatorControllerMeta,
        edits: &mut Vec<AnimEdit>,
    ) {
        let params = meta.parameters.clone();
        for p in &params {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.animator_view.rename_param == p.name, &p.name)
                    .clicked()
                {
                    self.animator_view.rename_param = p.name.clone();
                    self.animator_view.param_draft = p.name.clone();
                }
                ui.label(
                    RichText::new(match p.kind {
                        ControllerParamType::Bool => "Bool",
                        ControllerParamType::Float => "Float",
                        ControllerParamType::Trigger => "Trigger",
                    })
                    .size(11.0)
                    .color(theme::TEXT_DIM),
                );
                match p.kind {
                    ControllerParamType::Bool | ControllerParamType::Trigger => {
                        let mut v = p.default_bool();
                        if ui.checkbox(&mut v, "").changed() {
                            edits.push(AnimEdit::ParamDefault {
                                name: p.name.clone(),
                                value: serde_json::json!(v),
                            });
                        }
                    }
                    ControllerParamType::Float => {
                        let mut v = p.default_float();
                        if ui
                            .add(egui::DragValue::new(&mut v).speed(0.05).max_decimals(3))
                            .changed()
                            && v.is_finite()
                        {
                            edits.push(AnimEdit::ParamDefault {
                                name: p.name.clone(),
                                value: serde_json::json!(v as f64),
                            });
                        }
                    }
                }
                if ui.small_button("x").on_hover_text("Remove").clicked() {
                    edits.push(AnimEdit::RemoveParam(p.name.clone()));
                }
            });
        }
        ui.horizontal(|ui| {
            ui.menu_button("+", |ui| {
                if ui.button("Bool").clicked() {
                    edits.push(AnimEdit::AddParam(ControllerParamType::Bool));
                    ui.close_menu();
                }
                if ui.button("Float").clicked() {
                    edits.push(AnimEdit::AddParam(ControllerParamType::Float));
                    ui.close_menu();
                }
                if ui.button("Trigger").clicked() {
                    edits.push(AnimEdit::AddParam(ControllerParamType::Trigger));
                    ui.close_menu();
                }
            });
            let from = self.animator_view.rename_param.clone();
            if !from.is_empty() && params.iter().any(|p| p.name == from) {
                ui.add(
                    egui::TextEdit::singleline(&mut self.animator_view.param_draft)
                        .desired_width(90.0)
                        .hint_text("Rename"),
                );
                let to = self.animator_view.param_draft.trim().to_string();
                if ui.button("Rename").clicked() && !to.is_empty() && to != from {
                    edits.push(AnimEdit::RenameParam { from, to });
                }
            }
        });
        theme::muted(ui, "Parameters are shared by every layer.");
        self.ui_animator_details(ui, meta, edits);
    }

    fn ui_animator_details(
        &mut self,
        ui: &mut egui::Ui,
        meta: &AnimatorControllerMeta,
        edits: &mut Vec<AnimEdit>,
    ) {
        let Some(sel) = self.animator_view.sel.clone() else {
            return;
        };
        let layer = self.animator_view.layer.clone();
        let (states, transitions) = machine_of(meta, &layer);
        ui.add_space(8.0);
        ui.separator();
        match sel {
            AnimSel::Node(key) => {
                if matches!(key.as_str(), "Entry" | "Any" | "Exit") {
                    ui.label(RichText::new(&key).strong().color(theme::TEXT));
                    return;
                }
                let Some(state) = states.iter().find(|s| s.name == key) else {
                    return;
                };
                ui.label(RichText::new(&state.name).strong().color(theme::TEXT));
                ui.horizontal(|ui| {
                    let mut draft = if self.animator_view.side == AnimatorSide::Layers {
                        if self.animator_view.rename_state.is_empty() {
                            state.name.clone()
                        } else {
                            self.animator_view.rename_state.clone()
                        }
                    } else {
                        state.name.clone()
                    };
                    if self.animator_view.side == AnimatorSide::Layers {
                        ui.add(egui::TextEdit::singleline(&mut draft).desired_width(110.0));
                        if ui.button("Rename").clicked() {
                            let to = draft.trim().to_string();
                            if !to.is_empty() && to != state.name {
                                edits.push(AnimEdit::RenameState {
                                    from: state.name.clone(),
                                    to,
                                });
                            }
                        }
                        self.animator_view.rename_state = draft;
                    }
                });
                if let Some(tree) = &state.blend_tree {
                    theme::muted(
                        ui,
                        &format!(
                            "Blend {} · {}",
                            tree.dimension.as_str(),
                            tree.params.join(", ")
                        ),
                    );
                    for m in &tree.motions {
                        ui.label(
                            RichText::new(format!("· {}", m.clip))
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                    }
                } else {
                    let mut clip = state.clip.clone();
                    let clips: Vec<String> = self.anim_catalog.names().to_vec();
                    egui::ComboBox::from_id_salt(("anim_clip", &state.name))
                        .selected_text(if clip.is_empty() {
                            "(empty)".to_string()
                        } else {
                            clip.clone()
                        })
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(clip.is_empty(), "(empty)").clicked() {
                                clip.clear();
                            }
                            for name in &clips {
                                if ui.selectable_label(clip == *name, name).clicked() {
                                    clip = name.clone();
                                }
                            }
                        });
                    if clip != state.clip {
                        edits.push(AnimEdit::SetClip {
                            state: state.name.clone(),
                            clip,
                        });
                    }
                }
            }
            AnimSel::Transition(index) => {
                let Some(t) = transitions.get(index) else {
                    return;
                };
                ui.label(
                    RichText::new(format!("{}  →  {}", t.from, t.to))
                        .strong()
                        .color(theme::TEXT),
                );
                let mut exit = t.has_exit_time;
                if ui.checkbox(&mut exit, "Has Exit Time").changed() {
                    edits.push(AnimEdit::SetExit { index, value: exit });
                }
                let mut conditions = t.conditions.clone();
                let mut changed = false;
                let param_names: Vec<String> =
                    meta.parameters.iter().map(|p| p.name.clone()).collect();
                let mut remove = None;
                for (i, c) in conditions.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let mut param = c.param.clone();
                        egui::ComboBox::from_id_salt(("cond_param", index, i))
                            .selected_text(if param.is_empty() {
                                "(param)".to_string()
                            } else {
                                param.clone()
                            })
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for name in &param_names {
                                    if ui.selectable_label(param == *name, name).clicked() {
                                        param = name.clone();
                                    }
                                }
                            });
                        if param != c.param {
                            c.param = param;
                            changed = true;
                        }
                        let kind = meta
                            .parameters
                            .iter()
                            .find(|p| p.name == c.param)
                            .map(|p| p.kind);
                        match kind {
                            Some(ControllerParamType::Bool)
                            | Some(ControllerParamType::Trigger)
                            | None => {
                                let mut v =
                                    c.equals.as_ref().and_then(|v| v.as_bool()).unwrap_or(true);
                                let label = if matches!(kind, Some(ControllerParamType::Trigger)) {
                                    "Trigger"
                                } else {
                                    ""
                                };
                                if ui.checkbox(&mut v, label).changed() {
                                    c.equals = Some(serde_json::json!(v));
                                    c.greater = None;
                                    c.less = None;
                                    changed = true;
                                }
                            }
                            Some(ControllerParamType::Float) => {
                                let mut mode = if c.greater.is_some() {
                                    1
                                } else if c.less.is_some() {
                                    2
                                } else {
                                    0
                                };
                                let mut value = c
                                    .greater
                                    .or(c.less)
                                    .or_else(|| {
                                        c.equals.as_ref().and_then(|v| v.as_f64()).map(|n| n as f32)
                                    })
                                    .unwrap_or(0.0);
                                egui::ComboBox::from_id_salt(("cond_mode", index, i))
                                    .selected_text(match mode {
                                        1 => "Greater",
                                        2 => "Less",
                                        _ => "Equals",
                                    })
                                    .width(72.0)
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(mode == 0, "Equals").clicked() {
                                            mode = 0;
                                        }
                                        if ui.selectable_label(mode == 1, "Greater").clicked() {
                                            mode = 1;
                                        }
                                        if ui.selectable_label(mode == 2, "Less").clicked() {
                                            mode = 2;
                                        }
                                    });
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut value)
                                            .speed(0.05)
                                            .max_decimals(3),
                                    )
                                    .changed()
                                    || mode
                                        != if c.greater.is_some() {
                                            1
                                        } else if c.less.is_some() {
                                            2
                                        } else {
                                            0
                                        }
                                {
                                    c.equals = None;
                                    c.greater = None;
                                    c.less = None;
                                    match mode {
                                        1 => c.greater = Some(value),
                                        2 => c.less = Some(value),
                                        _ => c.equals = Some(serde_json::json!(value as f64)),
                                    }
                                    changed = true;
                                }
                            }
                        }
                        if ui.small_button("x").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    conditions.remove(i);
                    changed = true;
                }
                if ui.button("Add Condition").clicked() {
                    let param = param_names.first().cloned().unwrap_or_default();
                    let kind = meta
                        .parameters
                        .iter()
                        .find(|p| p.name == param)
                        .map(|p| p.kind);
                    let mut c = ControllerCondition {
                        param,
                        ..Default::default()
                    };
                    match kind {
                        Some(ControllerParamType::Float) => {
                            c.equals = Some(serde_json::json!(0.0));
                        }
                        _ => c.equals = Some(serde_json::json!(true)),
                    }
                    conditions.push(c);
                    changed = true;
                }
                if changed {
                    edits.push(AnimEdit::SetConditions { index, conditions });
                }
            }
        }
    }

    fn ui_animator_graph(
        &mut self,
        ui: &mut egui::Ui,
        stem: &str,
        meta: &AnimatorControllerMeta,
        edits: &mut Vec<AnimEdit>,
    ) {
        let layer = self.animator_view.layer.clone();
        let (default_state, states, transitions, graph) = machine_parts(meta, &layer);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{stem}   /   {layer}"))
                    .size(13.0)
                    .color(theme::TEXT),
            );
            if let Some(from) = &self.animator_view.link_from {
                ui.label(
                    RichText::new(format!("Make Transition from {from}"))
                        .size(12.0)
                        .color(theme::ACCENT),
                );
            }
        });
        let live = self.animator_live(stem, &layer);
        let height = (ui.available_height() - 4.0).max(32.0);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            Sense::click_and_drag(),
        );
        let origin = rect.min;
        let mut pan = self.animator_view.pan;
        let mut zoom = self.animator_view.zoom;
        let mut nodes = layout_layer(&default_state, &states, &graph);
        if let Some(drag) = &self.animator_view.drag {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
                let g = graph_of(origin, pan, zoom, p);
                if let Some(n) = nodes.iter_mut().find(|n| n.id.key() == drag.key) {
                    n.pos = [
                        drag.origin[0] + g[0] - drag.grab[0],
                        drag.origin[1] + g[1] - drag.grab[1],
                    ];
                }
            }
        }
        let selected_node = match &self.animator_view.sel {
            Some(AnimSel::Node(key)) => Some(key.clone()),
            _ => None,
        };
        let selected_transition = match self.animator_view.sel {
            Some(AnimSel::Transition(index)) => Some(index),
            _ => None,
        };
        paint_graph(
            ui,
            rect,
            pan,
            zoom,
            &nodes,
            &transitions,
            &default_state,
            live.as_ref(),
            selected_node.as_deref(),
            selected_transition,
        );

        let alt = ui.input(|i| i.modifiers.alt);
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if resp.hovered() && scroll.abs() > 0.1 {
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                let g = graph_of(origin, pan, zoom, p);
                let next = (zoom * (scroll * 0.0015).exp()).clamp(0.35, 2.5);
                pan[0] = p.x - origin.x - g[0] * next;
                pan[1] = p.y - origin.y - g[1] * next;
                zoom = next;
            }
        }
        if resp.dragged_by(PointerButton::Middle)
            || (alt && resp.dragged_by(PointerButton::Primary))
        {
            let d = resp.drag_delta();
            pan[0] += d.x;
            pan[1] += d.y;
            self.animator_view.drag = None;
        } else if self.animator_view.link_from.is_none()
            && resp.drag_started_by(PointerButton::Primary)
        {
            if let Some(p) = resp.interact_pointer_pos() {
                let g = graph_of(origin, pan, zoom, p);
                if let Some(id) = hit_node(&nodes, g[0], g[1]) {
                    if let Some(n) = nodes.iter().find(|n| n.id == id) {
                        self.animator_view.drag = Some(NodeDrag {
                            key: id.key().to_string(),
                            origin: n.pos,
                            grab: g,
                        });
                        self.animator_view.sel = Some(AnimSel::Node(id.key().to_string()));
                    }
                }
            }
        }
        if let Some(drag) = self.animator_view.drag.clone() {
            if resp.drag_stopped() {
                if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
                    let g = graph_of(origin, pan, zoom, p);
                    let pos = [
                        drag.origin[0] + g[0] - drag.grab[0],
                        drag.origin[1] + g[1] - drag.grab[1],
                    ];
                    if (pos[0] - drag.origin[0]).abs() > 0.5
                        || (pos[1] - drag.origin[1]).abs() > 0.5
                    {
                        edits.push(AnimEdit::Move {
                            node: drag.key,
                            pos,
                        });
                    }
                }
                self.animator_view.drag = None;
            }
        }
        if resp.secondary_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let g = graph_of(origin, pan, zoom, p);
                let node = hit_node(&nodes, g[0], g[1]).map(|id| id.key().to_string());
                let transition = if node.is_some() {
                    None
                } else {
                    hit_transition(&nodes, &transitions, g[0], g[1], TRANSITION_SLOP)
                };
                if let Some(key) = &node {
                    self.animator_view.sel = Some(AnimSel::Node(key.clone()));
                    self.animator_view.rename_state = key.clone();
                } else if let Some(index) = transition {
                    self.animator_view.sel = Some(AnimSel::Transition(index));
                }
                self.animator_view.menu = Some(MenuHit {
                    graph: g,
                    node,
                    transition,
                });
            }
        }
        if resp.clicked() && self.animator_view.drag.is_none() {
            if let Some(p) = resp.interact_pointer_pos() {
                let g = graph_of(origin, pan, zoom, p);
                let node = hit_node(&nodes, g[0], g[1]);
                let transition = if node.is_some() {
                    None
                } else {
                    hit_transition(&nodes, &transitions, g[0], g[1], TRANSITION_SLOP)
                };
                if let Some(from) = self.animator_view.link_from.clone() {
                    if let Some(GraphNodeId::State(to)) = node {
                        edits.push(AnimEdit::AddTransition { from, to });
                        self.animator_view.link_from = None;
                    } else if node.is_none() {
                        self.animator_view.link_from = None;
                    }
                } else if let Some(id) = node {
                    let key = id.key().to_string();
                    self.animator_view.rename_state = key.clone();
                    self.animator_view.sel = Some(AnimSel::Node(key));
                } else if let Some(index) = transition {
                    self.animator_view.sel = Some(AnimSel::Transition(index));
                } else {
                    self.animator_view.sel = None;
                }
            }
        }
        self.animator_view.pan = pan;
        self.animator_view.zoom = zoom;

        let menu = self.animator_view.menu.clone();
        let layer_name = layer.clone();
        resp.context_menu(|ui| {
            let hit = menu.clone();
            let node = hit.as_ref().and_then(|h| h.node.clone());
            let transition = hit.as_ref().and_then(|h| h.transition);
            let at = hit.as_ref().map(|h| h.graph).unwrap_or([160.0, 40.0]);
            if node.is_none() && transition.is_none() {
                ui.menu_button("Create State", |ui| {
                    if ui.button("Empty").clicked() {
                        let names: Vec<String> = states.iter().map(|s| s.name.clone()).collect();
                        edits.push(AnimEdit::AddState {
                            name: unique_label(&names, "New State"),
                            pos: at,
                            blend: false,
                        });
                        ui.close_menu();
                    }
                    if ui.button("From New Blend Tree").clicked() {
                        let names: Vec<String> = states.iter().map(|s| s.name.clone()).collect();
                        edits.push(AnimEdit::AddState {
                            name: unique_label(&names, "Blend Tree"),
                            pos: at,
                            blend: true,
                        });
                        ui.close_menu();
                    }
                });
            }
            if let Some(key) = node.clone() {
                let is_state = states.iter().any(|s| s.name == key);
                let can_from = is_state || key == "Any";
                if ui
                    .add_enabled(can_from, egui::Button::new("Make Transition"))
                    .clicked()
                {
                    let from = if key == "Any" {
                        "Any".to_string()
                    } else {
                        key.clone()
                    };
                    self.animator_view.link_from = Some(from);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_state, egui::Button::new("Set as Layer Default State"))
                    .clicked()
                {
                    edits.push(AnimEdit::SetDefault(key.clone()));
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_state, egui::Button::new("Rename"))
                    .clicked()
                {
                    self.animator_view.sel = Some(AnimSel::Node(key.clone()));
                    self.animator_view.rename_state = key.clone();
                    self.animator_view.side = AnimatorSide::Layers;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_state, egui::Button::new("Delete"))
                    .clicked()
                {
                    edits.push(AnimEdit::RemoveState(key));
                    ui.close_menu();
                }
            }
            if let Some(index) = transition {
                if ui.button("Delete").clicked() {
                    edits.push(AnimEdit::RemoveTransition(index));
                    ui.close_menu();
                }
            }
            let _ = layer_name;
        });
    }

    fn apply_animator_edits(&mut self, stem: &str, edits: Vec<AnimEdit>) {
        if edits.is_empty() {
            return;
        }
        let assets = self.project.assets_path(&self.game_dir);
        let layer = self.animator_view.layer.clone();
        let layer_arg = Some(layer.as_str());
        for edit in edits {
            let result = match edit {
                AnimEdit::AddLayer(name) => {
                    let created = name.clone();
                    match add_controller_layer(&assets, stem, &name, 1.0) {
                        Ok(v) => {
                            self.animator_view.layer = created;
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::RemoveLayer(name) => {
                    match remove_controller_layer(&assets, stem, &name) {
                        Ok(v) => {
                            if self.animator_view.layer == name {
                                self.animator_view.layer = BASE_LAYER_NAME.into();
                            }
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::RenameLayer { from, to } => {
                    match rename_controller_layer(&assets, stem, &from, &to) {
                        Ok(v) => {
                            self.animator_view.layer = to;
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::Weight { name, weight } => {
                    set_controller_layer_weight(&assets, stem, &name, weight)
                }
                AnimEdit::AddParam(kind) => {
                    let meta = self.controller_catalog.lookup(stem).cloned();
                    let names = meta
                        .as_ref()
                        .map(|m| {
                            m.parameters
                                .iter()
                                .map(|p| p.name.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let label = match kind {
                        ControllerParamType::Bool => "New Bool",
                        ControllerParamType::Float => "New Float",
                        ControllerParamType::Trigger => "New Trigger",
                    };
                    let name = unique_label(&names, label);
                    let default = match kind {
                        ControllerParamType::Float => Some(serde_json::json!(0.0)),
                        _ => Some(serde_json::json!(false)),
                    };
                    add_controller_param(
                        &assets,
                        stem,
                        ControllerParam {
                            name,
                            kind,
                            default,
                        },
                    )
                }
                AnimEdit::RenameParam { from, to } => {
                    match rename_controller_param(&assets, stem, &from, &to) {
                        Ok(v) => {
                            self.animator_view.rename_param = to.clone();
                            self.animator_view.param_draft = to;
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::RemoveParam(name) => remove_controller_param(&assets, stem, &name),
                AnimEdit::ParamDefault { name, value } => {
                    set_controller_param_default(&assets, stem, &name, value)
                }
                AnimEdit::AddState { name, pos, blend } => {
                    let created = name.clone();
                    let written = if blend {
                        let param = self
                            .controller_catalog
                            .lookup(stem)
                            .and_then(|m| {
                                m.parameters
                                    .iter()
                                    .find(|p| p.kind == ControllerParamType::Float)
                                    .map(|p| p.name.clone())
                            })
                            .unwrap_or_else(|| "Blend".into());
                        write_layer_state_blend_tree(
                            &assets,
                            stem,
                            layer_arg,
                            &name,
                            BlendTreeMeta {
                                dimension: BlendDimension::OneD,
                                params: vec![param],
                                motions: vec![],
                            },
                        )
                    } else {
                        write_layer_state(&assets, stem, layer_arg, &name, "")
                    };
                    match written {
                        Ok(_) => {
                            self.animator_view.sel = Some(AnimSel::Node(created.clone()));
                            self.animator_view.rename_state = created.clone();
                            move_layer_node(&assets, stem, layer_arg, &created, pos)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::RenameState { from, to } => {
                    match rename_layer_state(&assets, stem, layer_arg, &from, &to) {
                        Ok(v) => {
                            self.animator_view.sel = Some(AnimSel::Node(to.clone()));
                            self.animator_view.rename_state = to;
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::RemoveState(name) => {
                    match remove_layer_state(&assets, stem, layer_arg, &name) {
                        Ok(v) => {
                            if matches!(&self.animator_view.sel, Some(AnimSel::Node(n)) if n == &name)
                            {
                                self.animator_view.sel = None;
                            }
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::SetDefault(name) => set_layer_default(&assets, stem, layer_arg, &name),
                AnimEdit::AddTransition { from, to } => add_layer_transition(
                    &assets,
                    stem,
                    layer_arg,
                    ControllerTransition {
                        from,
                        to,
                        conditions: vec![],
                        has_exit_time: false,
                    },
                ),
                AnimEdit::RemoveTransition(index) => {
                    match remove_layer_transition(&assets, stem, layer_arg, index) {
                        Ok(v) => {
                            if matches!(self.animator_view.sel, Some(AnimSel::Transition(i)) if i == index)
                            {
                                self.animator_view.sel = None;
                            }
                            Ok(v)
                        }
                        Err(e) => Err(e),
                    }
                }
                AnimEdit::Move { node, pos } => {
                    move_layer_node(&assets, stem, layer_arg, &node, pos)
                }
                AnimEdit::SetClip { state, clip } => {
                    write_layer_state_clip(&assets, stem, layer_arg, &state, &clip)
                }
                AnimEdit::SetExit { index, value } => {
                    set_layer_transition(&assets, stem, layer_arg, index, Some(value), None)
                }
                AnimEdit::SetConditions { index, conditions } => {
                    set_layer_transition(&assets, stem, layer_arg, index, None, Some(conditions))
                }
            };
            if let Err(e) = result {
                self.status = format!("animator: {e:#}");
                return;
            }
        }
        match self.reload_assets() {
            Ok(()) => {
                self.rehydrate();
                self.status = format!("saved {stem}.controller.json");
            }
            Err(e) => self.status = format!("animator reload: {e:#}"),
        }
    }
}

fn machine_of(
    meta: &AnimatorControllerMeta,
    layer: &str,
) -> (
    Vec<wiimaker_assets::ControllerState>,
    Vec<ControllerTransition>,
) {
    if is_base_layer_name(layer) {
        (meta.states.clone(), meta.transitions.clone())
    } else {
        meta.layer(layer)
            .map(|l| (l.states.clone(), l.transitions.clone()))
            .unwrap_or_default()
    }
}

fn machine_parts(
    meta: &AnimatorControllerMeta,
    layer: &str,
) -> (
    String,
    Vec<wiimaker_assets::ControllerState>,
    Vec<ControllerTransition>,
    wiimaker_assets::ControllerGraph,
) {
    if is_base_layer_name(layer) {
        (
            meta.default_state.clone(),
            meta.states.clone(),
            meta.transitions.clone(),
            meta.graph.clone(),
        )
    } else if let Some(l) = meta.layer(layer) {
        (
            l.default_state.clone(),
            l.states.clone(),
            l.transitions.clone(),
            l.graph.clone(),
        )
    } else {
        (String::new(), Vec::new(), Vec::new(), Default::default())
    }
}

fn paint_graph(
    ui: &egui::Ui,
    rect: Rect,
    pan: [f32; 2],
    zoom: f32,
    nodes: &[GraphNode],
    transitions: &[ControllerTransition],
    default_state: &str,
    live: Option<&(String, f32)>,
    selected: Option<&str>,
    selected_transition: Option<usize>,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(36, 36, 36));
    let step = 24.0_f32;
    let origin = rect.min;
    let min_g = graph_of(origin, pan, zoom, rect.min);
    let max_g = graph_of(origin, pan, zoom, rect.max);
    let grid = Color32::from_rgb(52, 52, 52);
    let mut x = (min_g[0] / step).floor() * step;
    while x <= max_g[0] {
        let a = screen_of(origin, pan, zoom, [x, min_g[1]]);
        let b = screen_of(origin, pan, zoom, [x, max_g[1]]);
        painter.line_segment([a, b], Stroke::new(1.0_f32, grid));
        x += step;
    }
    let mut y = (min_g[1] / step).floor() * step;
    while y <= max_g[1] {
        let a = screen_of(origin, pan, zoom, [min_g[0], y]);
        let b = screen_of(origin, pan, zoom, [max_g[0], y]);
        painter.line_segment([a, b], Stroke::new(1.0_f32, grid));
        y += step;
    }

    if let Some(entry) = nodes.iter().find(|n| n.id == GraphNodeId::Entry) {
        if let Some(dest) = nodes
            .iter()
            .find(|n| matches!(&n.id, GraphNodeId::State(s) if s == default_state))
        {
            paint_edge(
                &painter,
                origin,
                pan,
                zoom,
                entry.center(),
                dest.center(),
                Color32::from_rgb(120, 170, 120),
            );
        }
    }
    for (t_index, t) in transitions.iter().enumerate() {
        let from_key = if is_any_state(&t.from) {
            "Any"
        } else {
            t.from.as_str()
        };
        let Some(from) = nodes.iter().find(|n| n.id.key() == from_key) else {
            continue;
        };
        let Some(to) = nodes.iter().find(|n| n.id.key() == t.to) else {
            continue;
        };
        let color = if selected_transition == Some(t_index) {
            Color32::from_rgb(109, 180, 232)
        } else {
            Color32::from_rgb(186, 186, 186)
        };
        paint_edge(
            &painter,
            origin,
            pan,
            zoom,
            from.center(),
            to.center(),
            color,
        );
    }
    for n in nodes {
        let a = screen_of(origin, pan, zoom, n.pos);
        let b = screen_of(
            origin,
            pan,
            zoom,
            [n.pos[0] + n.size[0], n.pos[1] + n.size[1]],
        );
        let r = Rect::from_min_max(a, b);
        let (fill, mut stroke) = node_colors(n);
        let mut width = 1.0_f32;
        if selected == Some(n.id.key()) {
            stroke = Color32::from_rgb(109, 180, 232);
            width = 2.0_f32;
        }
        painter.rect_filled(r, 4.0 * zoom.max(0.6), fill);
        painter.rect_stroke(r, 4.0 * zoom.max(0.6), Stroke::new(width, stroke));
        let label = if n.blend {
            format!("{}  Blend", n.label)
        } else {
            n.label.clone()
        };
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            Color32::from_rgb(236, 236, 236),
        );
        if let Some((state, t)) = live {
            if n.id.key() == state {
                let w = (r.width() - 4.0) * t.clamp(0.0, 1.0);
                let bar_h = (r.height() * 0.22).clamp(4.0, 7.0);
                let bar = Rect::from_min_max(
                    pos2(r.left() + 2.0, r.bottom() - bar_h),
                    pos2(r.left() + 2.0 + w, r.bottom() - 1.0),
                );
                painter.rect_filled(bar, 1.0, Color32::from_rgb(70, 150, 230));
            }
        }
    }
}

fn paint_edge(
    painter: &egui::Painter,
    origin: Pos2,
    pan: [f32; 2],
    zoom: f32,
    a: [f32; 2],
    b: [f32; 2],
    color: Color32,
) {
    let sa = screen_of(origin, pan, zoom, a);
    let sb = screen_of(origin, pan, zoom, b);
    painter.line_segment([sa, sb], Stroke::new(1.5_f32, color));
    let head = arrow_head(a, b, 7.0);
    let pts: Vec<Pos2> = head
        .into_iter()
        .map(|p| screen_of(origin, pan, zoom, p))
        .collect();
    painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn node_colors(n: &GraphNode) -> (Color32, Color32) {
    match n.role {
        wiimaker_assets::NodeRole::Entry => (
            Color32::from_rgb(46, 122, 64),
            Color32::from_rgb(110, 190, 120),
        ),
        wiimaker_assets::NodeRole::Any => (
            Color32::from_rgb(32, 112, 122),
            Color32::from_rgb(90, 190, 196),
        ),
        wiimaker_assets::NodeRole::Exit => (
            Color32::from_rgb(148, 52, 52),
            Color32::from_rgb(210, 96, 96),
        ),
        wiimaker_assets::NodeRole::Default => (
            Color32::from_rgb(176, 102, 36),
            Color32::from_rgb(230, 160, 80),
        ),
        wiimaker_assets::NodeRole::State => (
            Color32::from_rgb(78, 78, 78),
            Color32::from_rgb(120, 120, 120),
        ),
    }
}
