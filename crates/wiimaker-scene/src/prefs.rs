//! Editor chrome prefs (`<game>/.wiimaker/prefs.toml`).
//!
//! Scene zoom / pan / grid / gizmos, Game view aspect, and Project explorer
//! collapsed folders live here — not in `game.toml`, which is runtime/build
//! metadata. Editor GUI and CLI both call [`load_editor_prefs`] /
//! [`save_editor_prefs`].

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use wiimaker_assets::atomic_write;
use wiimaker_core::world::{SCREEN_H, SCREEN_W};

/// Relative path under the game dir.
pub const EDITOR_PREFS_REL: &str = ".wiimaker/prefs.toml";

pub fn editor_prefs_path(game_dir: &Path) -> PathBuf {
    game_dir.join(".wiimaker").join("prefs.toml")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameViewAspect {
    #[default]
    Free,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameViewPreset {
    #[default]
    Free,
    #[serde(rename = "640x480")]
    Res640x480,
    #[serde(rename = "16:9")]
    Ratio16x9,
    #[serde(rename = "4:3")]
    Ratio4x3,
    Custom,
}

/// Where the Scene tool handle sits. Unity default is Pivot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandlePosition {
    #[default]
    Pivot,
    Center,
}

impl HandlePosition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pivot => "pivot",
            Self::Center => "center",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pivot" => Ok(Self::Pivot),
            "center" => Ok(Self::Center),
            other => bail!("handle position must be pivot|center, got '{other}'"),
        }
    }
}

/// What entering Play does to the editor layout.
///
/// `focused` keeps the current docks. `maximized` fills the editor with the
/// Game view until Stop restores the previous layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GamePlayMode {
    #[default]
    Focused,
    Maximized,
}

impl GamePlayMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Maximized => "maximized",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Focused => "Play Focused",
            Self::Maximized => "Play Maximized",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "focused" | "play focused" | "play-focused" => Ok(Self::Focused),
            "maximized" | "play maximized" | "play-maximized" => Ok(Self::Maximized),
            other => bail!("play-mode must be focused|maximized, got '{other}'"),
        }
    }
}

/// How Move / Scale handles are oriented. Unity default is Local.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandleRotation {
    #[default]
    Local,
    Global,
}

impl HandleRotation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "global" => Ok(Self::Global),
            other => bail!("handle rotation must be local|global, got '{other}'"),
        }
    }
}

/// Active Scene toolbar tool. Persisted so the editor reopens on the same tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SceneTool {
    #[default]
    Move,
    Rotate,
    Scale,
    Hand,
    Paint,
    Erase,
    Pick,
}

impl SceneTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Rotate => "rotate",
            Self::Scale => "scale",
            Self::Hand => "hand",
            Self::Paint => "paint",
            Self::Erase => "erase",
            Self::Pick => "pick",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "move" | "translate" => Ok(Self::Move),
            "rotate" => Ok(Self::Rotate),
            "scale" => Ok(Self::Scale),
            "hand" | "pan" => Ok(Self::Hand),
            "paint" => Ok(Self::Paint),
            "erase" => Ok(Self::Erase),
            "pick" => Ok(Self::Pick),
            other => {
                bail!("scene tool must be move|rotate|scale|hand|paint|erase|pick, got '{other}'")
            }
        }
    }
}

impl GameViewPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Res640x480 => "640x480",
            Self::Ratio16x9 => "16:9",
            Self::Ratio4x3 => "4:3",
            Self::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "free" => Ok(Self::Free),
            "640x480" | "640×480" => Ok(Self::Res640x480),
            "16:9" | "16x9" => Ok(Self::Ratio16x9),
            "4:3" | "4x3" => Ok(Self::Ratio4x3),
            "custom" => Ok(Self::Custom),
            other => bail!("unknown game-view preset '{other}' (free|640x480|16:9|4:3|custom)"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneViewPrefs {
    /// 1.0 = fill the Scene well (same as the old stretch blit).
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    #[serde(default)]
    pub pan_x: f32,
    #[serde(default)]
    pub pan_y: f32,
    #[serde(default)]
    pub grid_overlay: bool,
    #[serde(default = "default_true")]
    pub gizmos: bool,
    /// Engine is 2D-only; kept so the toolbar control has a stored value.
    #[serde(default = "default_true")]
    pub mode_2d: bool,
    #[serde(default)]
    pub snap: bool,
    #[serde(default = "default_snap_size")]
    pub snap_size: f32,
    /// Tool handle position. Missing key → Pivot (Unity default).
    #[serde(default)]
    pub handle_position: HandlePosition,
    /// Tool handle orientation. Missing key → Local (Unity default).
    #[serde(default)]
    pub handle_rotation: HandleRotation,
    /// Active Scene tool. Missing key → Move.
    #[serde(default)]
    pub tool: SceneTool,
}

fn default_zoom() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_snap_size() -> f32 {
    16.0
}

impl Default for SceneViewPrefs {
    fn default() -> Self {
        Self {
            zoom: default_zoom(),
            pan_x: 0.0,
            pan_y: 0.0,
            grid_overlay: false,
            gizmos: true,
            mode_2d: true,
            snap: false,
            snap_size: default_snap_size(),
            handle_position: HandlePosition::Pivot,
            handle_rotation: HandleRotation::Local,
            tool: SceneTool::Move,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameViewPrefs {
    #[serde(default)]
    pub preset: GameViewPreset,
    #[serde(default)]
    pub aspect: GameViewAspect,
    #[serde(default = "default_view_w")]
    pub width: u32,
    #[serde(default = "default_view_h")]
    pub height: u32,
    /// 1.0 = contain-fit the chosen aspect in the Game well.
    #[serde(default = "default_zoom")]
    pub scale: f32,
    /// `focused` (keep docks) or `maximized` (Game view fills the editor in Play).
    #[serde(default)]
    pub play_mode: GamePlayMode,
    /// Host editor Play drops AudioSource oneshots and play-on-awake while on.
    #[serde(default)]
    pub mute_audio: bool,
    /// Game view stats overlay (measured FPS, entities, DrawList).
    #[serde(default)]
    pub stats: bool,
}

fn default_view_w() -> u32 {
    640
}
fn default_view_h() -> u32 {
    480
}

impl Default for GameViewPrefs {
    fn default() -> Self {
        Self {
            preset: GameViewPreset::Free,
            aspect: GameViewAspect::Free,
            width: default_view_w(),
            height: default_view_h(),
            scale: 1.0,
            play_mode: GamePlayMode::Focused,
            mute_audio: false,
            stats: false,
        }
    }
}

impl GameViewPrefs {
    /// Aspect numerator/denominator for letterbox. Free uses the well size (Unity Free Aspect).
    pub fn aspect_wh(&self, well_w: f32, well_h: f32) -> (f32, f32) {
        match self.preset {
            GameViewPreset::Free => (well_w.max(1.0), well_h.max(1.0)),
            GameViewPreset::Res640x480 => (640.0, 480.0),
            GameViewPreset::Ratio16x9 => (16.0, 9.0),
            GameViewPreset::Ratio4x3 => (4.0, 3.0),
            GameViewPreset::Custom => (self.width.max(1) as f32, self.height.max(1) as f32),
        }
    }

    pub fn is_free(&self) -> bool {
        self.preset == GameViewPreset::Free
    }
}

/// Project explorer chrome (collapsed folders). Search filter is session-only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectViewPrefs {
    /// Relative folder paths that are collapsed (e.g. `assets`, `assets/fx`).
    /// Empty = all expanded.
    #[serde(default)]
    pub collapsed: Vec<String>,
}

/// Console toolbar prefs. Search text is session-only and is not stored.
///
/// Unity 6 dark Console defaults: **Clear on Play** on, Clear on Build /
/// Collapse / Error Pause off. Missing `[console]` (or a missing key) loads
/// those defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsolePrefs {
    /// Clear the Console when entering Play from Stopped (Play or Step).
    /// Unpause does not clear.
    #[serde(default = "default_true")]
    pub clear_on_play: bool,
    /// Clear when Build, Build & Run, or Play in Dolphin starts.
    #[serde(default)]
    pub clear_on_build: bool,
    /// Identical lines (same level + same text) show once with a count.
    #[serde(default)]
    pub collapse: bool,
    /// While Playing, an Error-level Console line switches Play to Paused.
    #[serde(default)]
    pub error_pause: bool,
}

impl Default for ConsolePrefs {
    fn default() -> Self {
        Self {
            clear_on_play: true,
            clear_on_build: false,
            collapse: false,
            error_pause: false,
        }
    }
}

/// Why the editor might clear the Console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleClearWhen {
    /// Play or Step from Stopped. Not unpause.
    EnterPlayFromStopped,
    /// Resume from Paused. Never clears.
    Unpause,
    /// Build, Build & Run, or Play in Dolphin.
    Build,
}

/// Clear on Play only when entering from Stopped. Clear on Build only for builds.
pub fn console_should_clear(prefs: &ConsolePrefs, when: ConsoleClearWhen) -> bool {
    match when {
        ConsoleClearWhen::EnterPlayFromStopped => prefs.clear_on_play,
        ConsoleClearWhen::Unpause => false,
        ConsoleClearWhen::Build => prefs.clear_on_build,
    }
}

/// Error Pause pauses only while Play is running and the new line is an Error.
///
/// Warn / Info, and errors logged while already Paused or Stopped, do not change
/// transport. The editor calls this from the Console push path (play-tick
/// failures and any other Error it routes during Play).
pub fn console_should_error_pause(error_pause: bool, playing: bool, is_error: bool) -> bool {
    error_pause && playing && is_error
}

/// Optional fields for [`set_console`] / [`apply_console_prefs`]. `on` | `off`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConsolePatch<'a> {
    pub clear_on_play: Option<&'a str>,
    pub clear_on_build: Option<&'a str>,
    pub collapse: Option<&'a str>,
    pub error_pause: Option<&'a str>,
}

impl ConsolePatch<'_> {
    pub fn is_empty(self) -> bool {
        self.clear_on_play.is_none()
            && self.clear_on_build.is_none()
            && self.collapse.is_none()
            && self.error_pause.is_none()
    }
}

/// One scene's hidden / unpickable entity names.
///
/// Names that no longer exist in the scene are ignored by the Scene view and
/// dropped on rename/delete. They must not change draw or pick.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneVisibilityScene {
    /// Path relative to the game dir, e.g. `scenes/main.scene.json`.
    pub scene: String,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub unpickable: Vec<String>,
}

/// Scene-view visibility and pickability. Editor-only; not written into scene JSON.
///
/// `enabled` is the toolbar eye-with-slash toggle. When it is off, Scene view
/// draws every entity and the per-entity lists stay stored. Missing table on
/// old prefs files loads as enabled, with nothing hidden or unpickable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneVisibilityPrefs {
    /// `true` applies per-entity hides in the Scene view. Default on.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scenes: Vec<SceneVisibilityScene>,
}

impl Default for SceneVisibilityPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            scenes: Vec::new(),
        }
    }
}

impl SceneVisibilityPrefs {
    pub fn scene(&self, key: &str) -> Option<&SceneVisibilityScene> {
        self.scenes.iter().find(|s| s.scene == key)
    }

    pub fn scene_mut(&mut self, key: &str) -> Option<&mut SceneVisibilityScene> {
        self.scenes.iter_mut().find(|s| s.scene == key)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorPrefs {
    #[serde(default)]
    pub scene_view: SceneViewPrefs,
    #[serde(default)]
    pub game_view: GameViewPrefs,
    #[serde(default)]
    pub project_view: ProjectViewPrefs,
    /// Unity Console toggles. Missing table → Clear on Play on, the rest off.
    #[serde(default)]
    pub console: ConsolePrefs,
    /// Hierarchy Scene visibility / pickability. Missing table → everything
    /// visible and pickable, toolbar toggle on.
    #[serde(default)]
    pub scene_visibility: SceneVisibilityPrefs,
}

pub fn load_editor_prefs(game_dir: &Path) -> Result<EditorPrefs> {
    let path = editor_prefs_path(game_dir);
    if !path.is_file() {
        return Ok(EditorPrefs::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let prefs: EditorPrefs = toml::from_str(&text).context("parse .wiimaker/prefs.toml")?;
    Ok(prefs)
}

pub fn save_editor_prefs(game_dir: &Path, prefs: &EditorPrefs) -> Result<()> {
    let path = editor_prefs_path(game_dir);
    let text = toml::to_string_pretty(prefs).context("serialize editor prefs")?;
    atomic_write(&path, (text + "\n").as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Infer a named preset from pixel size (used when `--width/--height` are set).
pub fn infer_game_view_preset(width: u32, height: u32) -> GameViewPreset {
    if width == 640 && height == 480 {
        return GameViewPreset::Res640x480;
    }
    if width > 0 && height > 0 && width.saturating_mul(9) == height.saturating_mul(16) {
        return GameViewPreset::Ratio16x9;
    }
    if width > 0 && height > 0 && width.saturating_mul(3) == height.saturating_mul(4) {
        return GameViewPreset::Ratio4x3;
    }
    GameViewPreset::Custom
}

/// Optional fields for [`set_game_view`] / [`apply_game_view`].
/// Unset fields are left unchanged. `mute_audio` and `stats` are `on` | `off`.
#[derive(Clone, Copy, Debug, Default)]
pub struct GameViewPatch<'a> {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub aspect: Option<&'a str>,
    pub preset: Option<&'a str>,
    pub scale: Option<f32>,
    /// `focused` | `maximized`
    pub play_mode: Option<&'a str>,
    /// `on` | `off`
    pub mute_audio: Option<&'a str>,
    /// `on` | `off`
    pub stats: Option<&'a str>,
}

impl GameViewPatch<'_> {
    pub fn is_empty(self) -> bool {
        self.width.is_none()
            && self.height.is_none()
            && self.aspect.is_none()
            && self.preset.is_none()
            && self.scale.is_none()
            && self.play_mode.is_none()
            && self.mute_audio.is_none()
            && self.stats.is_none()
    }
}

fn parse_on_off(s: &str, flag: &str) -> Result<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "on" | "true" => Ok(true),
        "off" | "false" => Ok(false),
        other => bail!("{flag} must be on|off, got '{other}'"),
    }
}

/// Mutate Game view fields and persist.
pub fn set_game_view(game_dir: &Path, patch: GameViewPatch<'_>) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    apply_game_view(&mut prefs.game_view, patch)?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

pub fn apply_game_view(gv: &mut GameViewPrefs, patch: GameViewPatch<'_>) -> Result<()> {
    let GameViewPatch {
        width,
        height,
        aspect,
        preset,
        scale,
        play_mode,
        mute_audio,
        stats,
    } = patch;
    if let Some(p) = preset {
        gv.preset = GameViewPreset::parse(p)?;
        match gv.preset {
            GameViewPreset::Free => {
                gv.aspect = GameViewAspect::Free;
            }
            GameViewPreset::Res640x480 => {
                gv.aspect = GameViewAspect::Fixed;
                gv.width = 640;
                gv.height = 480;
            }
            GameViewPreset::Ratio16x9 => {
                gv.aspect = GameViewAspect::Fixed;
                gv.width = 640;
                gv.height = 360;
            }
            GameViewPreset::Ratio4x3 => {
                gv.aspect = GameViewAspect::Fixed;
                gv.width = 640;
                gv.height = 480;
            }
            GameViewPreset::Custom => {
                gv.aspect = GameViewAspect::Fixed;
            }
        }
    }
    if let Some(a) = aspect {
        match a.trim().to_ascii_lowercase().as_str() {
            "free" => {
                gv.aspect = GameViewAspect::Free;
                gv.preset = GameViewPreset::Free;
            }
            "fixed" => {
                gv.aspect = GameViewAspect::Fixed;
                if gv.preset == GameViewPreset::Free {
                    gv.preset = infer_game_view_preset(gv.width, gv.height);
                    if gv.preset == GameViewPreset::Free {
                        gv.preset = GameViewPreset::Res640x480;
                        gv.width = 640;
                        gv.height = 480;
                    }
                }
            }
            other => bail!("aspect must be free|fixed, got '{other}'"),
        }
    }
    if width.is_some() || height.is_some() {
        if let Some(w) = width {
            if w == 0 {
                bail!("width must be > 0");
            }
            gv.width = w;
        }
        if let Some(h) = height {
            if h == 0 {
                bail!("height must be > 0");
            }
            gv.height = h;
        }
        gv.aspect = GameViewAspect::Fixed;
        gv.preset = infer_game_view_preset(gv.width, gv.height);
    }
    if let Some(s) = scale {
        gv.scale = s.clamp(0.1, 8.0);
    }
    if let Some(mode) = play_mode {
        gv.play_mode = GamePlayMode::parse(mode)?;
    }
    if let Some(m) = mute_audio {
        gv.mute_audio = parse_on_off(m, "--mute-audio")?;
    }
    if let Some(s) = stats {
        gv.stats = parse_on_off(s, "--stats")?;
    }
    Ok(())
}

/// Mutate Scene view fields and persist.
#[allow(clippy::too_many_arguments)]
pub fn set_scene_view(
    game_dir: &Path,
    zoom: Option<f32>,
    pan_x: Option<f32>,
    pan_y: Option<f32>,
    grid_overlay: Option<bool>,
    gizmos: Option<bool>,
    snap: Option<bool>,
    snap_size: Option<f32>,
    mode_2d: Option<bool>,
    handle_position: Option<&str>,
    handle_rotation: Option<&str>,
    tool: Option<&str>,
) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    apply_scene_view(
        &mut prefs.scene_view,
        zoom,
        pan_x,
        pan_y,
        grid_overlay,
        gizmos,
        snap,
        snap_size,
        mode_2d,
        handle_position,
        handle_rotation,
        tool,
    )?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

#[allow(clippy::too_many_arguments)]
pub fn apply_scene_view(
    sv: &mut SceneViewPrefs,
    zoom: Option<f32>,
    pan_x: Option<f32>,
    pan_y: Option<f32>,
    grid_overlay: Option<bool>,
    gizmos: Option<bool>,
    snap: Option<bool>,
    snap_size: Option<f32>,
    mode_2d: Option<bool>,
    handle_position: Option<&str>,
    handle_rotation: Option<&str>,
    tool: Option<&str>,
) -> Result<()> {
    if let Some(z) = zoom {
        sv.zoom = z.clamp(0.1, 16.0);
    }
    if let Some(x) = pan_x {
        sv.pan_x = x;
    }
    if let Some(y) = pan_y {
        sv.pan_y = y;
    }
    if let Some(g) = grid_overlay {
        sv.grid_overlay = g;
    }
    if let Some(g) = gizmos {
        sv.gizmos = g;
    }
    if let Some(s) = snap {
        sv.snap = s;
    }
    if let Some(n) = snap_size {
        sv.snap_size = n.clamp(1.0, 128.0);
    }
    if let Some(m) = mode_2d {
        if !m {
            bail!("3D Scene view is not available (engine is 2D-only)");
        }
        sv.mode_2d = true;
    }
    if let Some(p) = handle_position {
        sv.handle_position = HandlePosition::parse(p)?;
    }
    if let Some(r) = handle_rotation {
        sv.handle_rotation = HandleRotation::parse(r)?;
    }
    if let Some(t) = tool {
        sv.tool = SceneTool::parse(t)?;
    }
    Ok(())
}

/// Clamp egui-style well sizing (Play Maximized can report infinite `available_size`).
pub fn clamp_viewport_well_size(
    available_w: f32,
    available_h: f32,
    max_w: f32,
    max_h: f32,
) -> (f32, f32) {
    let mut w = available_w;
    let mut h = available_h;
    if !w.is_finite() || w <= 0.0 {
        w = max_w;
    }
    if !h.is_finite() || h <= 0.0 {
        h = max_h;
    }
    w = w.min(max_w).max(1.0);
    h = h.min(max_h).max(1.0);
    (w, h)
}

/// Game view image rect inside the tab well (aspect prefs + [`fitted_blit_rect`]).
///
/// When `play_maximized` and the preset is Free, letterbox to the 640×480 game frame
/// instead of stretching to the maximized panel aspect.
pub fn game_view_blit_rect(
    gv: &GameViewPrefs,
    well_w: f32,
    well_h: f32,
    play_maximized: bool,
) -> (f32, f32, f32, f32) {
    let (aw, ah) = if play_maximized && gv.is_free() {
        (SCREEN_W, SCREEN_H)
    } else {
        gv.aspect_wh(well_w, well_h)
    };
    fitted_blit_rect(well_w, well_h, aw, ah, gv.scale)
}

/// Largest `aw:ah` rectangle that fits in `well_w × well_h`, then scaled about center.
/// Origin is the well top-left. `scale` 1 = contain-fit.
pub fn fitted_blit_rect(
    well_w: f32,
    well_h: f32,
    aw: f32,
    ah: f32,
    scale: f32,
) -> (f32, f32, f32, f32) {
    let well_w = well_w.max(1.0);
    let well_h = well_h.max(1.0);
    let aw = aw.max(1.0);
    let ah = ah.max(1.0);
    let scale = scale.clamp(0.1, 8.0);
    let well_aspect = well_w / well_h;
    let target = aw / ah;
    let (mut w, mut h) = if well_aspect > target {
        let h = well_h;
        (h * target, h)
    } else {
        let w = well_w;
        (w, w / target)
    };
    w *= scale;
    h *= scale;
    let x = (well_w - w) * 0.5;
    let y = (well_h - h) * 0.5;
    (x, y, w, h)
}

/// Scene blit rect: zoom 1 + pan 0 fills the well (legacy stretch).
pub fn scene_blit_rect(
    well_w: f32,
    well_h: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
) -> (f32, f32, f32, f32) {
    let well_w = well_w.max(1.0);
    let well_h = well_h.max(1.0);
    let zoom = zoom.clamp(0.1, 16.0);
    let w = well_w * zoom;
    let h = well_h * zoom;
    let x = (well_w - w) * 0.5 + pan_x;
    let y = (well_h - h) * 0.5 + pan_y;
    (x, y, w, h)
}

/// Slash-normalized relative path with no leading/trailing `/`.
pub fn normalize_project_rel(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

/// True when `rel` is a descendant of a collapsed folder (the folder row itself stays visible).
pub fn project_rel_hidden_by_collapse(rel: &str, collapsed: &[String]) -> bool {
    if collapsed.is_empty() {
        return false;
    }
    let rel = normalize_project_rel(rel);
    let collapsed: Vec<String> = collapsed.iter().map(|s| normalize_project_rel(s)).collect();
    let mut path = rel.as_str();
    while let Some((parent, _)) = path.rsplit_once('/') {
        if collapsed.iter().any(|c| c == parent) {
            return true;
        }
        path = parent;
    }
    false
}

/// True when this folder path is itself listed as collapsed.
pub fn folder_is_collapsed(collapsed: &[String], rel: &str) -> bool {
    let n = normalize_project_rel(rel);
    if n.is_empty() {
        return false;
    }
    collapsed.iter().any(|c| normalize_project_rel(c) == n)
}

/// Toggle `rel` in the collapsed list (normalize + sort). Empty paths are ignored.
pub fn toggle_project_collapsed(pv: &mut ProjectViewPrefs, rel: &str) {
    let n = normalize_project_rel(rel);
    if n.is_empty() {
        return;
    }
    if folder_is_collapsed(&pv.collapsed, &n) {
        pv.collapsed.retain(|c| normalize_project_rel(c) != n);
    } else {
        pv.collapsed.push(n);
        pv.collapsed.sort();
        pv.collapsed.dedup();
    }
}

/// Mutate Project view collapsed folders. `clear_collapsed` runs first, then
/// `--collapse` paths are added, then `--expand` paths are removed (expand wins).
pub fn apply_project_view(
    pv: &mut ProjectViewPrefs,
    collapse: &[String],
    expand: &[String],
    clear_collapsed: bool,
) -> Result<()> {
    if clear_collapsed {
        pv.collapsed.clear();
    }
    for p in collapse {
        let n = normalize_project_rel(p);
        if n.is_empty() {
            bail!("collapse path must not be empty");
        }
        if !folder_is_collapsed(&pv.collapsed, &n) {
            pv.collapsed.push(n);
        }
    }
    for p in expand {
        let n = normalize_project_rel(p);
        if n.is_empty() {
            bail!("expand path must not be empty");
        }
        pv.collapsed.retain(|c| normalize_project_rel(c) != n);
    }
    pv.collapsed.sort();
    pv.collapsed.dedup();
    Ok(())
}

pub fn apply_console_prefs(console: &mut ConsolePrefs, patch: ConsolePatch<'_>) -> Result<()> {
    if patch.is_empty() {
        bail!(
            "set-console needs at least one flag (--clear-on-play, --clear-on-build, --collapse, --error-pause)"
        );
    }
    if let Some(v) = patch.clear_on_play {
        console.clear_on_play = parse_on_off(v, "--clear-on-play")?;
    }
    if let Some(v) = patch.clear_on_build {
        console.clear_on_build = parse_on_off(v, "--clear-on-build")?;
    }
    if let Some(v) = patch.collapse {
        console.collapse = parse_on_off(v, "--collapse")?;
    }
    if let Some(v) = patch.error_pause {
        console.error_pause = parse_on_off(v, "--error-pause")?;
    }
    Ok(())
}

/// Mutate Console toolbar prefs and persist.
pub fn set_console(game_dir: &Path, patch: ConsolePatch<'_>) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    apply_console_prefs(&mut prefs.console, patch)?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

/// Toolbar Scene visibility toggle. `on` applies per-entity hides; `off` draws
/// every entity and keeps the lists. Persists `[scene_visibility].enabled`.
pub fn set_scene_visibility_enabled(game_dir: &Path, on_off: &str) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    prefs.scene_visibility.enabled = parse_on_off(on_off, "--scene-visibility")?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

/// Mutate Project view fields and persist.
pub fn set_project_view(
    game_dir: &Path,
    collapse: &[String],
    expand: &[String],
    clear_collapsed: bool,
) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    apply_project_view(&mut prefs.project_view, collapse, expand, clear_collapsed)?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tmp(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wiimaker-prefs-{}-{}", label, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_is_defaults() {
        let dir = tmp("missing");
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p, EditorPrefs::default());
        assert!(!editor_prefs_path(&dir).exists());
    }

    #[test]
    fn set_game_view_roundtrip() {
        let dir = tmp("game-view");
        set_game_view(
            &dir,
            GameViewPatch {
                width: Some(640),
                height: Some(480),
                aspect: Some("fixed"),
                scale: Some(0.5),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Res640x480);
        assert_eq!(p.game_view.aspect, GameViewAspect::Fixed);
        assert_eq!(p.game_view.width, 640);
        assert_eq!(p.game_view.height, 480);
        assert!((p.game_view.scale - 0.5).abs() < 1e-4);
        let text = fs::read_to_string(editor_prefs_path(&dir)).unwrap();
        assert!(text.contains("640x480") || text.contains("preset"));

        set_game_view(
            &dir,
            GameViewPatch {
                aspect: Some("free"),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Free);
        assert_eq!(p.game_view.aspect, GameViewAspect::Free);
    }

    #[test]
    fn preset_16_9_and_custom() {
        let dir = tmp("presets");
        set_game_view(
            &dir,
            GameViewPatch {
                preset: Some("16:9"),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Ratio16x9);
        assert_eq!(p.game_view.width, 640);
        assert_eq!(p.game_view.height, 360);

        set_game_view(
            &dir,
            GameViewPatch {
                width: Some(800),
                height: Some(600),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Ratio4x3);
        set_game_view(
            &dir,
            GameViewPatch {
                width: Some(333),
                height: Some(111),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Custom);
        assert_eq!(p.game_view.width, 333);
    }

    #[test]
    fn scene_view_roundtrip() {
        let dir = tmp("scene-view");
        set_scene_view(
            &dir,
            Some(2.0),
            Some(10.0),
            Some(-4.0),
            Some(true),
            Some(false),
            Some(true),
            Some(32.0),
            Some(true),
            None,
            None,
            None,
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!((p.scene_view.zoom - 2.0).abs() < 1e-4);
        assert!(p.scene_view.grid_overlay);
        assert!(!p.scene_view.gizmos);
        assert!(p.scene_view.snap);
        assert!((p.scene_view.snap_size - 32.0).abs() < 1e-4);
        assert_eq!(p.scene_view.handle_position, HandlePosition::Pivot);
        assert_eq!(p.scene_view.handle_rotation, HandleRotation::Local);
        assert_eq!(p.scene_view.tool, SceneTool::Move);
        assert!(set_scene_view(
            &dir,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(false),
            None,
            None,
            None,
        )
        .is_err());

        set_scene_view(
            &dir,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("center"),
            Some("global"),
            Some("rotate"),
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.scene_view.handle_position, HandlePosition::Center);
        assert_eq!(p.scene_view.handle_rotation, HandleRotation::Global);
        assert_eq!(p.scene_view.tool, SceneTool::Rotate);
        assert!((p.scene_view.zoom - 2.0).abs() < 1e-4);
        assert!(set_scene_view(
            &dir,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("nope"),
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn old_scene_prefs_default_pivot_local_move() {
        let dir = tmp("old-scene-prefs");
        fs::create_dir_all(dir.join(".wiimaker")).unwrap();
        fs::write(
            editor_prefs_path(&dir),
            "[scene_view]\nzoom = 1.25\ngrid_overlay = true\n",
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!((p.scene_view.zoom - 1.25).abs() < 1e-4);
        assert!(p.scene_view.grid_overlay);
        assert_eq!(p.scene_view.handle_position, HandlePosition::Pivot);
        assert_eq!(p.scene_view.handle_rotation, HandleRotation::Local);
        assert_eq!(p.scene_view.tool, SceneTool::Move);
        assert!(p.scene_view.gizmos);
        assert!(p.scene_visibility.enabled);
        assert!(p.scene_visibility.scenes.is_empty());
    }

    #[test]
    fn letterbox_640x480_in_wide_well() {
        // 800×400 well, 4:3 locked, scale 1 → pillarbox (bars on sides).
        let (x, y, w, h) = fitted_blit_rect(800.0, 400.0, 640.0, 480.0, 1.0);
        assert!((h - 400.0).abs() < 0.01);
        assert!((w - 400.0 * 640.0 / 480.0).abs() < 0.01);
        assert!(x > 50.0); // pillarbox
        assert!(y.abs() < 0.01);
    }

    #[test]
    fn letterbox_wide_in_tall_well() {
        let (x, y, w, h) = fitted_blit_rect(400.0, 800.0, 16.0, 9.0, 1.0);
        assert!((w - 400.0).abs() < 0.01);
        assert!(y > 50.0); // letterbox
        assert!(x.abs() < 0.01);
        let _ = h;
    }

    #[test]
    fn docked_free_fills_well_aspect() {
        let gv = GameViewPrefs::default();
        let (aw, ah) = gv.aspect_wh(1920.0, 1080.0);
        assert!((aw - 1920.0).abs() < 1e-3 && (ah - 1080.0).abs() < 1e-3);
        let (x, y, w, h) = game_view_blit_rect(&gv, 1920.0, 1080.0, false);
        assert!((w - 1920.0).abs() < 0.01 && (h - 1080.0).abs() < 0.01);
        assert!(x.abs() < 0.01 && y.abs() < 0.01);
    }

    #[test]
    fn maximized_free_letterboxes_ultrawide_well() {
        let gv = GameViewPrefs::default();
        let (well_w, well_h) = clamp_viewport_well_size(f32::INFINITY, 1080.0, 1920.0, 1080.0);
        assert!((well_w - 1920.0).abs() < 0.01 && (well_h - 1080.0).abs() < 0.01);
        let (x, y, w, h) = game_view_blit_rect(&gv, well_w, well_h, true);
        assert!((h - 1080.0).abs() < 0.01);
        assert!((w - 1440.0).abs() < 0.01);
        assert!((x - 240.0).abs() < 0.01);
        assert!(y.abs() < 0.01);
    }

    #[test]
    fn free_aspect_fills_then_scale_shrinks() {
        let (x, y, w, h) = fitted_blit_rect(640.0, 480.0, 640.0, 480.0, 1.0);
        assert!((w - 640.0).abs() < 0.01 && (h - 480.0).abs() < 0.01);
        assert!(x.abs() < 0.01 && y.abs() < 0.01);
        let (x, y, w, h) = fitted_blit_rect(640.0, 480.0, 640.0, 480.0, 0.5);
        assert!((w - 320.0).abs() < 0.01);
        assert!((h - 240.0).abs() < 0.01);
        assert!((x - 160.0).abs() < 0.01);
        assert!((y - 120.0).abs() < 0.01);
    }

    #[test]
    fn scene_zoom_pan() {
        let (x, y, w, h) = scene_blit_rect(200.0, 100.0, 1.0, 0.0, 0.0);
        assert_eq!((x, y, w, h), (0.0, 0.0, 200.0, 100.0));
        let (x, y, w, h) = scene_blit_rect(200.0, 100.0, 2.0, 10.0, 0.0);
        assert!((w - 400.0).abs() < 0.01);
        assert!((x - (-100.0 + 10.0)).abs() < 0.01);
        let _ = y;
        let _ = h;
    }

    #[test]
    fn project_view_collapse_roundtrip() {
        let dir = tmp("project-view");
        set_project_view(
            &dir,
            &["assets/".into(), "assets/fx".into(), r"assets\ui".into()],
            &[],
            false,
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(
            p.project_view.collapsed,
            vec![
                "assets".to_string(),
                "assets/fx".to_string(),
                "assets/ui".to_string()
            ]
        );
        let text = fs::read_to_string(editor_prefs_path(&dir)).unwrap();
        assert!(text.contains("project_view") || text.contains("collapsed"));

        set_project_view(&dir, &[], &["assets".into()], false).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(
            p.project_view.collapsed,
            vec!["assets/fx".to_string(), "assets/ui".to_string()]
        );

        set_project_view(&dir, &["assets".into()], &[], false).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.project_view.collapsed.len(), 3);

        set_project_view(&dir, &["assets".into()], &[], false).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(
            p.project_view
                .collapsed
                .iter()
                .filter(|c| *c == "assets")
                .count(),
            1
        );

        set_project_view(&dir, &["scenes".into()], &[], true).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.project_view.collapsed, vec!["scenes".to_string()]);

        let mut pv = p.project_view.clone();
        assert!(apply_project_view(&mut pv, &["".into()], &[], false).is_err());
        assert!(set_project_view(&dir, &[], &["".into()], false).is_err());
    }

    #[test]
    fn project_rel_hidden_by_collapsed_ancestors() {
        let collapsed = vec!["assets".into(), "scenes/ui".into()];
        assert!(!project_rel_hidden_by_collapse("assets", &collapsed));
        assert!(project_rel_hidden_by_collapse("assets/fx", &collapsed));
        assert!(project_rel_hidden_by_collapse(
            "assets/fx/spark.png",
            &collapsed
        ));
        assert!(!project_rel_hidden_by_collapse("game.toml", &collapsed));
        assert!(!project_rel_hidden_by_collapse("scenes", &collapsed));
        assert!(!project_rel_hidden_by_collapse(
            "scenes/main.scene.json",
            &collapsed
        ));
        assert!(!project_rel_hidden_by_collapse("scenes/ui", &collapsed));
        assert!(project_rel_hidden_by_collapse(
            "scenes/ui/hud.png",
            &collapsed
        ));
        assert!(folder_is_collapsed(&collapsed, "assets/"));
        assert!(!folder_is_collapsed(&collapsed, "assets/fx"));
    }

    #[test]
    fn toggle_project_collapsed_roundtrip() {
        let mut pv = ProjectViewPrefs::default();
        toggle_project_collapsed(&mut pv, "assets/");
        assert_eq!(pv.collapsed, vec!["assets".to_string()]);
        toggle_project_collapsed(&mut pv, "assets");
        assert!(pv.collapsed.is_empty());
        toggle_project_collapsed(&mut pv, "");
        assert!(pv.collapsed.is_empty());
    }

    #[test]
    fn missing_prefs_include_empty_project_view() {
        let dir = tmp("project-view-default");
        let p = load_editor_prefs(&dir).unwrap();
        assert!(p.project_view.collapsed.is_empty());
        assert_eq!(p.project_view, ProjectViewPrefs::default());
    }

    #[test]
    fn game_view_play_mode_mute_stats_roundtrip() {
        let dir = tmp("game-view-play");
        set_game_view(
            &dir,
            GameViewPatch {
                preset: Some("640x480"),
                play_mode: Some("maximized"),
                mute_audio: Some("on"),
                stats: Some("on"),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.play_mode, GamePlayMode::Maximized);
        assert!(p.game_view.mute_audio);
        assert!(p.game_view.stats);
        assert_eq!(p.game_view.preset, GameViewPreset::Res640x480);
        let text = fs::read_to_string(editor_prefs_path(&dir)).unwrap();
        assert!(text.contains("maximized"));
        assert!(text.contains("mute_audio"));
        assert!(text.contains("stats"));

        set_game_view(
            &dir,
            GameViewPatch {
                play_mode: Some("focused"),
                mute_audio: Some("off"),
                stats: Some("off"),
                scale: Some(1.25),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.play_mode, GamePlayMode::Focused);
        assert!(!p.game_view.mute_audio);
        assert!(!p.game_view.stats);
        assert!((p.game_view.scale - 1.25).abs() < 1e-4);
        assert_eq!(p.game_view.preset, GameViewPreset::Res640x480);

        let err = set_game_view(
            &dir,
            GameViewPatch {
                play_mode: Some("unfocused"),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("focused|maximized"), "{err}");
        let err = set_game_view(
            &dir,
            GameViewPatch {
                mute_audio: Some("maybe"),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("on|off"), "{err}");
    }

    #[test]
    fn old_game_view_prefs_default_play_chrome() {
        let dir = tmp("old-game-view");
        fs::create_dir_all(dir.join(".wiimaker")).unwrap();
        fs::write(
            editor_prefs_path(&dir),
            "[game_view]\npreset = \"640x480\"\nwidth = 640\nheight = 480\nscale = 1.0\n",
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Res640x480);
        assert_eq!(p.game_view.width, 640);
        assert_eq!(p.game_view.play_mode, GamePlayMode::Focused);
        assert!(!p.game_view.mute_audio);
        assert!(!p.game_view.stats);
        assert_eq!(p.game_view.aspect, GameViewAspect::Free);
    }

    #[test]
    fn old_prefs_without_project_view() {
        let dir = tmp("old-prefs");
        fs::create_dir_all(dir.join(".wiimaker")).unwrap();
        fs::write(editor_prefs_path(&dir), "[scene_view]\nzoom = 2.0\n").unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!((p.scene_view.zoom - 2.0).abs() < 1e-4);
        assert!(p.project_view.collapsed.is_empty());
        assert!(p.console.clear_on_play);
        assert!(!p.console.clear_on_build);
        assert!(!p.console.collapse);
        assert!(!p.console.error_pause);
    }

    #[test]
    fn missing_console_table_uses_unity_defaults() {
        let dir = tmp("console-default");
        fs::create_dir_all(dir.join(".wiimaker")).unwrap();
        fs::write(
            editor_prefs_path(&dir),
            "[game_view]\npreset = \"free\"\nscale = 1.0\n",
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!(p.console.clear_on_play);
        assert!(!p.console.clear_on_build);
        assert!(!p.console.collapse);
        assert!(!p.console.error_pause);
    }

    #[test]
    fn set_console_partial_roundtrip() {
        let dir = tmp("console-set");
        set_console(
            &dir,
            ConsolePatch {
                clear_on_play: Some("off"),
                error_pause: Some("on"),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!(!p.console.clear_on_play);
        assert!(!p.console.clear_on_build);
        assert!(!p.console.collapse);
        assert!(p.console.error_pause);
        let text = fs::read_to_string(editor_prefs_path(&dir)).unwrap();
        assert!(text.contains("[console]"), "{text}");
        assert!(text.contains("clear_on_play = false"), "{text}");
        assert!(text.contains("error_pause = true"), "{text}");

        set_console(
            &dir,
            ConsolePatch {
                collapse: Some("on"),
                clear_on_build: Some("true"),
                ..Default::default()
            },
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!(!p.console.clear_on_play);
        assert!(p.console.clear_on_build);
        assert!(p.console.collapse);
        assert!(p.console.error_pause);
    }

    #[test]
    fn set_console_rejects_empty_and_bad_flags() {
        let dir = tmp("console-bad");
        let err = set_console(&dir, ConsolePatch::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("set-console"), "{err}");
        let err = set_console(
            &dir,
            ConsolePatch {
                collapse: Some("maybe"),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("on|off"), "{err}");
    }

    #[test]
    fn clear_on_play_is_enter_from_stopped_only() {
        let mut prefs = ConsolePrefs::default();
        assert!(console_should_clear(
            &prefs,
            ConsoleClearWhen::EnterPlayFromStopped
        ));
        assert!(!console_should_clear(&prefs, ConsoleClearWhen::Unpause));
        assert!(!console_should_clear(&prefs, ConsoleClearWhen::Build));
        prefs.clear_on_play = false;
        prefs.clear_on_build = true;
        assert!(!console_should_clear(
            &prefs,
            ConsoleClearWhen::EnterPlayFromStopped
        ));
        assert!(!console_should_clear(&prefs, ConsoleClearWhen::Unpause));
        assert!(console_should_clear(&prefs, ConsoleClearWhen::Build));
    }

    #[test]
    fn error_pause_only_while_playing_on_error() {
        assert!(console_should_error_pause(true, true, true));
        assert!(!console_should_error_pause(false, true, true));
        assert!(!console_should_error_pause(true, false, true));
        assert!(!console_should_error_pause(true, true, false));
        assert!(!console_should_error_pause(true, false, false));
    }
}
