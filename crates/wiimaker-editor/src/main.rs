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
    add_component_disc, add_component_sprite, add_entity, diagnose, find_game_dir, hydrate_lenient,
    load_project, load_scene, remove_entity, render_world, save_scene, GameProject, MutateOpts,
    Scene,
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
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
            self.save();
        }

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
                            if let Some(ent) =
                                self.scene.entities.iter_mut().find(|e| e.name == sel)
                            {
                                if let Some(sp) = ent.components.sprite.as_mut() {
                                    sp.texture = name.clone();
                                    self.mark_dirty();
                                } else if add_component_sprite(
                                    &mut self.scene,
                                    &sel,
                                    &name,
                                    [32.0, 32.0],
                                )
                                .is_ok()
                                {
                                    self.mark_dirty();
                                }
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
                        let name = self.new_entity_name.clone();
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
                            self.selected = Some(name);
                            self.mark_dirty();
                        }
                    }
                });
                ui.separator();
                let names: Vec<_> = self.scene.entities.iter().map(|e| e.name.clone()).collect();
                let mut to_remove = None;
                for name in &names {
                    ui.horizontal(|ui| {
                        let selected = self.selected.as_deref() == Some(name.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected = Some(name.clone());
                        }
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(name.clone());
                        }
                    });
                }
                if let Some(name) = to_remove {
                    let _ = remove_entity(&mut self.scene, &name);
                    if self.selected.as_deref() == Some(name.as_str()) {
                        self.selected = None;
                    }
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

                if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                    ui.label(format!("name: {}", ent.name));
                    ui.separator();
                    ui.label("Transform");
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.translation[0], 0.0..=640.0)
                                .text("x"),
                        )
                        .changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.translation[1], 0.0..=480.0)
                                .text("y"),
                        )
                        .changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.scale[0], 0.1..=8.0).text("scale x"),
                        )
                        .changed();
                    dirty |= ui
                        .add(
                            egui::Slider::new(&mut ent.transform.scale[1], 0.1..=8.0).text("scale y"),
                        )
                        .changed();

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

                if remove_sprite {
                    if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                        ent.components.sprite = None;
                        dirty = true;
                    }
                }
                if remove_disc {
                    if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == sel) {
                        ent.components.disc = None;
                        dirty = true;
                    }
                }
                if add_sprite {
                    let tex = self
                        .asset_names
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "missing".into());
                    let _ = add_component_sprite(&mut self.scene, &sel, &tex, [32.0, 32.0]);
                    dirty = true;
                }
                if add_disc {
                    let _ = add_component_disc(&mut self.scene, &sel, 36.0, [72, 210, 160, 255]);
                    dirty = true;
                }
                if dirty {
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
            ui.image((tex.id(), size));
        });

        ctx.request_repaint();
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
