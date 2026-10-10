//! Animator controller asset Inspector: 1D / 2D blend trees on `*.controller.json` states.
//!
//! Writes go through `wiimaker_assets::write_state_blend_tree` / `write_state_clip`, the same
//! helpers `wiimaker asset blend-tree` calls, so GUI and CLI produce identical files.

use anyhow::Result;
use eframe::egui::{self, RichText};
use wiimaker_assets::{
    add_controller_layer, remove_controller_layer, set_controller_layer_weight,
    write_layer_state_blend_tree, write_layer_state_clip, BlendDimension, BlendMotion,
    BlendTreeMeta, ControllerParamType, ControllerState, BASE_LAYER_NAME,
};

use crate::app::EditorApp;
use crate::theme;

enum ControllerEdit {
    Tree {
        layer: Option<String>,
        state: String,
        tree: BlendTreeMeta,
    },
    Clip {
        layer: Option<String>,
        state: String,
        clip: String,
    },
    AddLayer {
        name: String,
    },
    RemoveLayer {
        name: String,
    },
    Weight {
        name: String,
        weight: f32,
    },
}

pub(crate) fn blend_state_label(tree: &BlendTreeMeta) -> String {
    let dim = tree.dimension.as_str();
    format!("Blend {dim} ({})", tree.params.join(", "))
}

pub(crate) fn controller_state_label(state: &ControllerState) -> String {
    match &state.blend_tree {
        Some(tree) => blend_state_label(tree),
        None => state.clip.clone(),
    }
}

impl EditorApp {
    pub(crate) fn ui_controller_asset_inspector(
        &mut self,
        ui: &mut egui::Ui,
        rel: &std::path::Path,
    ) {
        let file = rel.to_string_lossy().replace('\\', "/");
        let Some(stem) = file.strip_suffix(".controller.json") else {
            return;
        };
        let stem = stem.rsplit('/').next().unwrap_or(stem).to_string();
        let Some(meta) = self.controller_catalog.lookup(&stem).cloned() else {
            theme::muted(ui, "Controller failed to load");
            return;
        };
        let clips: Vec<String> = self.anim_catalog.names().to_vec();
        let floats: Vec<String> = meta
            .parameters
            .iter()
            .filter(|p| p.kind == ControllerParamType::Float)
            .map(|p| p.name.clone())
            .collect();
        let assets = self.project.assets_path(&self.game_dir);
        let mut edit: Option<ControllerEdit> = None;
        let layer_id = egui::Id::new(("controller_layer", &stem));
        let mut selected = ui
            .data_mut(|d| d.get_temp::<String>(layer_id))
            .filter(|name| name == BASE_LAYER_NAME || meta.layers.iter().any(|l| l.name == *name))
            .unwrap_or_else(|| BASE_LAYER_NAME.to_string());

        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            ui.label(
                RichText::new("Layers")
                    .strong()
                    .size(13.0)
                    .color(theme::TEXT),
            );
            theme::muted(
                ui,
                "Override only — sprites play one clip. A layer with weight > 0 replaces the base.",
            );
            let mut rows: Vec<(String, f32, bool)> =
                vec![(BASE_LAYER_NAME.into(), meta.weight, true)];
            for layer in &meta.layers {
                rows.push((layer.name.clone(), layer.weight, false));
            }
            for (name, weight, _base) in rows {
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected == name, &name).clicked() {
                        selected = name.clone();
                    }
                    let mut w = weight.clamp(0.0, 1.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut w)
                                .speed(0.01)
                                .range(0.0..=1.0)
                                .max_decimals(2)
                                .prefix("w "),
                        )
                        .changed()
                    {
                        edit = Some(ControllerEdit::Weight {
                            name: name.clone(),
                            weight: w,
                        });
                    }
                });
            }
            let draft_layer = egui::Id::new(("controller_new_layer", &stem));
            let mut draft = ui.data_mut(|d| d.get_temp::<String>(draft_layer).unwrap_or_default());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft)
                        .desired_width(120.0)
                        .hint_text("UpperBody"),
                );
                let trimmed = draft.trim().to_string();
                let taken = trimmed.eq_ignore_ascii_case(BASE_LAYER_NAME)
                    || meta
                        .layers
                        .iter()
                        .any(|l| l.name.eq_ignore_ascii_case(&trimmed));
                let add = ui.add_enabled(
                    !trimmed.is_empty() && !taken,
                    egui::Button::new("Add Layer"),
                );
                if add.clicked() {
                    edit = Some(ControllerEdit::AddLayer { name: trimmed });
                    draft.clear();
                }
                let can_remove = selected != BASE_LAYER_NAME;
                if ui
                    .add_enabled(can_remove, egui::Button::new("Remove"))
                    .clicked()
                {
                    edit = Some(ControllerEdit::RemoveLayer {
                        name: selected.clone(),
                    });
                }
            });
            ui.data_mut(|d| d.insert_temp(draft_layer, draft));
        });
        ui.data_mut(|d| d.insert_temp(layer_id, selected.clone()));

        let layer_arg = if selected == BASE_LAYER_NAME {
            None
        } else {
            Some(selected.clone())
        };
        let states: Vec<ControllerState> = if let Some(name) = &layer_arg {
            meta.layer(name)
                .map(|l| l.states.clone())
                .unwrap_or_default()
        } else {
            meta.states.clone()
        };
        let (default_state, transitions) = if let Some(name) = &layer_arg {
            meta.layer(name)
                .map(|l| (l.default_state.clone(), l.transitions.clone()))
                .unwrap_or_default()
        } else {
            (meta.default_state.clone(), meta.transitions.clone())
        };

        ui.add_space(8.0);
        theme::card_frame().show(ui, |ui| {
            ui.label(
                RichText::new("Animator Controller")
                    .strong()
                    .size(13.0)
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new(format!("{selected}  ·  Default · {default_state}"))
                    .size(11.0)
                    .color(theme::TEXT_MUTED),
            );
            for state in &states {
                ui.add_space(6.0);
                match &state.blend_tree {
                    None => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&state.name).color(theme::TEXT));
                            ui.label(
                                RichText::new(format!("·  clip {}", state.clip))
                                    .size(11.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                        let convert = ui.add_enabled(
                            !floats.is_empty(),
                            egui::Button::new("Convert to Blend Tree"),
                        );
                        let convert = if floats.is_empty() {
                            convert.on_disabled_hover_text(
                                "Add a Float parameter first (asset controller --param Name:Float=0)",
                            )
                        } else {
                            convert
                        };
                        if convert.clicked() {
                            edit = Some(ControllerEdit::Tree {
                                layer: layer_arg.clone(),
                                state: state.name.clone(),
                                tree: BlendTreeMeta {
                                    dimension: BlendDimension::OneD,
                                    params: vec![floats[0].clone()],
                                    motions: vec![BlendMotion {
                                        clip: state.clip.clone(),
                                        threshold: Some(0.0),
                                        position: None,
                                    }],
                                },
                            });
                        }
                    }
                    Some(tree) => {
                        if let Some(next) = blend_tree_card(
                            ui,
                            layer_arg.as_deref(),
                            &state.name,
                            tree,
                            &floats,
                            &clips,
                        ) {
                            edit = Some(next);
                        }
                    }
                }
            }
            if !transitions.is_empty() {
                ui.add_space(6.0);
                for t in &transitions {
                    ui.label(
                        RichText::new(format!("{} -> {}", t.from, t.to))
                            .size(11.0)
                            .color(theme::TEXT_DIM),
                    );
                }
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new("Add Blend Tree State")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            let draft_id = egui::Id::new(("controller_new_state", &stem));
            let mut draft = ui.data_mut(|d| d.get_temp::<String>(draft_id).unwrap_or_default());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft)
                        .desired_width(120.0)
                        .hint_text("Locomotion"),
                );
                let trimmed = draft.trim().to_string();
                let taken = states.iter().any(|s| s.name == trimmed);
                let can_add = !trimmed.is_empty() && !taken && !floats.is_empty();
                let add = ui.add_enabled(can_add, egui::Button::new("Add"));
                if add.clicked() {
                    edit = Some(ControllerEdit::Tree {
                        layer: layer_arg.clone(),
                        state: trimmed,
                        tree: BlendTreeMeta {
                            dimension: BlendDimension::OneD,
                            params: vec![floats[0].clone()],
                            motions: Vec::new(),
                        },
                    });
                    draft.clear();
                }
            });
            ui.data_mut(|d| d.insert_temp(draft_id, draft));
            theme::muted(
                ui,
                "Sprites can't cross-fade: the highest-weight motion plays its cells.",
            );
        });

        let result: Option<(String, Result<()>)> = match edit {
            None => None,
            Some(ControllerEdit::Tree { layer, state, tree }) => Some((
                state.clone(),
                write_layer_state_blend_tree(&assets, &stem, layer.as_deref(), &state, tree)
                    .map(|_| ()),
            )),
            Some(ControllerEdit::Clip { layer, state, clip }) => Some((
                state.clone(),
                write_layer_state_clip(&assets, &stem, layer.as_deref(), &state, &clip).map(|_| ()),
            )),
            Some(ControllerEdit::AddLayer { name }) => Some((
                name.clone(),
                add_controller_layer(&assets, &stem, &name, 1.0).map(|_| ()),
            )),
            Some(ControllerEdit::RemoveLayer { name }) => {
                ui.data_mut(|d| d.insert_temp(layer_id, BASE_LAYER_NAME.to_string()));
                Some((
                    name.clone(),
                    remove_controller_layer(&assets, &stem, &name).map(|_| ()),
                ))
            }
            Some(ControllerEdit::Weight { name, weight }) => Some((
                name.clone(),
                set_controller_layer_weight(&assets, &stem, &name, weight).map(|_| ()),
            )),
        };
        if let Some((state, res)) = result {
            match res {
                Ok(()) => match self.reload_assets() {
                    Ok(()) => {
                        self.rehydrate();
                        self.status = format!("saved {stem}.controller.json · {state}");
                    }
                    Err(e) => self.status = format!("controller reload: {e}"),
                },
                Err(e) => self.status = format!("controller save: {e}"),
            }
        }
    }
}

fn blend_tree_card(
    ui: &mut egui::Ui,
    layer: Option<&str>,
    state: &str,
    tree: &BlendTreeMeta,
    floats: &[String],
    clips: &[String],
) -> Option<ControllerEdit> {
    let mut work = tree.clone();
    let mut changed = false;
    let mut clip_to: Option<String> = None;

    ui.label(
        RichText::new(format!("{state}  ·  {}", blend_state_label(tree)))
            .size(12.0)
            .color(theme::TEXT),
    );

    let mut dim = work.dimension;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Type").size(11.0).color(theme::TEXT_MUTED));
        egui::ComboBox::from_id_salt(("blend_dim", layer, state))
            .selected_text(dim.as_str())
            .width(64.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut dim, BlendDimension::OneD, "1D");
                let two_ok = floats.len() >= 2;
                ui.add_enabled_ui(two_ok, |ui| {
                    ui.selectable_value(&mut dim, BlendDimension::TwoD, "2D");
                })
                .response
                .on_disabled_hover_text("2D needs two Float parameters");
            });
    });
    if dim != work.dimension {
        convert_dimension(&mut work, dim, floats);
        changed = true;
    }

    let slots = work.dimension.param_count();
    ui.horizontal(|ui| {
        for i in 0..slots {
            let label = if slots == 1 {
                "Param"
            } else if i == 0 {
                "X"
            } else {
                "Y"
            };
            ui.label(RichText::new(label).size(11.0).color(theme::TEXT_MUTED));
            let current = work.params.get(i).cloned().unwrap_or_default();
            let shown = if current.is_empty() {
                "(none)".to_string()
            } else {
                current.clone()
            };
            egui::ComboBox::from_id_salt(("blend_param", layer, state, i))
                .selected_text(shown)
                .width(100.0)
                .show_ui(ui, |ui| {
                    for f in floats {
                        if ui.selectable_label(current == *f, f).clicked() {
                            if let Some(slot) = work.params.get_mut(i) {
                                *slot = f.clone();
                            }
                            changed = true;
                        }
                    }
                });
        }
    });

    let dimension = work.dimension;
    let mut remove: Option<usize> = None;
    for (i, m) in work.motions.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let shown = if m.clip.is_empty() {
                "(clip)"
            } else {
                m.clip.as_str()
            };
            egui::ComboBox::from_id_salt(("blend_clip", layer, state, i))
                .selected_text(shown)
                .width(90.0)
                .show_ui(ui, |ui| {
                    for c in clips {
                        if ui.selectable_label(m.clip == *c, c).clicked() {
                            m.clip = c.clone();
                            changed = true;
                        }
                    }
                });
            match dimension {
                BlendDimension::OneD => {
                    let mut t = m.threshold.unwrap_or(0.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut t)
                                .speed(0.05)
                                .max_decimals(3)
                                .prefix("at "),
                        )
                        .changed()
                    {
                        m.threshold = Some(t);
                        changed = true;
                    }
                }
                BlendDimension::TwoD => {
                    let mut p = m.position.unwrap_or([0.0, 0.0]);
                    let mut moved = false;
                    moved |= ui
                        .add(
                            egui::DragValue::new(&mut p[0])
                                .speed(0.02)
                                .range(-1.0..=1.0)
                                .prefix("x "),
                        )
                        .changed();
                    moved |= ui
                        .add(
                            egui::DragValue::new(&mut p[1])
                                .speed(0.02)
                                .range(-1.0..=1.0)
                                .prefix("y "),
                        )
                        .changed();
                    if moved {
                        m.position = Some(p);
                        changed = true;
                    }
                }
            }
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        work.motions.remove(i);
        changed = true;
    }

    ui.horizontal(|ui| {
        if ui
            .add_enabled(!clips.is_empty(), egui::Button::new("Add Motion"))
            .on_disabled_hover_text("Create an animation clip first (asset anim)")
            .clicked()
        {
            let motion = match work.dimension {
                BlendDimension::OneD => BlendMotion {
                    clip: clips[0].clone(),
                    threshold: Some(0.0),
                    position: None,
                },
                BlendDimension::TwoD => BlendMotion {
                    clip: clips[0].clone(),
                    threshold: None,
                    position: Some([0.0, 0.0]),
                },
            };
            work.motions.push(motion);
            changed = true;
        }
        let first_clip = tree.motions.first().map(|m| m.clip.clone());
        if ui
            .add_enabled(first_clip.is_some(), egui::Button::new("Use Clip"))
            .on_hover_text("Replace the tree with the first motion's clip")
            .on_disabled_hover_text("Add a motion first")
            .clicked()
        {
            clip_to = first_clip;
        }
    });

    let layer = layer.map(|s| s.to_string());
    if let Some(clip) = clip_to {
        return Some(ControllerEdit::Clip {
            layer,
            state: state.to_string(),
            clip,
        });
    }
    changed.then(|| ControllerEdit::Tree {
        layer,
        state: state.to_string(),
        tree: work,
    })
}

fn convert_dimension(tree: &mut BlendTreeMeta, dim: BlendDimension, floats: &[String]) {
    tree.dimension = dim;
    tree.params = match dim {
        BlendDimension::OneD => tree.params.iter().take(1).cloned().collect(),
        BlendDimension::TwoD => {
            let x = tree
                .params
                .first()
                .cloned()
                .unwrap_or_else(|| floats[0].clone());
            let y = floats
                .iter()
                .find(|f| **f != x)
                .cloned()
                .unwrap_or_else(|| x.clone());
            vec![x, y]
        }
    };
    for m in &mut tree.motions {
        match dim {
            BlendDimension::TwoD => {
                let x = m.threshold.unwrap_or(0.0);
                m.threshold = None;
                m.position = Some([x.clamp(-1.0, 1.0), 0.0]);
            }
            BlendDimension::OneD => {
                let x = m.position.map(|p| p[0]).unwrap_or(0.0);
                m.position = None;
                m.threshold = Some(x);
            }
        }
    }
}
