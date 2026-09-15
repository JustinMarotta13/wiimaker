use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use eframe::egui;
use wiimaker_assets::{list_wav_clips, AnimClipCatalog, SpriteCatalog, WPack};
use wiimaker_core::input::Input;
use wiimaker_core::world::World;
use wiimaker_host::{Framebuffer, HostAudio, PlayOutcome, TextureAtlas};
pub(crate) use wiimaker_play::PlayKind;
use wiimaker_play::{apply_pad_keys, PadKeys, PlaySession, PlayStart};
use wiimaker_scene::{
    add_build_scene, add_component_sprite, create_named_scene, diagnose, duplicate_entity,
    find_game_dir, hydrate_lenient_with_sorting_layers, insert_entity_clone, list_scenes,
    load_editor_prefs, load_project, load_scene, remove_build_scene, rename_entity,
    save_editor_prefs, save_scene, set_default_scene, EditorPrefs, EntityData, GameProject, Scene,
    Severity, TranslateHandle, UndoStack,
};

use crate::dock::{self, EditorTab};
use crate::sprite_editor::{self, SpriteEditorState};
use crate::theme;
use crate::ui_project;
use crate::viewport::{VIEW_H, VIEW_W};

/// Active viewport transform drag.
#[derive(Clone)]
pub(crate) struct ViewportDrag {
    pub(crate) entity: String,
    /// `pointer_scene - translation` at drag start so the entity doesn't jump.
    pub(crate) grab_offset: [f32; 2],
    /// Primary entity world XY at drag start.
    pub(crate) primary_start: [f32; 2],
    /// Other selected entities' world XY at drag start (for bulk translate).
    pub(crate) others_start: Vec<(String, [f32; 2])>,
    /// Scale tool: local XY scale at drag start (primary).
    pub(crate) scale_start: [f32; 2],
    /// Scale tool: pointer distance from entity at drag start.
    pub(crate) dist_start: f32,
    /// Rotate tool: atan2 angle at drag start + entity Z angle at start.
    pub(crate) angle_start: f32,
    pub(crate) rot_z_start: f32,
    /// Move tool: axis constraint (`Free` for body drag / XY square).
    pub(crate) translate_handle: TranslateHandle,
}

#[derive(Clone)]
pub(crate) struct TilePaintDrag {
    pub(crate) entity: String,
    pub(crate) last: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditTool {
    Translate,
    Scale,
    Rotate,
    Paint,
    Erase,
    Pick,
    Hand,
}

impl EditTool {
    pub(crate) fn is_tile_tool(self) -> bool {
        matches!(self, Self::Paint | Self::Erase | Self::Pick)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayMode {
    Edit,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CenterTab {
    Scene,
    Game,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsoleLevel {
    Info,
    Warn,
    Error,
}

#[derive(Clone)]
pub(crate) struct ConsoleLine {
    pub(crate) level: ConsoleLevel,
    pub(crate) text: String,
}

pub(crate) struct EditorApp {
    pub(crate) root: PathBuf,
    pub(crate) game_dir: PathBuf,
    pub(crate) project: GameProject,
    pub(crate) scene: Scene,
    pub(crate) scene_path: PathBuf,
    pub(crate) dirty: bool,
    /// Selected entity names (Hierarchy / viewport). Last entry is the primary (Inspector).
    pub(crate) selected: Vec<String>,
    /// Project file shown in Inspector (relative to game_dir).
    pub(crate) selected_file: Option<PathBuf>,
    pub(crate) atlas: TextureAtlas,
    pub(crate) world: World,
    pub(crate) fb: Framebuffer,
    pub(crate) texture_handle: Option<egui::TextureHandle>,
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) new_entity_name: String,
    pub(crate) hierarchy_filter: String,
    pub(crate) asset_names: Vec<String>,
    /// Cached relative paths under the game dir for the Project explorer.
    pub(crate) project_entries: Vec<ProjectEntry>,
    pub(crate) catalog: SpriteCatalog,
    pub(crate) anim_catalog: AnimClipCatalog,
    pub(crate) wav_names: Vec<String>,
    pub(crate) host_audio: HostAudio,
    pub(crate) sprite_editor: Option<SpriteEditorState>,
    pub(crate) sprite_editor_open: bool,
    /// Pending stem to open in Sprite Editor (set from context menu).
    pub(crate) open_sprite_editor_stem: Option<String>,
    pub(crate) undo: UndoStack,
    /// Scene snapshot before the current inspector drag gesture.
    pub(crate) undo_baseline: Scene,
    pub(crate) inspector_gesture: bool,
    pub(crate) clipboard: Option<EntityData>,
    pub(crate) rename_draft: String,
    pub(crate) viewport_drag: Option<ViewportDrag>,
    /// Scene paths relative to `game_dir`.
    pub(crate) scene_rels: Vec<PathBuf>,
    /// Pending open when current scene is dirty (absolute path).
    pub(crate) pending_open: Option<PathBuf>,
    pub(crate) new_scene_name: String,
    /// File → Build Settings… dialog.
    pub(crate) show_build_settings: bool,
    /// Selected row in the Scenes in Build list (relative path).
    pub(crate) build_settings_pick: Option<String>,
    /// Combo draft for adding a discovered scene to the build list.
    pub(crate) build_add_draft: String,
    /// Draft name for a new Sorting Layer (game.toml inspector).
    pub(crate) sorting_layer_draft: String,
    /// Selected Sorting Layer row (for rename).
    pub(crate) sorting_layer_pick: String,
    /// Unused: dock splitters own leaf size now.
    #[allow(dead_code)]
    pub(crate) project_panel_height: f32,
    #[allow(dead_code)]
    pub(crate) hierarchy_width: f32,
    #[allow(dead_code)]
    pub(crate) inspector_width: f32,
    /// Scene/Game chrome — `<game>/.wiimaker/prefs.toml`.
    pub(crate) prefs: EditorPrefs,
    pub(crate) prefs_dirty: bool,
    pub(crate) edit_tool: EditTool,
    pub(crate) play_mode: PlayMode,
    pub(crate) play_session: Option<PlaySession>,
    pub(crate) tile_brush_id: u16,
    pub(crate) tile_brush_solid: bool,
    pub(crate) tile_paint: Option<TilePaintDrag>,
    pub(crate) dock_state: egui_dock::DockState<EditorTab>,
    pub(crate) pending_focus: Option<EditorTab>,
    pub(crate) console: Vec<ConsoleLine>,
    pub(crate) last_play_hit: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ProjectEntry {
    /// Path relative to `game_dir`.
    pub(crate) rel: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) depth: u32,
}

impl EditorApp {
    pub(crate) fn open(root: &std::path::Path, game: &str) -> Result<Self> {
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
            selected: Vec::new(),
            selected_file: None,
            atlas: TextureAtlas::empty(),
            world: World::new(),
            fb: Framebuffer::new(VIEW_W, VIEW_H),
            texture_handle: None,
            status: String::new(),
            new_entity_name: "NewEntity".into(),
            hierarchy_filter: String::new(),
            asset_names: Vec::new(),
            project_entries: Vec::new(),
            catalog: SpriteCatalog::empty(),
            anim_catalog: AnimClipCatalog::empty(),
            wav_names: Vec::new(),
            host_audio: HostAudio::new(),
            sprite_editor: None,
            sprite_editor_open: false,
            open_sprite_editor_stem: None,
            undo: UndoStack::with_default_depth(),
            inspector_gesture: false,
            clipboard: None,
            rename_draft: String::new(),
            viewport_drag: None,
            scene_rels: Vec::new(),
            pending_open: None,
            new_scene_name: "menu".into(),
            show_build_settings: false,
            build_settings_pick: None,
            build_add_draft: String::new(),
            sorting_layer_draft: String::new(),
            sorting_layer_pick: String::new(),
            project_panel_height: 200.0,
            hierarchy_width: 240.0,
            inspector_width: 300.0,
            prefs: EditorPrefs::default(),
            prefs_dirty: false,
            edit_tool: EditTool::Translate,
            play_mode: PlayMode::Edit,
            play_session: None,
            tile_brush_id: 1,
            tile_brush_solid: true,
            tile_paint: None,
            dock_state: dock::default_unity_layout(),
            pending_focus: None,
            console: Vec::new(),
            last_play_hit: None,
        };
        app.reload_assets()?;
        app.refresh_scenes();
        app.refresh_project_tree();
        app.rehydrate();
        match load_editor_prefs(&app.game_dir) {
            Ok(p) => app.prefs = p,
            Err(e) => app.status = format!("prefs: {e}"),
        }
        app.status = format!("opened {}", app.project.name);
        // Agent / screenshot helper: `WIIMAKER_EDITOR_SELECT=Player` selects an entity on open.
        if let Ok(name) = std::env::var("WIIMAKER_EDITOR_SELECT") {
            let name = name.trim().to_string();
            if !name.is_empty() && app.scene.find_entity(&name).is_some() {
                app.select(Some(name));
            }
            // More Inspector room for screenshot / agent OCR.
            app.project_panel_height = 140.0;
        }
        if let Ok(rel) = std::env::var("WIIMAKER_EDITOR_SELECT_FILE") {
            let rel = rel.trim();
            if !rel.is_empty() {
                app.select_file(Some(PathBuf::from(rel)));
            }
        }
        if std::env::var("WIIMAKER_EDITOR_PLAY").ok().as_deref() == Some("1") {
            app.play();
        }
        Ok(app)
    }

    pub(crate) fn refresh_scenes(&mut self) {
        match list_scenes(&self.game_dir) {
            Ok(rels) => self.scene_rels = rels,
            Err(e) => self.status = format!("list scenes failed: {e}"),
        }
    }

    /// Flatten `assets/`, `scenes/`, and top-level `game.toml` for the Project explorer.
    pub(crate) fn refresh_project_tree(&mut self) {
        let mut entries = Vec::new();
        let game_toml = PathBuf::from("game.toml");
        if self.game_dir.join(&game_toml).is_file() {
            entries.push(ProjectEntry {
                rel: game_toml,
                is_dir: false,
                depth: 0,
            });
        }
        for folder in ["assets", "scenes"] {
            let abs = self.game_dir.join(folder);
            if !abs.is_dir() {
                continue;
            }
            entries.push(ProjectEntry {
                rel: PathBuf::from(folder),
                is_dir: true,
                depth: 0,
            });
            ui_project::push_dir_entries(&abs, PathBuf::from(folder), 1, &mut entries);
        }
        self.project_entries = entries;
    }

    /// Request opening a scene; if dirty, show confirm modal first.
    pub(crate) fn request_open_scene(&mut self, abs_path: PathBuf) {
        if abs_path == self.scene_path {
            return;
        }
        if self.dirty {
            self.pending_open = Some(abs_path);
        } else {
            self.open_scene_at(abs_path);
        }
    }

    pub(crate) fn open_scene_at(&mut self, abs_path: PathBuf) {
        match load_scene(&abs_path) {
            Ok(scene) => {
                self.scene = scene;
                self.scene_path = abs_path;
                self.dirty = false;
                self.undo.clear();
                self.viewport_drag = None;
                self.inspector_gesture = false;
                self.select(None);
                self.select_file(None);
                self.pending_open = None;
                self.sync_baseline();
                self.rehydrate();
                self.status = format!("opened {}", self.scene_path.display());
            }
            Err(e) => {
                self.pending_open = None;
                self.status = format!("open failed: {e}");
            }
        }
    }

    pub(crate) fn log_line(&mut self, level: ConsoleLevel, text: impl Into<String>) {
        let text = text.into();
        self.status = text.clone();
        self.console_push(level, text);
    }

    pub(crate) fn console_push(&mut self, level: ConsoleLevel, text: impl Into<String>) {
        self.console.push(ConsoleLine {
            level,
            text: text.into(),
        });
        if self.console.len() > 200 {
            let drain = self.console.len() - 200;
            self.console.drain(0..drain);
        }
    }

    pub(crate) fn create_new_scene(&mut self) {
        match create_named_scene(&self.game_dir, self.new_scene_name.trim()) {
            Ok(rel) => {
                let abs = self.game_dir.join(&rel);
                self.refresh_scenes();
                self.refresh_project_tree();
                if self.dirty {
                    self.pending_open = Some(abs);
                    self.log_line(
                        ConsoleLevel::Info,
                        format!("created {}; save current scene to open it", rel.display()),
                    );
                } else {
                    self.open_scene_at(abs);
                    self.log_line(ConsoleLevel::Info, format!("created {}", rel.display()));
                }
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("new scene: {e}")),
        }
    }

    pub(crate) fn set_as_default_scene(&mut self) {
        let Ok(rel) = self.scene_path.strip_prefix(&self.game_dir) else {
            self.log_line(ConsoleLevel::Error, "set default: scene outside game dir");
            return;
        };
        let key = rel.to_string_lossy().replace('\\', "/");
        match set_default_scene(&self.game_dir, &key) {
            Ok(rel) => {
                self.project.default_scene = rel.to_string_lossy().replace('\\', "/");
                self.log_line(
                    ConsoleLevel::Info,
                    format!("default scene → {}", self.project.default_scene),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("save project failed: {e}")),
        }
    }

    pub(crate) fn reload_project(&mut self) {
        match load_project(&self.game_dir) {
            Ok(p) => self.project = p,
            Err(e) => self.log_line(ConsoleLevel::Error, format!("reload game.toml: {e}")),
        }
    }

    pub(crate) fn set_default_scene_rel(&mut self, rel: &str) {
        match set_default_scene(&self.game_dir, rel) {
            Ok(path) => {
                self.reload_project();
                self.log_line(
                    ConsoleLevel::Info,
                    format!("default scene → {}", path.display()),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("set default: {e}")),
        }
    }

    pub(crate) fn add_to_build_list(&mut self, scene: &str) {
        match add_build_scene(&self.game_dir, scene) {
            Ok(rel) => {
                self.reload_project();
                self.build_settings_pick = Some(rel.to_string_lossy().replace('\\', "/"));
                self.log_line(
                    ConsoleLevel::Info,
                    format!("build scenes += {}", rel.display()),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("build add: {e}")),
        }
    }

    pub(crate) fn remove_from_build_list(&mut self, scene: &str) {
        match remove_build_scene(&self.game_dir, scene) {
            Ok(rel) => {
                self.reload_project();
                if self.build_settings_pick.as_deref()
                    == Some(&rel.to_string_lossy().replace('\\', "/"))
                {
                    self.build_settings_pick = None;
                }
                self.log_line(
                    ConsoleLevel::Info,
                    format!("build scenes -= {}", rel.display()),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("build remove: {e}")),
        }
    }

    pub(crate) fn add_project_sorting_layer(&mut self, name: &str) {
        match wiimaker_scene::add_sorting_layer(&self.game_dir, name, None) {
            Ok(_) => {
                self.reload_project();
                self.sorting_layer_draft.clear();
                self.rehydrate();
                self.log_line(ConsoleLevel::Info, format!("sorting layers += {name}"));
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("sorting layer add: {e}")),
        }
    }

    pub(crate) fn rename_project_sorting_layer(&mut self, from: &str, to: &str) {
        match wiimaker_scene::rename_sorting_layer(&self.game_dir, from, to, Some(&self.scene_path))
        {
            Ok(_) => {
                let n = wiimaker_scene::remap_scene_sorting_layer(&mut self.scene, from, to);
                if n > 0 {
                    self.dirty = true;
                }
                self.reload_project();
                self.rehydrate();
                self.log_line(
                    ConsoleLevel::Info,
                    format!("sorting layer '{from}' → '{to}'"),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("sorting layer rename: {e}")),
        }
    }

    pub(crate) fn move_project_sorting_layer(&mut self, name: &str, index: usize) {
        match wiimaker_scene::move_sorting_layer(&self.game_dir, name, index) {
            Ok(_) => {
                self.reload_project();
                self.rehydrate();
                self.log_line(
                    ConsoleLevel::Info,
                    format!("sorting layer '{name}' → {index}"),
                );
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("sorting layer move: {e}")),
        }
    }

    pub(crate) fn remove_project_sorting_layer(&mut self, name: &str) {
        match wiimaker_scene::remove_sorting_layer(&self.game_dir, name, Some(&self.scene_path)) {
            Ok(_) => {
                let n = wiimaker_scene::remap_scene_sorting_layer(
                    &mut self.scene,
                    name,
                    wiimaker_core::DEFAULT_SORTING_LAYER,
                );
                if n > 0 {
                    self.dirty = true;
                }
                self.reload_project();
                self.rehydrate();
                self.log_line(ConsoleLevel::Info, format!("sorting layers -= {name}"));
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("sorting layer remove: {e}")),
        }
    }

    pub(crate) fn open_build_scene_rel(&mut self, rel: &str) {
        self.request_open_scene(self.game_dir.join(rel));
    }

    pub(crate) fn reload_assets(&mut self) -> Result<()> {
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

        self.catalog = SpriteCatalog::load_dir(&assets, |stem| self.atlas.size_of(stem))?;
        self.anim_catalog = AnimClipCatalog::load_dir(&assets)?;
        self.wav_names = list_wav_clips(&assets).unwrap_or_default();
        self.refresh_project_tree();
        Ok(())
    }

    pub(crate) fn rehydrate(&mut self) {
        let layers = self.project.effective_sorting_layers();
        self.world = hydrate_lenient_with_sorting_layers(
            &self.scene,
            self.atlas.map(),
            Some(&self.catalog),
            Some(&self.anim_catalog),
            Some(&layers),
        );
    }

    pub(crate) fn open_sprite_editor(&mut self, stem: &str, ctx: &egui::Context) {
        match SpriteEditorState::open(&self.project.assets_path(&self.game_dir), stem, ctx) {
            Ok(state) => {
                self.sprite_editor = Some(state);
                self.sprite_editor_open = true;
                self.status = format!("sprite editor · {stem}");
            }
            Err(e) => self.status = format!("sprite editor failed: {e}"),
        }
    }

    pub(crate) fn refresh_catalog_after_slice(&mut self) {
        if let Err(e) = self.reload_assets() {
            self.status = format!("refresh failed: {e}");
        } else {
            self.rehydrate();
            self.status = "sprites updated".into();
        }
    }

    pub(crate) fn assign_sprite_name(&mut self, entity: &str, sprite_id: &str) {
        self.push_undo();
        let size = self
            .catalog
            .lookup(sprite_id)
            .map(|r| r.pixel_size)
            .unwrap_or([32.0, 32.0]);
        let mut applied = false;
        if let Some(ent) = self.scene.entities.iter_mut().find(|e| e.name == entity) {
            if let Some(sp) = ent.components.sprite.as_mut() {
                sp.texture = sprite_id.to_string();
                sp.size = size;
                applied = true;
            }
        }
        if !applied {
            if add_component_sprite(&mut self.scene, entity, sprite_id, size).is_ok() {
                applied = true;
            }
        }
        if applied {
            self.sync_baseline();
            self.mark_dirty();
            self.status = format!("sprite → {sprite_id}");
        } else {
            let _ = self.undo.undo(&mut self.scene);
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.rehydrate();
    }

    pub(crate) fn persist_prefs(&mut self) {
        self.prefs_dirty = true;
    }

    pub(crate) fn flush_prefs(&mut self) {
        if !self.prefs_dirty {
            return;
        }
        match save_editor_prefs(&self.game_dir, &self.prefs) {
            Ok(()) => self.prefs_dirty = false,
            Err(e) => self.status = format!("prefs save failed: {e}"),
        }
    }

    /// Nudge all selected entities by `(dx, dy)` in world space.
    pub(crate) fn nudge_selected(&mut self, dx: f32, dy: f32) {
        if self.selected.is_empty() {
            return;
        }
        let names = self.selected.clone();
        self.push_undo();
        let mut ok = true;
        for name in &names {
            let Some(world) = self.scene.world_transform(name) else {
                ok = false;
                break;
            };
            if wiimaker_scene::set_entity_world_xy(
                &mut self.scene,
                name,
                world.translation[0] + dx,
                world.translation[1] + dy,
            )
            .is_err()
            {
                ok = false;
                break;
            }
        }
        if ok {
            self.sync_baseline();
            self.mark_dirty();
        } else {
            let _ = self.undo.undo(&mut self.scene);
        }
    }

    pub(crate) fn primary_selected(&self) -> Option<&str> {
        self.selected.last().map(|s| s.as_str())
    }

    pub(crate) fn is_selected(&self, name: &str) -> bool {
        self.selected.iter().any(|s| s == name)
    }

    pub(crate) fn sync_baseline(&mut self) {
        self.undo_baseline = self.scene.clone();
        self.inspector_gesture = false;
    }

    /// Push undo for a discrete mutation (buttons, rename, add/remove, etc.).
    pub(crate) fn push_undo(&mut self) {
        self.undo.push(&self.scene);
        self.inspector_gesture = false;
    }

    /// First change of an inspector slider gesture: push pre-edit baseline once.
    pub(crate) fn begin_inspector_gesture(&mut self) {
        if !self.inspector_gesture {
            self.undo.push(&self.undo_baseline);
            self.inspector_gesture = true;
        }
    }

    pub(crate) fn end_inspector_gesture_if_released(&mut self, ctx: &egui::Context) {
        if self.inspector_gesture && !ctx.input(|i| i.pointer.any_down()) {
            self.sync_baseline();
        }
    }

    pub(crate) fn select(&mut self, name: Option<String>) {
        self.selected = name.into_iter().collect();
        self.rename_draft = self.primary_selected().unwrap_or("").to_string();
        if !self.selected.is_empty() {
            self.selected_file = None;
        }
        self.announce_selection();
    }

    /// Cmd/Ctrl-click: toggle `name` in the multi-selection; becomes primary if added.
    pub(crate) fn select_toggle(&mut self, name: String) {
        self.selected_file = None;
        if let Some(i) = self.selected.iter().position(|s| *s == name) {
            self.selected.remove(i);
        } else {
            self.selected.push(name);
        }
        self.rename_draft = self.primary_selected().unwrap_or("").to_string();
        self.announce_selection();
    }

    /// Status line for OCR / agents: `selected: A` or `selected 2: A, B`.
    pub(crate) fn announce_selection(&mut self) {
        match self.selected.len() {
            0 => {}
            1 => {
                self.status = format!("selected: {}", self.selected[0]);
            }
            n => {
                self.status = format!("selected {n}: {}", self.selected.join(", "));
            }
        }
    }

    pub(crate) fn select_file(&mut self, rel: Option<PathBuf>) {
        self.selected_file = rel;
        if self.selected_file.is_some() {
            self.selected.clear();
            self.rename_draft.clear();
        }
    }

    pub(crate) fn prune_selection(&mut self) {
        self.selected
            .retain(|s| self.scene.entities.iter().any(|e| e.name == *s));
        self.rename_draft = self.primary_selected().unwrap_or("").to_string();
        if let Some(rel) = self.selected_file.clone() {
            if !self.game_dir.join(&rel).exists() {
                self.select_file(None);
            }
        }
    }

    pub(crate) fn do_undo(&mut self) {
        if self.undo.undo(&mut self.scene) {
            self.prune_selection();
            self.sync_baseline();
            self.mark_dirty();
            self.status = "undo".into();
        }
    }

    pub(crate) fn do_redo(&mut self) {
        if self.undo.redo(&mut self.scene) {
            self.prune_selection();
            self.sync_baseline();
            self.mark_dirty();
            self.status = "redo".into();
        }
    }

    pub(crate) fn do_duplicate(&mut self) {
        let Some(sel) = self.primary_selected().map(|s| s.to_string()) else {
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

    pub(crate) fn do_copy(&mut self) {
        let Some(sel) = self.primary_selected().map(|s| s.to_string()) else {
            return;
        };
        if let Some(ent) = self.scene.entities.iter().find(|e| e.name == sel) {
            self.clipboard = Some(ent.clone());
            self.status = format!("copied {sel}");
        }
    }

    pub(crate) fn do_paste(&mut self) {
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

    pub(crate) fn commit_rename(&mut self) {
        let Some(old) = self.primary_selected().map(|s| s.to_string()) else {
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

    pub(crate) fn save(&mut self) {
        match save_scene(&self.scene_path, &self.scene) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("saved {}", self.scene_path.display());
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    pub(crate) fn cook(&mut self) {
        let assets = self.project.assets_path(&self.game_dir);
        let out = self.project.wpack_path(&self.game_dir);
        let mut pack = WPack::new();
        match pack.cook_dir(&assets) {
            Ok(warnings) => {
                if let Err(e) = pack.write_to(&out) {
                    self.status = format!("prepare write failed: {e}");
                    return;
                }
                self.atlas = TextureAtlas::from_wpack(&pack);
                let _ = self.reload_assets();
                self.rehydrate();
                let warn = if warnings.is_empty() {
                    String::new()
                } else {
                    format!(" ({} warnings)", warnings.len())
                };
                self.status = format!("assets ready{warn}");
            }
            Err(e) => self.status = format!("prepare failed: {e}"),
        }
    }

    /// Ensure `.wpack` exists before host play / Wii build.
    fn prepare_assets(&mut self) {
        self.status = "Preparing assets…".into();
        self.cook();
    }

    /// Copy PNG/WAV into `assets/` (cook PNG), TXT maze files into `assets/` (Project import).
    pub(crate) fn import_asset_paths(&mut self, paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }
        let assets = self.project.assets_path(&self.game_dir);
        if let Err(e) = std::fs::create_dir_all(&assets) {
            self.status = format!("import failed: {e}");
            return;
        }
        let mut imported = Vec::new();
        let mut need_cook = false;
        for src in paths {
            let Some(ext) = src.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let ext_l = ext.to_ascii_lowercase();
            if ext_l != "png" && ext_l != "wav" && ext_l != "txt" {
                continue;
            }
            let stem = src
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("tex")
                .to_string();
            let dest = assets.join(format!("{stem}.{ext_l}"));
            match std::fs::copy(src, &dest) {
                Ok(_) => {
                    imported.push(format!("{stem}.{ext_l}"));
                    if ext_l == "png" {
                        need_cook = true;
                    }
                }
                Err(e) => {
                    self.status = format!("import {} failed: {e}", src.display());
                    return;
                }
            }
        }
        if imported.is_empty() {
            self.status = "drop PNG, WAV, or TXT files to import".into();
            return;
        }
        if need_cook {
            self.cook();
        } else if let Err(e) = self.reload_assets() {
            self.status = format!("refresh failed: {e}");
            return;
        }
        self.status = format!("imported {} · assets ready", imported.join(", "));
    }

    /// Stamp a UTF-8 maze file into the named Tilemap (CLI twin: `tilemap from-ascii`).
    pub(crate) fn import_ascii_tilemap(&mut self, entity: &str, path: &std::path::Path) {
        use wiimaker_scene::{add_entity, tilemap_from_ascii_path, MutateOpts};
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.game_dir.join(path)
        };
        self.push_undo();
        if self.scene.find_entity(entity).is_none() {
            if let Err(e) = add_entity(&mut self.scene, entity, &MutateOpts::default()) {
                let _ = self.undo.undo(&mut self.scene);
                self.status = format!("import ASCII failed: {e}");
                return;
            }
        }
        match tilemap_from_ascii_path(&mut self.scene, entity, &abs, 0, 0, true, None) {
            Ok(r) => {
                self.sync_baseline();
                self.mark_dirty();
                let file = abs.file_name().and_then(|s| s.to_str()).unwrap_or("map");
                self.status = format!(
                    "imported {file} → {entity} {}×{} ({} cells{})",
                    r.width,
                    r.height,
                    r.stamped,
                    if r.resized { ", resized" } else { "" }
                );
            }
            Err(e) => {
                let _ = self.undo.undo(&mut self.scene);
                self.status = format!("import ASCII failed: {e}");
            }
        }
    }

    pub(crate) fn preview_wav_clip(&mut self, clip: &str, volume: f32) {
        if clip.is_empty() {
            self.status = "AudioSource has no clip".into();
            return;
        }
        let assets = self.project.assets_path(&self.game_dir);
        match self.host_audio.play_clip(&assets, clip, volume) {
            Ok((PlayOutcome::Played, path)) => {
                self.status = format!("played {}", path.display());
            }
            Ok((PlayOutcome::SkippedNoDevice, path)) => {
                self.status = format!("audio skipped (no device) {}", path.display());
            }
            Err(e) => {
                self.status = format!("audio: {e}");
            }
        }
    }

    pub(crate) fn save_entity_as_prefab(&mut self, name: &str) {
        match wiimaker_scene::entity_to_prefab(&self.scene, name) {
            Ok(prefab) => {
                let dest = self
                    .game_dir
                    .join("assets")
                    .join("prefabs")
                    .join(format!("{name}.prefab.json"));
                match wiimaker_scene::save_prefab(&dest, &prefab) {
                    Ok(()) => {
                        let link = format!("assets/prefabs/{name}.prefab.json");
                        self.push_undo();
                        if let Err(e) =
                            wiimaker_scene::attach_prefab_instance(&mut self.scene, name, &link)
                        {
                            self.status = format!("prefab saved, link failed: {e}");
                        } else {
                            self.sync_baseline();
                            self.mark_dirty();
                            self.status = format!("prefab → {link}");
                        }
                        self.refresh_project_tree();
                    }
                    Err(e) => self.status = format!("save prefab failed: {e}"),
                }
            }
            Err(e) => self.status = format!("prefab failed: {e}"),
        }
    }

    pub(crate) fn instantiate_prefab_rel(&mut self, rel: &std::path::Path) {
        let abs = self.game_dir.join(rel);
        match wiimaker_scene::load_prefab(&abs) {
            Ok(prefab) => {
                self.push_undo();
                let source = rel.to_string_lossy().replace('\\', "/");
                let new_name = wiimaker_scene::instantiate_prefab(
                    &mut self.scene,
                    &prefab,
                    &source,
                    Some(320.0),
                    Some(240.0),
                );
                self.select(Some(new_name.clone()));
                self.sync_baseline();
                self.mark_dirty();
                self.status = format!("instantiated → {new_name}");
            }
            Err(e) => self.status = format!("load prefab failed: {e}"),
        }
    }

    pub(crate) fn apply_selected_prefab(&mut self, name: &str) {
        let Some(src) = self.scene.find_entity(name).and_then(|e| e.prefab.clone()) else {
            self.status = format!("{name} is not a prefab instance");
            return;
        };
        match wiimaker_scene::resolve_prefab_asset(&self.game_dir, &src) {
            Ok(path) => match wiimaker_scene::load_prefab(&path) {
                Ok(mut prefab) => {
                    self.push_undo();
                    if let Err(e) =
                        wiimaker_scene::apply_prefab(&mut self.scene, name, &mut prefab, &src)
                    {
                        let _ = self.undo.undo(&mut self.scene);
                        self.status = format!("apply prefab failed: {e}");
                        return;
                    }
                    if let Err(e) = wiimaker_scene::save_prefab(&path, &prefab) {
                        let _ = self.undo.undo(&mut self.scene);
                        self.status = format!("write prefab failed: {e}");
                        return;
                    }
                    self.sync_baseline();
                    self.mark_dirty();
                    self.refresh_project_tree();
                    self.status = format!("applied {name} → {}", path.display());
                }
                Err(e) => self.status = format!("load prefab failed: {e}"),
            },
            Err(e) => self.status = format!("{e}"),
        }
    }

    pub(crate) fn revert_selected_prefab(&mut self, name: &str) {
        let Some(ent) = self.scene.find_entity(name).cloned() else {
            self.status = format!("entity '{name}' not found");
            return;
        };
        let Some(src) = ent.prefab.clone() else {
            self.status = format!("{name} is not a prefab instance");
            return;
        };
        match wiimaker_scene::load_prefab_for_instance(&self.game_dir, &ent) {
            Ok(prefab) => {
                self.push_undo();
                if let Err(e) =
                    wiimaker_scene::revert_prefab_instance(&mut self.scene, name, &prefab)
                {
                    let _ = self.undo.undo(&mut self.scene);
                    self.status = format!("revert prefab failed: {e}");
                    return;
                }
                self.sync_baseline();
                self.mark_dirty();
                self.status = format!("reverted {name} from {src}");
            }
            Err(e) => self.status = format!("{e}"),
        }
    }

    pub(crate) fn unpack_selected_prefab(&mut self, name: &str) {
        self.push_undo();
        if let Err(e) = wiimaker_scene::unpack_prefab_instance(&mut self.scene, name) {
            let _ = self.undo.undo(&mut self.scene);
            self.status = format!("unpack failed: {e}");
            return;
        }
        self.sync_baseline();
        self.mark_dirty();
        self.status = format!("unpacked {name}");
    }

    pub(crate) fn doctor(&mut self) {
        let diag = diagnose(&self.game_dir, &self.project);
        if diag.issues.is_empty() {
            self.log_line(ConsoleLevel::Info, "doctor ok");
        } else {
            for issue in &diag.issues {
                let level = match issue.severity {
                    Severity::Error => ConsoleLevel::Error,
                    Severity::Warning => ConsoleLevel::Warn,
                    Severity::Info => ConsoleLevel::Info,
                };
                self.console_push(level, format!("doctor: {}", issue.message));
            }
            let summary = if diag.ok {
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
            self.status = summary;
            self.focus_tab(EditorTab::Console);
        }
    }

    pub(crate) fn play(&mut self) {
        match self.play_mode {
            PlayMode::Edit => {
                self.prepare_assets();
                self.rehydrate();
                let scene_json = serde_json::to_string(&self.scene).ok();
                let mut session = PlaySession::start(PlayStart {
                    workspace: &self.root,
                    package: &self.project.name,
                    game_dir: &self.game_dir,
                    scene_json: scene_json.as_deref(),
                    force_fallback: false,
                    build_if_needed: true,
                });
                for line in session.take_logs() {
                    self.log_line(ConsoleLevel::Info, line);
                }
                if !session.is_plugin() {
                    self.world.queue_awake_audio();
                    let assets = self.project.assets_path(&self.game_dir);
                    for err in self.host_audio.play_world(&mut self.world, &assets) {
                        self.log_line(ConsoleLevel::Warn, format!("audio: {err}"));
                    }
                }
                self.play_session = Some(session);
                self.play_mode = PlayMode::Playing;
                self.last_play_hit = None;
            }
            PlayMode::Paused => {
                self.play_mode = PlayMode::Playing;
                self.log_line(ConsoleLevel::Info, "resumed");
            }
            PlayMode::Playing => {
                self.play_mode = PlayMode::Paused;
                self.log_line(ConsoleLevel::Info, "paused");
            }
        }
    }

    pub(crate) fn stop_play(&mut self) {
        if self.play_mode == PlayMode::Edit {
            return;
        }
        self.play_mode = PlayMode::Edit;
        self.play_session = None;
        self.last_play_hit = None;
        self.rehydrate();
        self.log_line(ConsoleLevel::Info, "stopped · edits preserved");
    }

    pub(crate) fn play_kind(&self) -> Option<PlayKind> {
        self.play_session.as_ref().map(|s| s.kind())
    }

    pub(crate) fn play_external(&mut self) {
        self.prepare_assets();
        let status = Command::new("cargo")
            .args(["run", "-p", &self.project.name])
            .current_dir(&self.root)
            .spawn();
        match status {
            Ok(_) => self.status = "launched external play".into(),
            Err(e) => self.status = format!("play failed: {e}"),
        }
    }

    /// In-editor play tick: game `App` plugin when present, else WASD `Player`.
    pub(crate) fn tick_play_mode(&mut self, ctx: &egui::Context) {
        if self.play_mode != PlayMode::Playing {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_play();
            return;
        }
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.05);
        let keys = ctx.input(|i| PadKeys {
            left: i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft),
            right: i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight),
            up: i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp),
            down: i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown),
            a: i.key_down(egui::Key::Z) || i.key_down(egui::Key::Space),
            b: i.key_down(egui::Key::X),
            start: i.key_down(egui::Key::Enter),
        });
        let mut play_input = Input::new();
        play_input.begin_frame();
        apply_pad_keys(&mut play_input, keys);

        let Some(session) = self.play_session.as_mut() else {
            return;
        };
        let plugin = session.is_plugin();
        let tick = if plugin {
            session.tick(
                &play_input,
                dt,
                None,
                None,
                None,
                VIEW_W as u32,
                VIEW_H as u32,
            )
        } else {
            session.tick(
                &play_input,
                dt,
                Some(&mut self.world),
                Some(&self.catalog),
                Some(self.atlas.map()),
                VIEW_W as u32,
                VIEW_H as u32,
            )
        };
        match tick {
            Ok(result) => {
                if let Some(name) = result.hit_name {
                    if self.last_play_hit.as_deref() != Some(name.as_str()) {
                        self.console_push(
                            ConsoleLevel::Info,
                            format!("Play collision · Player × {name}"),
                        );
                        self.last_play_hit = Some(name);
                    }
                } else if !plugin {
                    self.last_play_hit = None;
                }
            }
            Err(e) => self.log_line(ConsoleLevel::Error, format!("play tick: {e}")),
        }
        if !plugin {
            let assets = self.project.assets_path(&self.game_dir);
            for err in self.host_audio.play_world(&mut self.world, &assets) {
                self.log_line(ConsoleLevel::Warn, format!("audio: {err}"));
            }
        }
    }

    fn spawn_wiimaker(&mut self, args: &[&str], ok_status: &str) {
        let status = Command::new("cargo")
            .args(["run", "-q", "-p", "wiimaker-cli", "--"])
            .args(args)
            .current_dir(&self.root)
            .spawn();
        match status {
            Ok(_) => self.status = ok_status.into(),
            Err(e) => self.status = format!("{ok_status} failed: {e}"),
        }
    }

    pub(crate) fn build_wii(&mut self) {
        self.status = "Building…".into();
        let name = self.project.name.clone();
        self.spawn_wiimaker(&["build", &name], "Build started");
    }

    pub(crate) fn play_dolphin(&mut self) {
        let name = self.project.name.clone();
        self.spawn_wiimaker(&["dolphin", &name], "Play in Dolphin started");
    }

    pub(crate) fn build_and_run_wii(&mut self) {
        self.status = "Build & Run…".into();
        let name = self.project.name.clone();
        self.spawn_wiimaker(&["play-wii", &name], "Build & Run started");
    }

    pub(crate) fn tilemap_target(&self) -> Option<String> {
        if let Some(name) = self.primary_selected() {
            if self
                .scene
                .find_entity(name)
                .and_then(|e| e.components.tilemap.as_ref())
                .is_some()
            {
                return Some(name.to_string());
            }
        }
        self.scene
            .entities
            .iter()
            .find(|e| e.components.tilemap.as_ref().is_some_and(|t| t.enabled))
            .map(|e| e.name.clone())
    }

    fn show_unsaved_modal(&mut self, ctx: &egui::Context) {
        if self.pending_open.is_none() {
            return;
        }
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Current scene has unsaved changes. Save before switching?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.save();
                        if !self.dirty {
                            if let Some(path) = self.pending_open.take() {
                                self.open_scene_at(path);
                            }
                        }
                    }
                    if ui.button("Discard").clicked() {
                        if let Some(path) = self.pending_open.take() {
                            self.open_scene_at(path);
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending_open = None;
                    }
                });
            });
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (cmd_s, cmd_z, cmd_shift_z, cmd_y, cmd_d, cmd_c, cmd_v, cmd_i, nudge) =
            ctx.input(|i| {
                let cmd = i.modifiers.command;
                let shift = i.modifiers.shift;
                let mut nudge_xy = [0.0_f32, 0.0_f32];
                if !cmd && !self.selected.is_empty() {
                    let step = if shift || self.prefs.scene_view.snap {
                        self.prefs.scene_view.snap_size.max(1.0)
                    } else {
                        1.0
                    };
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        nudge_xy[0] -= step;
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        nudge_xy[0] += step;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        nudge_xy[1] -= step;
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        nudge_xy[1] += step;
                    }
                }
                (
                    cmd && i.key_pressed(egui::Key::S),
                    cmd && !shift && i.key_pressed(egui::Key::Z),
                    cmd && shift && i.key_pressed(egui::Key::Z),
                    cmd && i.key_pressed(egui::Key::Y),
                    cmd && i.key_pressed(egui::Key::D),
                    cmd && i.key_pressed(egui::Key::C),
                    cmd && i.key_pressed(egui::Key::V),
                    cmd && i.key_pressed(egui::Key::I),
                    nudge_xy,
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
        if cmd_i {
            if let Some(rel) = self
                .project_entries
                .iter()
                .find(|e| !e.is_dir && e.rel.to_string_lossy().ends_with(".prefab.json"))
                .map(|e| e.rel.clone())
            {
                self.instantiate_prefab_rel(&rel);
            } else {
                self.status = "instantiate: no .prefab.json in Project".into();
            }
        }
        if cmd_c {
            self.do_copy();
        }
        if cmd_v {
            self.do_paste();
        }
        if nudge != [0.0, 0.0] && self.play_mode == PlayMode::Edit {
            self.nudge_selected(nudge[0], nudge[1]);
        }

        self.tick_play_mode(ctx);
        if self.play_mode == PlayMode::Playing {
            ctx.request_repaint();
        } else if self.play_mode == PlayMode::Edit {
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.05);
            if self.world.tick_tilemaps(dt) {
                ctx.request_repaint();
            }
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.import_asset_paths(&dropped);
        }

        self.end_inspector_gesture_if_released(ctx);

        if let Some(stem) = self.open_sprite_editor_stem.take() {
            self.open_sprite_editor(&stem, ctx);
        }

        if self.sprite_editor_open {
            let refresh = if let Some(state) = self.sprite_editor.as_mut() {
                sprite_editor::show_sprite_editor(ctx, state, &mut self.sprite_editor_open)
            } else {
                false
            };
            if refresh {
                self.refresh_catalog_after_slice();
            }
            if !self.sprite_editor_open {
                self.sprite_editor = None;
            }
        }

        self.ui_toolbar(ctx);
        self.show_unsaved_modal(ctx);
        self.show_build_settings_window(ctx);
        self.apply_pending_focus();

        // Top toolbar stays as TopBottomPanel. Everything else is one DockArea.
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_DEEP)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                self.ui_dock(ui);
            });
        self.apply_pending_focus();

        self.flush_prefs();

        ctx.request_repaint();
    }
}
