//! Sprite sheet sidecar (`assets/<name>.sprites.json`) + catalog resolve.
//!
//! One PNG packs once; cells are named sub-rects with normalized pivots.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Pixel rect in source PNG space: `[x, y, w, h]`.
pub type PixelRect = [u32; 4];

/// Normalized pivot in cell space: `[0,0]` = top-left, `[0.5,0.5]` = center.
pub type Pivot = [f32; 2];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteSheetMeta {
    pub columns: u32,
    pub rows: u32,
    pub sprites: Vec<SpriteCell>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteCell {
    pub name: String,
    /// `[x, y, w, h]` in source PNG pixels.
    pub rect: PixelRect,
    /// Normalized pivot; default center.
    #[serde(default = "default_pivot")]
    pub pivot: Pivot,
}

fn default_pivot() -> Pivot {
    [0.5, 0.5]
}

impl SpriteSheetMeta {
    pub fn sidecar_path(png_or_stem: impl AsRef<Path>) -> PathBuf {
        let path = png_or_stem.as_ref();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sprite");
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(format!("{stem}.sprites.json"))
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

    /// Regenerate equal cell rects from cols/rows; preserve pivots for names that remain.
    pub fn slice_grid(
        &mut self,
        sheet_w: u32,
        sheet_h: u32,
        columns: u32,
        rows: u32,
        stem: &str,
    ) -> Result<Vec<String>> {
        if columns == 0 || rows == 0 {
            bail!("columns and rows must be >= 1");
        }
        let mut warnings = Vec::new();
        if sheet_w % columns != 0 || sheet_h % rows != 0 {
            warnings.push(format!(
                "sheet {sheet_w}x{sheet_h} does not divide evenly by {columns}x{rows}; cells use floor division"
            ));
        }
        let cell_w = sheet_w / columns;
        let cell_h = sheet_h / rows;
        if cell_w == 0 || cell_h == 0 {
            bail!("cell size is zero — image too small for {columns}x{rows}");
        }

        let old_pivots: HashMap<String, Pivot> = self
            .sprites
            .iter()
            .map(|s| (s.name.clone(), s.pivot))
            .collect();

        self.columns = columns;
        self.rows = rows;
        self.sprites.clear();
        let mut i = 0u32;
        for row in 0..rows {
            for col in 0..columns {
                let name = format!("{stem}_{i}");
                let pivot = old_pivots.get(&name).copied().unwrap_or_else(default_pivot);
                self.sprites.push(SpriteCell {
                    name,
                    rect: [col * cell_w, row * cell_h, cell_w, cell_h],
                    pivot,
                });
                i += 1;
            }
        }
        Ok(warnings)
    }

    pub fn set_pivot(&mut self, sprite_name: &str, pivot: Pivot) -> Result<()> {
        let cell = self
            .sprites
            .iter_mut()
            .find(|s| s.name == sprite_name)
            .ok_or_else(|| anyhow::anyhow!("sprite '{sprite_name}' not in sheet"))?;
        cell.pivot = pivot;
        Ok(())
    }
}

/// Equal cell rects for a grid (left→right, top→bottom). Shared by CLI + editor.
pub fn grid_by_cell_count(
    sheet_w: u32,
    sheet_h: u32,
    columns: u32,
    rows: u32,
) -> Result<Vec<PixelRect>> {
    if columns == 0 || rows == 0 {
        bail!("columns and rows must be >= 1");
    }
    let cell_w = sheet_w / columns;
    let cell_h = sheet_h / rows;
    if cell_w == 0 || cell_h == 0 {
        bail!("cell size is zero");
    }
    let mut rects = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows {
        for col in 0..columns {
            rects.push([col * cell_w, row * cell_h, cell_w, cell_h]);
        }
    }
    Ok(rects)
}

/// Resolved sprite for hydrate / bake / pick.
#[derive(Clone, Debug)]
pub struct ResolvedSprite {
    /// Wpack texture name (sheet stem).
    pub sheet_texture: String,
    /// Normalized UV rect over the **packed** texture: x, y, w, h.
    pub uv: [f32; 4],
    pub pivot: Pivot,
    /// Cell size in source pixels (before transform scale).
    pub pixel_size: [f32; 2],
    /// True when lookup name was a sheet cell (not the whole PNG).
    pub is_cell: bool,
}

/// Lookup table: whole textures + sheet cells.
#[derive(Clone, Debug, Default)]
pub struct SpriteCatalog {
    by_name: HashMap<String, ResolvedSprite>,
    names: Vec<String>,
}

impl SpriteCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn lookup(&self, name: &str) -> Option<&ResolvedSprite> {
        self.by_name.get(name)
    }

    /// Load every `*.png` under `assets_dir`, plus sidecar cells.
    ///
    /// `packed_size(name)` returns wpack dimensions when known (for PoT pad UV);
    /// otherwise `next_power_of_two` of content size.
    pub fn load_dir(
        assets_dir: &Path,
        mut packed_size: impl FnMut(&str) -> Option<(u32, u32)>,
    ) -> Result<Self> {
        let mut cat = Self::empty();
        if !assets_dir.is_dir() {
            return Ok(cat);
        }

        let mut pngs: Vec<PathBuf> = fs::read_dir(assets_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
            .collect();
        pngs.sort();

        for png in pngs {
            let stem = png
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("tex")
                .to_string();
            let (content_w, content_h) = image::image_dimensions(&png)
                .with_context(|| format!("dimensions {}", png.display()))?;
            let (pack_w, pack_h) = packed_size(&stem).unwrap_or_else(|| {
                (
                    content_w.next_power_of_two().max(1),
                    content_h.next_power_of_two().max(1),
                )
            });

            let content_uv_w = content_w as f32 / pack_w as f32;
            let content_uv_h = content_h as f32 / pack_h as f32;
            cat.push(
                stem.clone(),
                ResolvedSprite {
                    sheet_texture: stem.clone(),
                    uv: [0.0, 0.0, content_uv_w, content_uv_h],
                    pivot: default_pivot(),
                    pixel_size: [content_w as f32, content_h as f32],
                    is_cell: false,
                },
            );

            let sidecar = SpriteSheetMeta::sidecar_path(&png);
            if sidecar.is_file() {
                let meta = SpriteSheetMeta::load(&sidecar)?;
                for cell in &meta.sprites {
                    let [x, y, w, h] = cell.rect;
                    let uv = [
                        x as f32 / pack_w as f32,
                        y as f32 / pack_h as f32,
                        w as f32 / pack_w as f32,
                        h as f32 / pack_h as f32,
                    ];
                    cat.push(
                        cell.name.clone(),
                        ResolvedSprite {
                            sheet_texture: stem.clone(),
                            uv,
                            pivot: cell.pivot,
                            pixel_size: [w as f32, h as f32],
                            is_cell: true,
                        },
                    );
                }
            }
        }

        cat.names.sort();
        Ok(cat)
    }

    fn push(&mut self, name: String, resolved: ResolvedSprite) {
        if !self.by_name.contains_key(&name) {
            self.names.push(name.clone());
        }
        self.by_name.insert(name, resolved);
    }
}

/// Slice a sheet PNG: write/update `.sprites.json` with grid cells.
pub fn slice_sheet(
    assets_dir: &Path,
    sheet_stem: &str,
    columns: u32,
    rows: u32,
) -> Result<(PathBuf, SpriteSheetMeta, Vec<String>)> {
    let png = assets_dir.join(format!("{sheet_stem}.png"));
    if !png.is_file() {
        bail!("missing {}", png.display());
    }
    let (w, h) = image::image_dimensions(&png)?;
    let sidecar = SpriteSheetMeta::sidecar_path(&png);
    let mut meta = if sidecar.is_file() {
        SpriteSheetMeta::load(&sidecar)?
    } else {
        SpriteSheetMeta {
            columns: 1,
            rows: 1,
            sprites: Vec::new(),
        }
    };
    let warnings = meta.slice_grid(w, h, columns, rows, sheet_stem)?;
    meta.save(&sidecar)?;
    Ok((sidecar, meta, warnings))
}

/// Set pivot on a cell inside its sheet sidecar.
pub fn set_sprite_pivot(
    assets_dir: &Path,
    sprite_name: &str,
    pivot: Pivot,
) -> Result<PathBuf> {
    // Find which sidecar owns this sprite name.
    if !assets_dir.is_dir() {
        bail!("assets dir missing");
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !fname.ends_with(".sprites.json") {
            continue;
        }
        let mut meta = SpriteSheetMeta::load(&path)?;
        if meta.sprites.iter().any(|s| s.name == sprite_name) {
            meta.set_pivot(sprite_name, pivot)?;
            meta.save(&path)?;
            return Ok(path);
        }
    }
    bail!("sprite '{sprite_name}' not found in any .sprites.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_four_by_one() {
        let rects = grid_by_cell_count(64, 16, 4, 1).unwrap();
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0], [0, 0, 16, 16]);
        assert_eq!(rects[2], [32, 0, 16, 16]);
        assert_eq!(rects[3], [48, 0, 16, 16]);
    }

    #[test]
    fn slice_preserves_pivot_by_name() {
        let mut meta = SpriteSheetMeta {
            columns: 4,
            rows: 1,
            sprites: vec![SpriteCell {
                name: "sheet_2".into(),
                rect: [32, 0, 16, 16],
                pivot: [0.375, 0.375],
            }],
        };
        let _ = meta.slice_grid(64, 16, 4, 1, "sheet").unwrap();
        assert_eq!(meta.sprites.len(), 4);
        assert_eq!(meta.sprites[2].name, "sheet_2");
        assert_eq!(meta.sprites[2].pivot, [0.375, 0.375]);
        assert_eq!(meta.sprites[0].pivot, [0.5, 0.5]);
    }
}
