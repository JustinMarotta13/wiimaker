//! Shared ship pipeline: prepare assets → bake → .dol → Dolphin.
//!
//! CLI and editor both use these helpers (editor shells `wiimaker build` /
//! `dolphin` / `play-wii` so Docker/make stay in one place).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use wiimaker_assets::{CookWarning, SpriteCatalog, WPack};
use wiimaker_scene::{
    find_game_dir, load_project, load_scene, write_scene_wscn_with_catalog, GameProject,
};

#[derive(Debug, Serialize)]
pub struct PrepareAssetsOut {
    pub output: String,
    pub textures: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BakeWiiOut {
    pub output: String,
    pub entities: usize,
    pub textures: usize,
}

#[derive(Debug, Serialize)]
pub struct BuildDolOut {
    pub dol: String,
    pub hbc: String,
}

/// Cook PNGs → `.wpack` for a game (project paths from `game.toml`).
pub fn prepare_assets(
    root: &Path,
    name: &str,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
) -> Result<PrepareAssetsOut> {
    if let (Some(input), Some(output)) = (&input, &output) {
        return cook_dir(input, output);
    }

    let game_dir = find_game_dir(root, name)?;
    let project = if game_dir.join("game.toml").is_file() {
        load_project(&game_dir)?
    } else {
        GameProject::new(name)
    };
    let assets = input.unwrap_or_else(|| project.assets_path(&game_dir));
    let out = output.unwrap_or_else(|| project.wpack_path(&game_dir));
    cook_dir(&assets, &out)
}

fn cook_dir(input: &Path, output: &Path) -> Result<PrepareAssetsOut> {
    let mut pack = WPack::new();
    let warnings = pack.cook_dir(input)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    pack.write_to(output)?;
    Ok(PrepareAssetsOut {
        output: output.display().to_string(),
        textures: pack.textures.len(),
        warnings: warning_msgs(&warnings),
    })
}

fn warning_msgs(warnings: &[CookWarning]) -> Vec<String> {
    warnings
        .iter()
        .map(|w| format!("{}: {}", w.texture, w.message))
        .collect()
}

/// Bake `scene.wscn` from the default scene + cooked `.wpack`.
pub fn bake_wii(root: &Path, name: &str) -> Result<BakeWiiOut> {
    let game_dir = find_game_dir(root, name)?;
    let project = load_project(&game_dir)?;
    let wpack_path = project.wpack_path(&game_dir);
    if !wpack_path.is_file() {
        bail!(
            "missing {} — prepare assets first (`wiimaker cook {name}`)",
            wpack_path.display()
        );
    }
    let pack = WPack::read_from(&wpack_path)?;
    let assets = project.assets_path(&game_dir);
    let catalog = SpriteCatalog::load_dir(&assets, |stem| {
        pack.textures
            .iter()
            .find(|t| t.name == stem)
            .map(|t| (t.width as u32, t.height as u32))
    })?;
    let scene = load_scene(&project.scene_path(&game_dir))?;
    let out = project.wscn_path(&game_dir);
    write_scene_wscn_with_catalog(&out, &scene, &pack, Some(&catalog))?;
    Ok(BakeWiiOut {
        output: out.display().to_string(),
        entities: scene.entities.len(),
        textures: pack.textures.len(),
    })
}

/// Full Wii `.dol`: prepare + bake + Docker/`wii-build.sh`.
///
/// `wii-build.sh` re-runs cook + bake before make; calling prepare first keeps
/// status messaging consistent and fails fast if assets are broken.
pub fn build_dol(root: &Path, name: &str) -> Result<BuildDolOut> {
    let _ = prepare_assets(root, name, None, None)?;
    let _ = bake_wii(root, name)?;

    let script = root.join("tools/wii-build.sh");
    let status = Command::new(&script)
        .arg(name)
        .current_dir(root)
        .status()
        .with_context(|| format!("run {}", script.display()))?;
    if !status.success() {
        bail!("Wii build failed");
    }

    let dol = root.join("target/wii").join(name).join("boot.dol");
    let hbc = root
        .join("target/wii")
        .join(name)
        .join("hbc")
        .join("apps")
        .join(name);
    if !dol.is_file() {
        bail!("expected {} after build", dol.display());
    }
    Ok(BuildDolOut {
        dol: dol.display().to_string(),
        hbc: hbc.display().to_string(),
    })
}

/// Launch existing `target/wii/<game>/boot.dol` in Dolphin.
pub fn run_dolphin(root: &Path, name: &str) -> Result<PathBuf> {
    let dol = root.join("target/wii").join(name).join("boot.dol");
    if !dol.is_file() {
        bail!(
            "missing {} — run `wiimaker build {name}` first",
            dol.display()
        );
    }
    let script = root.join("tools/run-dolphin.sh");
    let status = Command::new(&script)
        .arg(&dol)
        .current_dir(root)
        .status()
        .with_context(|| format!("run {}", script.display()))?;
    if !status.success() {
        bail!("Dolphin launch failed");
    }
    Ok(dol)
}

/// Build `.dol` then launch Dolphin.
pub fn play_wii(root: &Path, name: &str) -> Result<BuildDolOut> {
    let out = build_dol(root, name)?;
    let _ = run_dolphin(root, name)?;
    Ok(out)
}
