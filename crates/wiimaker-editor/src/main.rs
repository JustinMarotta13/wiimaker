//! egui scene editor for wiimaker games.
//!
//! Unity-shaped panels: Hierarchy · Scene viewport · Inspector · Project

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use eframe::egui;
use wiimaker_assets::WPack;
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::World;
use wiimaker_host::{flush_with_atlas, Framebuffer, TextureAtlas};
use wiimaker_scene::{
    add_component_disc, add_component_sprite, add_entity, diagnose, duplicate_entity, find_game_dir,
    hydrate_lenient, insert_entity_clone, load_project, load_scene, pick_entity_at, pointer_to_scene,
    remove_entity, rename_entity, render_world, save_scene, set_entity_transform, unique_entity_name,
    EntityData, GameProject, MutateOpts, Scene, UndoStack,
};

const VIEW_W: usize = 640;
const VIEW_H: usize = 480;

fn main() -> eframe::Result<()> {
    let game = std::env::args().nth(1).unwrap_or_else(|| "hello-orb".into());
    let root = find_root().expect("wiimaker workspace root");
    let state = EditorApp::open(&root, &game).unwrap_or_else(|e| {
        eprintln!("editor error: {e:#}");
        std::process::exit(1);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title(format!("wiimaker · {}", state.project.title)),
        ..Default::default()
    };
    eframe::run_native(
        "wiimaker editor",
        options,
        Box::new(|_cc| Ok(Box::new(state))),
    )
}

/// Active viewport translate drag (scene-space grab offset from entity origin).
#[derive(Clone)]
struct ViewportDrag {
    entity: String,
    /// `pointer_scene - translation` at drag start so the entity doesn't jump.
    grab_offset: [f32; 2],
}

struct EditorApp {
    root: PathBuf,
    game_dir: PathBuf,
    project: GameProject,
    scene: Scene,
    scene_path: PathBuf,
    dirty: bool,
    selected: Option<String>,
    atlas: TextureAtlas,
    world: World,
    fb: Framebuffer,
    texture_handle: Option<egui::TextureHandle>,
    status: String,
    new_entity_name: String,
    asset_names: Vec<String>,
    undo: UndoStack,
    /// Scene snapshot before the current inspector drag gesture.
    undo_baseline: Scene,
    inspector_gesture: bool,
    clipboard: Option<EntityData>,
    rename_draft: String,
    viewport_drag: Option<ViewportDrag>,
}

impl EditorApp {
    fn open(root: &std::path::Path, game: &str) -> Result<Self> {
        let game_dir = find_game_dir(root, game)?;
        let project = load_project(&game_dir).with_context(|| "load game.toml")?;
        let scene_path = project.scene_path(&game_dir);
        let scene = load_scene(&scene_path)?;
        let mut app = Self {
            root: root.to_path_buf(),
            game_dir,
            project,
            undo_baseline: scene.clone(),
            scene,
            scene_path,
            dirty: false,
            selected: None,
            atlas: TextureAtlas::empty(),
            world: World::new(),
            fb: Framebuffer::new(VIEW_W, VIEW_H),
            texture_handle: None,
            status: String::new(),
            new_entity_name: "NewEntity".into(),
            asset_names: Vec::new(),
            undo: UndoStack::with_default_depth(),
            inspector_gesture: false,
            clipboard: None,
            rename_draft: String::new(),
            viewport_drag: None,
        };
        app.reload_assets()?;
        app.rehydrate();
        app.status = format!("opened {}", app.project.name);
        Ok(app)
    }

    fn reload_assets(&mut self) -> Result<()> {
        let assets = self.project.assets_path(&self.game_dir);
        self.asset_names.clear();
        if assets.is_dir() {
            let mut names = Vec::new();
            for entry in std::fs::read_dir(&assets)? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) == Some("png") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
            names.sort();
            self.asset_names = names;
        }

        let wpack_path = self.project.wpack_path(&self.game_dir);
        if wpack_path.is_file() {
            let pack = WPack::read_from(&wpack_path)?;
            self.atlas = TextureAtlas::from_wpack(&pack);
        } else if assets.is_dir() {
            let mut pack = WPack::new();
            let _ = pack.cook_dir(&assets)?;
            self.atlas = TextureAtlas::from_wpack(&pack);
        } else {
            self.atlas = TextureAtlas::empty();
        }
        Ok(())
    }

    fn rehydrate(&mut self) {
        self.world = hydrate_lenient(&self.scene, self.atlas.map());
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.rehydrate();
    }

    fn sync_baseline(&mut self) {
        self.undo_baseline = self.scene.clone();
        self.inspector_gesture = false;
    }

    /// Push undo for a discrete mutation (buttons, rename, add/remove, etc.).
    fn push_undo(&mut self) {
        self.undo.push(&self.scene);
        self.inspector_gesture = false;
    }

    /// First change of an inspector slider gesture: push pre-edit baseline once.
    fn begin_inspector_gesture(&mut self) {
        if !self.inspector_gesture {
            self.undo.push(&self.undo_baseline);
            self.inspector_gesture = true;
        }
    }

    fn end_inspector_gesture_if_released(&mut self, ctx: &egui::Context) {
        if self.inspector_gesture && !ctx.input(|i| i.pointer.any_down()) {
            self.sync_baseline();
        }
    }

    fn select(&mut self, name: Option<String>) {
        self.selected = name.clone();
        self.rename_draft = name.unwrap_or_default();
    }

    fn prune_selection(&mut self) {
        if let Some(sel) = self.selected.clone() {
            if !self.scene.entities.iter().any(|e| e.name == sel) {
                self.select(None);
            } else {
                self.rename_draft = sel;
            }
        }
    }

    fn do_undo(&mut self) {
        if self.undo.undo(&mut self.scene) {
            self.prune_selection();
            self.sync_baseline();
            self.mark_dirty();
            self.status = "undo".into();
        }
    }

    fn do_redo(&mut self) {
        if self.undo.redo(&mut self.scene) {
            self.prune_selection();
            self.sync_baseline();
            self.mark_dirty();
            self.status = "redo".into();
        }
    }

    fn do_duplicate(&mut self) {
        let Some(sel) = self.selected.clone() else {
            return;
        };
        self.push_undo();
        match duplicate_entity(&mut self.scene, &sel) {
            Ok(new_name) => {
                self.select(Some(new_name.clone()));
                self.sync_baseline();
                self.mark_dirty();
                self.status = format!("duplicated → {new_name}");
            }
            Err(e) => {
                let _ = self.undo.undo(&mut self.scene);
                self.status = format!("duplicate failed: {e}");
            }
        }
    }

    fn do_copy(&mut self) {
        let Some(sel) = self.selected.as_deref() else {
            return;
        };
        if let Some(ent) = self.scene.entities.iter().find(|e| e.name == sel) {
            self.clipboard = Some(ent.clone());
            self.status = format!("copied {sel}");
        }
    }

    fn do_paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            return;
        };
        self.push_undo();
        let new_name = insert_entity_clone(&mut self.scene, &clip);
        self.select(Some(new_name.clone()));
        self.sync_baseline();
        self.mark_dirty();
        self.status = format!("pasted {new_name}");
    }

    fn commit_rename(&mut self) {
        let Some(old) = self.selected.clone() else {
            return;
        };
        let new = self.rename_draft.trim().to_string();
        if new == old {
            return;
        }
        self.push_undo();
        match rename_entity(&mut self.scene, &old, &new) {
            Ok(()) => {
                self.select(Some(new.clone()));
                self.sync_baseline();
                self.mark_dirty();
                self.status = format!("renamed {old} → {new}");
            }
            Err(e) => {
                let _ = self.undo.undo(&mut self.scene);
                self.rename_draft = old;
                self.status = format!("rename failed: {e}");
            }
        }
    }

    fn save(&mut self) {
        match save_scene(&self.scene_path, &self.scene) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("saved {}", self.scene_path.display());
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    fn cook(&mut self) {
        let assets = self.project.assets_path(&self.game_dir);
        let out = self.project.wpack_path(&self.game_dir);
        let mut pack = WPack::new();
        match pack.cook_dir(&assets) {
            Ok(warnings) => {
                if let Err(e) = pack.write_to(&out) {
                    self.status = format!("cook write failed: {e}");
                    return;
                }
                self.atlas = TextureAtlas::from_wpack(&pack);
                self.rehydrate();
                let warn = if warnings.is_empty() {
                    String::new()
                } else {
                    format!(" ({} warnings)", warnings.len())
                };
                self.status = format!("cooked {}{warn}", out.display());
            }
            Err(e) => self.status = format!("cook failed: {e}"),
        }
    }

    fn doctor(&mut self) {
        let diag = diagnose(&self.game_dir, &self.project);
        self.status = if diag.ok {
            format!("doctor ok ({} notes)", diag.issues.len())
        } else {
            format!(
                "doctor: {}",
                diag.issues
                    .iter()
                    .map(|i| i.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
    }

    fn play(&mut self) {
        let status = Command::new("cargo")
            .args(["run", "-p", &self.project.name])
            .current_dir(&self.root)
            .spawn();
        match status {
            Ok(_) => self.status = "launched play".into(),
            Err(e) => self.status = format!("play failed: {e}"),
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (cmd_s, cmd_z, cmd_shift_z, cmd_y, cmd_d, cmd_c, cmd_v) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            (
                cmd && i.key_pressed(egui::Key::S),
                cmd && !shift && i.key_pressed(egui::Key::Z),
                cmd && shift && i.key_pressed(egui::Key::Z),
                cmd && i.key_pressed(egui::Key::Y),
                cmd && i.key_pressed(egui::Key::D),
                cmd && i.key_pressed(egui::Key::C),
                cmd && i.key_pressed(egui::Key::V),
            )
        });
        if cmd_s {
            self.save();
        }
        if cmd_z {
            self.do_undo();
        }
        if cmd_shift_z || cmd_y {
            self.do_redo();
        }
        if cmd_d {
            self.do_duplicate();
        }
        if cmd_c {
            self.do_copy();
        }
        if cmd_v {
            self.do_paste();
        }

        self.end_inspector_gesture_if_released(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save scene").clicked() {
                        self.save();
                        ui.close_menu();
                    }
                    if ui.button("Cook assets").clicked() {
                        self.cook();
                        ui.close_menu();
                    }
                    if ui.button("Doctor").clicked() {
                        self.doctor();
                        ui.close_menu();
                    }
                    if ui.button("Play").clicked() {
                        self.play();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(self.undo.can_undo(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        self.do_undo();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.undo.can_redo(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        self.do_redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Duplicate"))
                        .clicked()
                    {
                        self.do_duplicate();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Copy"))
                        .clicked()
                    {
                        self.do_copy();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.clipboard.is_some(), egui::Button::new("Paste"))
                        .clicked()
                    {
                        self.do_paste();
                        ui.close_menu();
                    }
                });
                ui.label(if self.dirty { "● dirty" } else { "○ saved" });
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::TopBottomPanel::bottom("project").show(ctx, |ui| {
            ui.heading("Project");
            ui.horizontal(|ui| {
                ui.label(format!("game: {}", self.project.name));
                ui.label(format!("scene: {}", self.scene.name));
                ui.label(format!("assets: {}", self.asset_names.len()));
            });
            ui.horizontal_wrapped(|ui| {
                for name in self.asset_names.clone() {
                    if ui.button(&name).clicked() {
                        if let Some(sel) = self.selected.clone() {
                            self.push_undo();
                            let mut applied = false;
                            if let Some(ent) =
                                self.scene.entities.iter_mut().find(|e| e.name == sel)
                            {
                                if let Some(sp) = ent.components.sprite.as_mut() {
                                    sp.texture = name.clone();
                                    applied = true;
                                }
                            }
                            if !applied {
                                if add_component_sprite(
                                    &mut self.scene,
                                    &sel,
                                    &name,
                                    [32.0, 32.0],
                                )
                                .is_ok()
                                {
                                    applied = true;
                                }
                            }
                            if applied {
                                self.sync_baseline();
                                self.mark_dirty();
                            } else {
                                let _ = self.undo.undo(&mut self.scene);
                            }
                        }
                    }
                }
            });
        });

        egui::SidePanel::left("hierarchy")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Hierarchy");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_entity_name);
                    if ui.button("+").clicked() {
                        let name = unique_entity_name(&self.scene, &self.new_entity_name);
                        self.push_undo();
                        if add_entity(
                            &mut self.scene,
                            &name,
                            &MutateOpts {
                                x: Some(320.0),
                                y: Some(240.0),
                                ..Default::default()
                            },
                        )
                        .is_ok()
                        {
                            self.select(Some(name.clone()));
                            self.new_entity_name = unique_entity_name(&self.scene, "NewEntity");
                            self.sync_baseline();
                            self.mark_dirty();
                        } else {
                            let _ = self.undo.undo(&mut self.scene);
                        }
                    }
                });
                ui.separator();
                let names: Vec<_> = self.scene.entities.iter().map(|e| e.name.clone()).collect();
                let mut to_remove = None;
                let mut to_duplicate = None;
                for name in &names {
                    ui.horizontal(|ui| {
                        let selected = self.selected.as_deref() == Some(name.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.select(Some(name.clone()));
                        }
                        if ui.small_button("⧉").on_hover_text("Duplicate").clicked() {
                            to_duplicate = Some(name.clone());
                        }
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(name.clone());
                        }
                    });
                }
                if let Some(name) = to_duplicate {
                    self.select(Some(name));
                    self.do_duplicate();
                }
                if let Some(name) = to_remove {
                    self.push_undo();
                    let _ = remove_entity(&mut self.scene, &name);
                    if self.selected.as_deref() == Some(name.as_str()) {
                        self.select(None);
                    }
                    self.sync_baseline();
                    self.mark_dirty();
                }
            });

        egui::SidePanel::right("inspector")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                let Some(sel) = self.selected.clone() else {
                    ui.label("Select an entity");
                    return;
                };
                let mut dirty = false;
                let mut add_sprite = false;
                let mut add_disc = false;
                let mut remove_sprite = false;
                let mut remove_disc = false;
                let mut rename_committed = false;

                ui.horizontal(|ui| {
                    ui.label("name");
                    let resp = ui.text_edit_singleline(&mut self.rename_draft);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        rename_committed = true;
                    } else if resp.lost_focus() {
                        rename_committed = true;
                    }
                });
                if rename_committed {
                    self.commit_rename();
                }

                if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                    ui.separator();
                    ui.label("Transform");
                    let mut changed = false;
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.translation[0], 0.0..=640.0)
                                .text("x"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.translation[1], 0.0..=480.0)
                                .text("y"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.scale[0], 0.1..=8.0).text("scale x"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.scale[1], 0.1..=8.0).text("scale y"),
                        )
                        .changed();
                    if changed {
                        dirty = true;
                    }

                    ui.separator();
                    ui.label("Components");
                    if let Some(sp) = ent.components.sprite.as_mut() {
                        ui.group(|ui| {
                            ui.label("Sprite");
                            ui.label(format!("texture: {}", sp.texture));
                            dirty |= ui
                                .add(egui::Slider::new(&mut sp.size[0], 1.0..=256.0).text("w"))
                                .changed();
                            dirty |= ui
                                .add(egui::Slider::new(&mut sp.size[1], 1.0..=256.0).text("h"))
                                .changed();
                            dirty |= ui
                                .add(egui::Slider::new(&mut sp.z, -10.0..=10.0).text("z"))
                                .changed();
                            if ui.button("Remove Sprite").clicked() {
                                remove_sprite = true;
                            }
                        });
                    } else if ui.button("Add Sprite").clicked() {
                        add_sprite = true;
                    }

                    if let Some(d) = ent.components.disc.as_mut() {
                        ui.group(|ui| {
                            ui.label("Disc");
                            dirty |= ui
                                .add(egui::Slider::new(&mut d.radius, 1.0..=200.0).text("radius"))
                                .changed();
                            dirty |= ui
                                .add(egui::Slider::new(&mut d.z, -10.0..=10.0).text("z"))
                                .changed();
                            if ui.button("Remove Disc").clicked() {
                                remove_disc = true;
                            }
                        });
                    } else if ui.button("Add Disc").clicked() {
                        add_disc = true;
                    }
                }

                if dirty {
                    self.begin_inspector_gesture();
                    self.mark_dirty();
                }
                if remove_sprite {
                    self.push_undo();
                    if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                        ent.components.sprite = None;
                        self.sync_baseline();
                        self.mark_dirty();
                    }
                }
                if remove_disc {
                    self.push_undo();
                    if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                        ent.components.disc = None;
                        self.sync_baseline();
                        self.mark_dirty();
                    }
                }
                if add_sprite {
                    let tex = self
                        .asset_names
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "missing".into());
                    self.push_undo();
                    let _ = add_component_sprite(&mut self.scene, &sel, &tex, [32.0, 32.0]);
                    self.sync_baseline();
                    self.mark_dirty();
                }
                if add_disc {
                    self.push_undo();
                    let _ = add_component_disc(&mut self.scene, &sel, 36.0, [72, 210, 160, 255]);
                    self.sync_baseline();
                    self.mark_dirty();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Scene");
            let mut draw = DrawList::new();
            render_world(&self.world, &mut draw, self.scene.clear_rgba());
            flush_with_atlas(&draw, &mut self.fb, Some(&self.atlas));

            let color_image = egui::ColorImage::from_rgb([VIEW_W, VIEW_H], &fb_to_rgb(&self.fb));
            let tex = self.texture_handle.get_or_insert_with(|| {
                ctx.load_texture("viewport", color_image.clone(), Default::default())
            });
            tex.set(color_image, Default::default());

            let max = ui.available_size();
            let scale = (max.x / VIEW_W as f32)
                .min(max.y / VIEW_H as f32)
                .min(1.0);
            let size = egui::vec2(VIEW_W as f32 * scale, VIEW_H as f32 * scale);
            let image = egui::Image::new((tex.id(), size)).sense(egui::Sense::click_and_drag());
            let response = ui.add(image);
            let rect = response.rect;

            paint_selection_outline(ui, rect, &self.scene, self.selected.as_deref());

            self.handle_viewport_input(&response, rect);
        });

        ctx.request_repaint();
    }
}

impl EditorApp {
    fn handle_viewport_input(&mut self, response: &egui::Response, rect: egui::Rect) {
        let to_scene = |pos: egui::Pos2| -> Option<[f32; 2]> {
            pointer_to_scene(
                [pos.x, pos.y],
                [rect.min.x, rect.min.y],
                [rect.width(), rect.height()],
                VIEW_W as f32,
                VIEW_H as f32,
            )
        };

        let pick_at = |app: &Self, pos: [f32; 2]| -> Option<(String, [f32; 2])> {
            let name = pick_entity_at(&app.scene, pos[0], pos[1])?;
            let grab_offset = app
                .scene
                .entities
                .iter()
                .find(|e| e.name == name)
                .map(|e| {
                    [
                        pos[0] - e.transform.translation[0],
                        pos[1] - e.transform.translation[1],
                    ]
                })
                .unwrap_or([0.0, 0.0]);
            Some((name, grab_offset))
        };

        // Click (no drag): select or clear.
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                match pick_at(self, pos) {
                    Some((name, _)) => self.select(Some(name)),
                    None => self.select(None),
                }
            }
        }

        // Drag start: select hit entity and begin translate.
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                match pick_at(self, pos) {
                    Some((name, grab_offset)) => {
                        self.select(Some(name.clone()));
                        self.push_undo();
                        self.viewport_drag = Some(ViewportDrag {
                            entity: name,
                            grab_offset,
                        });
                    }
                    None => {
                        self.select(None);
                        self.viewport_drag = None;
                    }
                }
            }
        }

        if let Some(drag) = self.viewport_drag.clone() {
            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos().and_then(to_scene) {
                    let x = pos[0] - drag.grab_offset[0];
                    let y = pos[1] - drag.grab_offset[1];
                    if set_entity_transform(&mut self.scene, &drag.entity, Some(x), Some(y)).is_ok()
                    {
                        self.mark_dirty();
                    }
                }
            }
        }

        if response.drag_stopped() {
            if self.viewport_drag.is_some() {
                self.sync_baseline();
            }
            self.viewport_drag = None;
        }
    }
}

fn paint_selection_outline(
    ui: &egui::Ui,
    image_rect: egui::Rect,
    scene: &Scene,
    selected: Option<&str>,
) {
    let Some(name) = selected else {
        return;
    };
    let Some(ent) = scene.entities.iter().find(|e| e.name == name) else {
        return;
    };

    let to_screen = |sx: f32, sy: f32| -> egui::Pos2 {
        egui::pos2(
            image_rect.min.x + sx / VIEW_W as f32 * image_rect.width(),
            image_rect.min.y + sy / VIEW_H as f32 * image_rect.height(),
        )
    };
    let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 200, 64));
    let painter = ui.painter();

    if let Some(sp) = &ent.components.sprite {
        let hw = sp.size[0] * ent.transform.scale[0] * 0.5;
        let hh = sp.size[1] * ent.transform.scale[1] * 0.5;
        let cx = ent.transform.translation[0];
        let cy = ent.transform.translation[1];
        let r = egui::Rect::from_min_max(to_screen(cx - hw, cy - hh), to_screen(cx + hw, cy + hh));
        painter.rect_stroke(r, 0.0, stroke);
    }
    if let Some(d) = &ent.components.disc {
        let cx = ent.transform.translation[0];
        let cy = ent.transform.translation[1];
        let r_scene = d.radius * ent.transform.scale[0].max(ent.transform.scale[1]);
        let center = to_screen(cx, cy);
        let radius_px = r_scene / VIEW_W as f32 * image_rect.width();
        painter.circle_stroke(center, radius_px, stroke);
    }
}

fn fb_to_rgb(fb: &Framebuffer) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(fb.pixels.len() * 3);
    for p in &fb.pixels {
        rgb.push(((p >> 16) & 0xff) as u8);
        rgb.push(((p >> 8) & 0xff) as u8);
        rgb.push((p & 0xff) as u8);
    }
    rgb
}

fn find_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("runtime/wii").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("not inside a wiimaker workspace");
        }
    }
}
