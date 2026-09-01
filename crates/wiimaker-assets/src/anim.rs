//! Animation clip sidecars (`assets/<name>.anim.json`).
//!
//! A clip lists sprite-catalog cell names and playback settings.
//! Sheets stay in `*.sprites.json`; clips only reference cell names.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

fn default_fps() -> f32 {
    10.0
}
fn default_true() -> bool {
    true
}

/// Authoring clip: cells are sprite catalog names (sheet cells or whole PNG stems).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimClipMeta {
    #[serde(default = "default_fps")]
    pub fps: f32,
    #[serde(rename = "loop", default = "default_true")]
    pub loop_: bool,
    pub cells: Vec<String>,
}

impl Default for AnimClipMeta {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            loop_: true,
            cells: Vec::new(),
        }
    }
}

impl AnimClipMeta {
    pub fn path(assets_dir: impl AsRef<Path>, name: &str) -> PathBuf {
        assets_dir.as_ref().join(format!("{name}.anim.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let meta: Self =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        Ok(meta)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text + "\n").with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}

/// Write / overwrite `assets/<name>.anim.json`.
pub fn write_anim_clip(
    assets_dir: &Path,
    name: &str,
    cells: Vec<String>,
    fps: f32,
    loop_: bool,
) -> Result<(PathBuf, AnimClipMeta)> {
    if name.is_empty() {
        bail!("anim clip name must not be empty");
    }
    if cells.is_empty() {
        bail!("anim clip must list at least one cell");
    }
    if fps <= 0.0 {
        bail!("fps must be > 0");
    }
    let meta = AnimClipMeta {
        fps,
        loop_,
        cells,
    };
    let path = AnimClipMeta::path(assets_dir, name);
    meta.save(&path)?;
    Ok((path, meta))
}

/// Stem names of every `*.anim.json` under `assets_dir`.
pub fn list_anim_clips(assets_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !assets_dir.is_dir() {
        return Ok(names);
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(stem) = fname.strip_suffix(".anim.json") {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Lookup table of animation clips by name.
#[derive(Clone, Debug, Default)]
pub struct AnimClipCatalog {
    by_name: HashMap<String, AnimClipMeta>,
    names: Vec<String>,
}

impl AnimClipCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn lookup(&self, name: &str) -> Option<&AnimClipMeta> {
        self.by_name.get(name)
    }

    pub fn load_dir(assets_dir: &Path) -> Result<Self> {
        let mut cat = Self::empty();
        if !assets_dir.is_dir() {
            return Ok(cat);
        }
        for name in list_anim_clips(assets_dir)? {
            let path = AnimClipMeta::path(assets_dir, &name);
            let meta = AnimClipMeta::load(&path)?;
            cat.names.push(name.clone());
            cat.by_name.insert(name, meta);
        }
        Ok(cat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn roundtrip_anim_json() {
        let dir = env::temp_dir().join(format!("wiimaker-anim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (path, meta) = write_anim_clip(
            &dir,
            "chomp",
            vec!["a".into(), "b".into()],
            12.0,
            true,
        )
        .unwrap();
        assert!(path.ends_with("chomp.anim.json"));
        assert_eq!(meta.cells.len(), 2);
        let loaded = AnimClipMeta::load(&path).unwrap();
        assert_eq!(loaded.fps, 12.0);
        assert!(loaded.loop_);
        assert_eq!(loaded.cells, vec!["a", "b"]);
        let names = list_anim_clips(&dir).unwrap();
        assert_eq!(names, vec!["chomp"]);
        let cat = AnimClipCatalog::load_dir(&dir).unwrap();
        assert!(cat.lookup("chomp").is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
