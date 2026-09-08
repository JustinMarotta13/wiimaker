//! Editor chrome prefs (`<game>/.wiimaker/prefs.toml`).
//!
//! Scene zoom / pan / grid / gizmos and Game view aspect live here — not in
//! `game.toml`, which is runtime/build metadata. Editor GUI and CLI both call
//! [`load_editor_prefs`] / [`save_editor_prefs`].

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

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
        }
    }
}

impl GameViewPrefs {
    /// Aspect numerator/denominator for letterbox. Free uses the well size.
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorPrefs {
    #[serde(default)]
    pub scene_view: SceneViewPrefs,
    #[serde(default)]
    pub game_view: GameViewPrefs,
}

pub fn load_editor_prefs(game_dir: &Path) -> Result<EditorPrefs> {
    let path = editor_prefs_path(game_dir);
    if !path.is_file() {
        return Ok(EditorPrefs::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let prefs: EditorPrefs = toml::from_str(&text).context("parse .wiimaker/prefs.toml")?;
    Ok(prefs)
}

pub fn save_editor_prefs(game_dir: &Path, prefs: &EditorPrefs) -> Result<()> {
    let path = editor_prefs_path(game_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(prefs).context("serialize editor prefs")?;
    fs::write(&path, text)?;
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

/// Mutate Game view fields and persist. `aspect`: `free` | `fixed`.
pub fn set_game_view(
    game_dir: &Path,
    width: Option<u32>,
    height: Option<u32>,
    aspect: Option<&str>,
    preset: Option<&str>,
    scale: Option<f32>,
) -> Result<EditorPrefs> {
    let mut prefs = load_editor_prefs(game_dir)?;
    apply_game_view(
        &mut prefs.game_view,
        width,
        height,
        aspect,
        preset,
        scale,
    )?;
    save_editor_prefs(game_dir, &prefs)?;
    Ok(prefs)
}

pub fn apply_game_view(
    gv: &mut GameViewPrefs,
    width: Option<u32>,
    height: Option<u32>,
    aspect: Option<&str>,
    preset: Option<&str>,
    scale: Option<f32>,
) -> Result<()> {
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
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wiimaker-prefs-{}-{}",
            label,
            std::process::id()
        ));
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
        set_game_view(&dir, Some(640), Some(480), Some("fixed"), None, Some(0.5)).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Res640x480);
        assert_eq!(p.game_view.aspect, GameViewAspect::Fixed);
        assert_eq!(p.game_view.width, 640);
        assert_eq!(p.game_view.height, 480);
        assert!((p.game_view.scale - 0.5).abs() < 1e-4);
        let text = fs::read_to_string(editor_prefs_path(&dir)).unwrap();
        assert!(text.contains("640x480") || text.contains("preset"));

        set_game_view(&dir, None, None, Some("free"), None, None).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Free);
        assert_eq!(p.game_view.aspect, GameViewAspect::Free);
    }

    #[test]
    fn preset_16_9_and_custom() {
        let dir = tmp("presets");
        set_game_view(&dir, None, None, None, Some("16:9"), None).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Ratio16x9);
        assert_eq!(p.game_view.width, 640);
        assert_eq!(p.game_view.height, 360);

        set_game_view(&dir, Some(800), Some(600), None, None, None).unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert_eq!(p.game_view.preset, GameViewPreset::Ratio4x3);
        set_game_view(&dir, Some(333), Some(111), None, None, None).unwrap();
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
        )
        .unwrap();
        let p = load_editor_prefs(&dir).unwrap();
        assert!((p.scene_view.zoom - 2.0).abs() < 1e-4);
        assert!(p.scene_view.grid_overlay);
        assert!(!p.scene_view.gizmos);
        assert!(p.scene_view.snap);
        assert!((p.scene_view.snap_size - 32.0).abs() < 1e-4);
        assert!(set_scene_view(&dir, None, None, None, None, None, None, None, Some(false)).is_err());
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
}
