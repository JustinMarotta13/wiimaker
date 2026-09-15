//! Tilemap scene mutations shared by CLI and editor.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::scene::{Scene, SceneAutoTile, SceneTilePalette, SceneTilemap};

fn find_mut<'a>(scene: &'a mut Scene, name: &str) -> Result<&'a mut crate::scene::EntityData> {
    scene
        .entities
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))
}

pub fn add_component_tilemap(
    scene: &mut Scene,
    name: &str,
    width: u32,
    height: u32,
    cell: f32,
) -> Result<()> {
    let ent = find_mut(scene, name)?;
    ent.components.tilemap = Some(SceneTilemap::new(width.max(1), height.max(1), cell));
    Ok(())
}

pub fn remove_component_tilemap(scene: &mut Scene, name: &str) -> Result<()> {
    let ent = find_mut(scene, name)?;
    if ent.components.tilemap.is_none() {
        bail!("entity '{name}' has no Tilemap");
    }
    ent.components.tilemap = None;
    Ok(())
}

/// Return a mutable tilemap, creating a default grid if missing.
pub fn ensure_tilemap<'a>(scene: &'a mut Scene, name: &str) -> Result<&'a mut SceneTilemap> {
    let ent = find_mut(scene, name)?;
    if ent.components.tilemap.is_none() {
        ent.components.tilemap = Some(SceneTilemap::default());
    }
    Ok(ent.components.tilemap.as_mut().unwrap())
}

pub fn tilemap_set_cell(
    scene: &mut Scene,
    name: &str,
    x: i32,
    y: i32,
    id: u16,
    solid: bool,
) -> Result<(u16, bool)> {
    let tm = ensure_tilemap(scene, name)?;
    tm.ensure_len();
    if !tm.in_bounds(x, y) {
        bail!(
            "cell ({x},{y}) out of bounds for '{name}' ({}x{})",
            tm.width,
            tm.height
        );
    }
    let prev = tm.get(x, y);
    tm.set(x, y, id, solid);
    Ok(prev)
}

pub fn tilemap_get_cell(scene: &Scene, name: &str, x: i32, y: i32) -> Result<(u16, bool)> {
    let ent = scene
        .find_entity(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let tm = ent
        .components
        .tilemap
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Tilemap"))?;
    if !tm.in_bounds(x, y) {
        bail!(
            "cell ({x},{y}) out of bounds for '{name}' ({}x{})",
            tm.width,
            tm.height
        );
    }
    Ok(tm.get(x, y))
}

pub fn tilemap_fill(
    scene: &mut Scene,
    name: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: u16,
    solid: bool,
) -> Result<u32> {
    if w <= 0 || h <= 0 {
        bail!("fill width/height must be positive");
    }
    let tm = ensure_tilemap(scene, name)?;
    tm.ensure_len();
    let mut n = 0u32;
    for cy in y..y.saturating_add(h) {
        for cx in x..x.saturating_add(w) {
            if tm.set(cx, cy, id, solid) {
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Stamp a row-major buffer of width `stamp_w`. `solid[i]` follows `id != 0` when `solid` is None.
pub fn tilemap_stamp(
    scene: &mut Scene,
    name: &str,
    x: i32,
    y: i32,
    stamp_w: u32,
    cells: &[u16],
    solid: Option<&[u8]>,
) -> Result<u32> {
    if stamp_w == 0 {
        bail!("stamp width must be > 0");
    }
    let tm = ensure_tilemap(scene, name)?;
    tm.ensure_len();
    let mut n = 0u32;
    for (i, &id) in cells.iter().enumerate() {
        let cx = x + (i as u32 % stamp_w) as i32;
        let cy = y + (i as u32 / stamp_w) as i32;
        let is_solid = match solid {
            Some(s) => s.get(i).copied().unwrap_or(0) != 0,
            None => id != 0,
        };
        if tm.set(cx, cy, id, is_solid) {
            n += 1;
        }
    }
    Ok(n)
}

/// Parsed ASCII maze: row-major `cells` + matching `solid` flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsciiTileMap {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u16>,
    pub solid: Vec<u8>,
}

/// Optional `CHAR=id` / `CHAR=id:solid` overrides for [`parse_ascii_tilemap_with`].
///
/// Default (no map): `#` → id 1 solid; `.` / space / `0` → empty; `1`–`9` → that id
/// solid; any other glyph → id 1 solid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AsciiCharMap {
    entries: Vec<(char, u16, bool)>,
}

impl AsciiCharMap {
    /// Parse `#,=1,.=0,P=2:0` (comma-separated). Solid defaults to `id != 0`.
    pub fn parse(spec: &str) -> Result<Self> {
        let mut entries = Vec::new();
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((ch_s, rest)) = part.split_once('=') else {
                bail!("ascii map '{part}' (expected CHAR=id or CHAR=id:solid)");
            };
            let ch_s = ch_s.trim();
            let mut chars = ch_s.chars();
            let Some(ch) = chars.next() else {
                bail!("ascii map '{part}' has empty character");
            };
            if chars.next().is_some() {
                bail!("ascii map '{part}' character must be a single char");
            }
            let rest = rest.trim();
            let (id_s, solid_s) = match rest.split_once(':') {
                Some((id_s, sol_s)) => (id_s.trim(), Some(sol_s.trim())),
                None => (rest, None),
            };
            let id: u16 = id_s
                .parse()
                .map_err(|_| anyhow::anyhow!("ascii map id '{id_s}'"))?;
            let solid = match solid_s {
                None => id != 0,
                Some("1") | Some("true") | Some("yes") | Some("solid") => true,
                Some("0") | Some("false") | Some("no") | Some("empty") => false,
                Some(s) => bail!("ascii map solid '{s}' (expected 0/1 or true/false)"),
            };
            if let Some(slot) = entries.iter_mut().find(|(c, _, _)| *c == ch) {
                *slot = (ch, id, solid);
            } else {
                entries.push((ch, id, solid));
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, ch: char) -> Option<(u16, bool)> {
        self.entries
            .iter()
            .find(|(c, _, _)| *c == ch)
            .map(|(_, id, solid)| (*id, *solid))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Default ASCII → (palette id, solid) used by stamp / from-ascii.
pub fn default_ascii_cell(ch: char) -> (u16, bool) {
    match ch {
        '#' => (1, true),
        '.' | ' ' | '0' => (0, false),
        '1'..='9' => {
            let id = (ch as u8 - b'0') as u16;
            (id, true)
        }
        _ => (1, true),
    }
}

fn ascii_cell(ch: char, map: Option<&AsciiCharMap>) -> (u16, bool) {
    if let Some(m) = map {
        if let Some(v) = m.get(ch) {
            return v;
        }
    }
    default_ascii_cell(ch)
}

/// Parse an ASCII maze into a rectangular cell buffer (short rows pad with empty).
pub fn parse_ascii_tilemap(ascii: &str) -> Result<AsciiTileMap> {
    parse_ascii_tilemap_with(ascii, None)
}

pub fn parse_ascii_tilemap_with(ascii: &str, map: Option<&AsciiCharMap>) -> Result<AsciiTileMap> {
    let ascii = ascii.strip_prefix('\u{feff}').unwrap_or(ascii);
    let mut rows: Vec<Vec<(u16, bool)>> = Vec::new();
    let mut row: Vec<(u16, bool)> = Vec::new();
    for ch in ascii.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            rows.push(row);
            row = Vec::new();
            continue;
        }
        row.push(ascii_cell(ch, map));
    }
    if !row.is_empty() {
        rows.push(row);
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
    if width == 0 {
        bail!("stamp ascii is empty");
    }
    let height = rows.len() as u32;
    let mut cells = Vec::with_capacity(width as usize * height as usize);
    let mut solid = Vec::with_capacity(cells.capacity());
    for mut r in rows {
        r.resize(width as usize, (0, false));
        for (id, is_solid) in r {
            cells.push(id);
            solid.push(if is_solid { 1 } else { 0 });
        }
    }
    Ok(AsciiTileMap {
        width,
        height,
        cells,
        solid,
    })
}

/// Stamp ASCII at `(x,y)` without resizing (clips out of bounds). Same glyphs as
/// [`parse_ascii_tilemap`].
pub fn tilemap_stamp_ascii(
    scene: &mut Scene,
    name: &str,
    x: i32,
    y: i32,
    ascii: &str,
) -> Result<u32> {
    Ok(tilemap_from_ascii(scene, name, ascii, x, y, false, None)?.stamped)
}

/// Result of [`tilemap_from_ascii`]: stamp size plus whether the grid was resized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilemapFromAscii {
    pub stamped: u32,
    pub width: u32,
    pub height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub resized: bool,
}

/// Load ASCII into a Tilemap. When `resize` is true (CLI `from-ascii` default), the
/// grid grows/shrinks so the stamp at `(x,y)` fits. `map` overrides glyph → id/solid.
pub fn tilemap_from_ascii(
    scene: &mut Scene,
    name: &str,
    ascii: &str,
    x: i32,
    y: i32,
    resize: bool,
    map: Option<&AsciiCharMap>,
) -> Result<TilemapFromAscii> {
    let parsed = parse_ascii_tilemap_with(ascii, map)?;
    let mut resized = false;
    if resize {
        let need_w = if x <= 0 {
            parsed.width
        } else {
            (x as u32).saturating_add(parsed.width)
        };
        let need_h = if y <= 0 {
            parsed.height
        } else {
            (y as u32).saturating_add(parsed.height)
        };
        let w = need_w.max(1);
        let h = need_h.max(1);
        let (cur_w, cur_h) = {
            let tm = ensure_tilemap(scene, name)?;
            (tm.width, tm.height)
        };
        if cur_w != w || cur_h != h {
            tilemap_resize(scene, name, w, h)?;
            resized = true;
        }
    }
    let stamped = tilemap_stamp(
        scene,
        name,
        x,
        y,
        parsed.width,
        &parsed.cells,
        Some(&parsed.solid),
    )?;
    Ok(TilemapFromAscii {
        stamped,
        width: parsed.width,
        height: parsed.height,
        origin_x: x,
        origin_y: y,
        resized,
    })
}

/// Read `path` as UTF-8 and [`tilemap_from_ascii`].
pub fn tilemap_from_ascii_path(
    scene: &mut Scene,
    name: &str,
    path: &Path,
    x: i32,
    y: i32,
    resize: bool,
    map: Option<&AsciiCharMap>,
) -> Result<TilemapFromAscii> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read ASCII map {}", path.display()))?;
    tilemap_from_ascii(scene, name, &text, x, y, resize, map)
}

pub fn tilemap_resize(scene: &mut Scene, name: &str, width: u32, height: u32) -> Result<()> {
    let tm = ensure_tilemap(scene, name)?;
    tm.resize(width.max(1), height.max(1));
    Ok(())
}

/// Optional fields for [`tilemap_set_palette`]. `None` leaves the current value.
#[derive(Clone, Debug, Default)]
pub struct TilePaletteOpts {
    pub sprite: Option<String>,
    pub color: Option<[u8; 4]>,
    pub anim: Option<String>,
    pub anim_fps: Option<f32>,
    pub auto_tile: Option<String>,
    pub auto_sprites: Option<Vec<String>>,
}

/// Create or update a palette entry on the named tilemap.
pub fn tilemap_set_palette(
    scene: &mut Scene,
    name: &str,
    id: u16,
    opts: &TilePaletteOpts,
) -> Result<SceneTilePalette> {
    if id == 0 {
        bail!("palette id 0 is reserved (empty cell)");
    }
    let tm = ensure_tilemap(scene, name)?;
    let pal = if let Some(existing) = tm.palette.iter_mut().find(|p| p.id == id) {
        existing
    } else {
        tm.palette.push(SceneTilePalette::new(id));
        tm.palette.last_mut().unwrap()
    };
    if let Some(sprite) = &opts.sprite {
        pal.sprite = if sprite.trim().is_empty() {
            None
        } else {
            Some(sprite.trim().to_string())
        };
    }
    if let Some(color) = opts.color {
        pal.color = color;
    }
    if let Some(anim) = &opts.anim {
        pal.anim = if anim.trim().is_empty() {
            None
        } else {
            Some(anim.trim().to_string())
        };
    }
    if let Some(fps) = opts.anim_fps {
        pal.anim_fps = if fps > 0.0 { Some(fps) } else { None };
    }
    if let Some(mode) = &opts.auto_tile {
        let trimmed = mode.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("off")
            || trimmed.eq_ignore_ascii_case("none")
            || trimmed.eq_ignore_ascii_case("false")
        {
            pal.auto_tile = None;
        } else {
            pal.auto_tile = Some(SceneAutoTile::parse(trimmed).ok_or_else(|| {
                anyhow::anyhow!("auto-tile '{trimmed}' (expected id, solid, or off)")
            })?);
        }
    }
    if let Some(sprites) = &opts.auto_sprites {
        pal.auto_sprites = sprites.clone();
    }
    Ok(pal.clone())
}

/// NESW bitmask (N=1 E=2 S=4 W=8) plus the palette rule used for cell `(x, y)`.
pub fn tilemap_autotile_mask(
    scene: &Scene,
    name: &str,
    x: i32,
    y: i32,
) -> Result<(u16, bool, u8, Option<SceneAutoTile>)> {
    let (id, solid) = tilemap_get_cell(scene, name, x, y)?;
    let ent = scene
        .find_entity(name)
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' not found"))?;
    let tm = ent
        .components
        .tilemap
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("entity '{name}' has no Tilemap"))?;
    let rule = tm
        .palette
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.auto_tile);
    let mask = if let Some(mode) = rule {
        tm.autotile_mask_mode(x, y, mode)
    } else {
        tm.autotile_mask_mode(x, y, SceneAutoTile::Id)
    };
    Ok((id, solid, mask, rule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_entity, MutateOpts};
    use crate::scene::{Scene, SceneAutoTile};

    fn scene_with_maze() -> Scene {
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        add_component_tilemap(&mut scene, "Maze", 5, 3, 16.0).unwrap();
        scene
    }

    #[test]
    fn set_get_and_solid() {
        let mut scene = scene_with_maze();
        tilemap_set_cell(&mut scene, "Maze", 2, 1, 1, true).unwrap();
        assert_eq!(tilemap_get_cell(&scene, "Maze", 2, 1).unwrap(), (1, true));
        tilemap_set_cell(&mut scene, "Maze", 2, 1, 0, false).unwrap();
        assert_eq!(tilemap_get_cell(&scene, "Maze", 2, 1).unwrap(), (0, false));
        assert!(tilemap_set_cell(&mut scene, "Maze", 9, 0, 1, true).is_err());
    }

    #[test]
    fn fill_and_stamp_ascii() {
        let mut scene = scene_with_maze();
        let n = tilemap_fill(&mut scene, "Maze", 0, 0, 5, 3, 1, true).unwrap();
        assert_eq!(n, 15);
        let carved = tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "#####\n#...#\n#####").unwrap();
        assert_eq!(carved, 15);
        assert_eq!(tilemap_get_cell(&scene, "Maze", 0, 0).unwrap(), (1, true));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 1, 1).unwrap(), (0, false));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 2, 1).unwrap(), (0, false));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 4, 1).unwrap(), (1, true));
    }

    #[test]
    fn json_roundtrip_preserves_cells() {
        let mut scene = scene_with_maze();
        tilemap_stamp_ascii(&mut scene, "Maze", 0, 0, "##.\n.#.").unwrap();
        let text = serde_json::to_string_pretty(&scene).unwrap();
        let loaded: Scene = serde_json::from_str(&text).unwrap();
        assert_eq!(tilemap_get_cell(&loaded, "Maze", 0, 0).unwrap(), (1, true));
        assert_eq!(tilemap_get_cell(&loaded, "Maze", 2, 0).unwrap(), (0, false));
        assert_eq!(tilemap_get_cell(&loaded, "Maze", 1, 1).unwrap(), (1, true));
        let tm = loaded
            .find_entity("Maze")
            .unwrap()
            .components
            .tilemap
            .as_ref()
            .unwrap();
        assert_eq!(tm.width, 5);
        assert_eq!(tm.cell, 16.0);
    }

    #[test]
    fn resize_keeps_overlap() {
        let mut scene = scene_with_maze();
        tilemap_set_cell(&mut scene, "Maze", 1, 1, 7, true).unwrap();
        tilemap_resize(&mut scene, "Maze", 8, 4).unwrap();
        assert_eq!(tilemap_get_cell(&scene, "Maze", 1, 1).unwrap(), (7, true));
        let tm = scene
            .find_entity("Maze")
            .unwrap()
            .components
            .tilemap
            .as_ref()
            .unwrap();
        assert_eq!((tm.width, tm.height), (8, 4));
        assert_eq!(tm.cells.len(), 32);
    }

    #[test]
    fn palette_anim_and_autotile_roundtrip() {
        let mut scene = scene_with_maze();
        tilemap_set_palette(
            &mut scene,
            "Maze",
            2,
            &TilePaletteOpts {
                sprite: Some("water".into()),
                anim: Some("water".into()),
                anim_fps: Some(8.0),
                auto_tile: Some("id".into()),
                auto_sprites: Some(vec!["water_0".into(), "water_1".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        tilemap_set_cell(&mut scene, "Maze", 1, 1, 2, false).unwrap();
        tilemap_set_cell(&mut scene, "Maze", 2, 1, 2, false).unwrap();
        let text = serde_json::to_string_pretty(&scene).unwrap();
        let loaded: Scene = serde_json::from_str(&text).unwrap();
        let pal = loaded
            .find_entity("Maze")
            .unwrap()
            .components
            .tilemap
            .as_ref()
            .unwrap()
            .palette
            .iter()
            .find(|p| p.id == 2)
            .unwrap();
        assert_eq!(pal.anim.as_deref(), Some("water"));
        assert_eq!(pal.anim_fps, Some(8.0));
        assert_eq!(pal.auto_tile, Some(SceneAutoTile::Id));
        assert_eq!(pal.auto_sprites, vec!["water_0", "water_1"]);
        let (id, solid, mask, rule) = tilemap_autotile_mask(&loaded, "Maze", 1, 1).unwrap();
        assert_eq!(id, 2);
        assert!(!solid);
        assert_eq!(rule, Some(SceneAutoTile::Id));
        // East neighbor is also id 2; N/S/W are not.
        assert_eq!(mask, wiimaker_core::tilemap::AUTOTILE_E);
    }

    #[test]
    fn autotile_solid_treats_oob_as_wall() {
        let mut scene = scene_with_maze();
        tilemap_set_palette(
            &mut scene,
            "Maze",
            1,
            &TilePaletteOpts {
                auto_tile: Some("solid".into()),
                ..Default::default()
            },
        )
        .unwrap();
        tilemap_fill(&mut scene, "Maze", 0, 0, 5, 3, 1, true).unwrap();
        tilemap_set_cell(&mut scene, "Maze", 1, 1, 0, false).unwrap();
        // Corner (0,0): N and W are OOB (solid), E and S are solid walls → mask 15.
        let mask = tilemap_autotile_mask(&scene, "Maze", 0, 0).unwrap().2;
        assert_eq!(mask, 15);
        // Cell east of the hole: W is open.
        let mask = tilemap_autotile_mask(&scene, "Maze", 2, 1).unwrap().2;
        assert_eq!(mask & wiimaker_core::tilemap::AUTOTILE_W, 0);
        assert_ne!(mask & wiimaker_core::tilemap::AUTOTILE_E, 0);
    }

    #[test]
    fn parse_ascii_default_glyphs() {
        let parsed = parse_ascii_tilemap("#####\n#...#\n#####").unwrap();
        assert_eq!(parsed.width, 5);
        assert_eq!(parsed.height, 3);
        assert_eq!(parsed.cells.len(), 15);
        assert_eq!(parsed.cells[0], 1);
        assert_eq!(parsed.solid[0], 1);
        assert_eq!(parsed.cells[6], 0); // (1,1)
        assert_eq!(parsed.solid[6], 0);
        assert_eq!(parsed.cells[8], 0); // (3,1)

        // digits + unknown glyph
        let mixed = parse_ascii_tilemap("#2.\nX 0").unwrap();
        assert_eq!(mixed.width, 3);
        assert_eq!(mixed.height, 2);
        assert_eq!(mixed.cells, vec![1, 2, 0, 1, 0, 0]);
        assert_eq!(mixed.solid, vec![1, 1, 0, 1, 0, 0]);
    }

    #[test]
    fn parse_ascii_custom_map_and_pad() {
        let map = AsciiCharMap::parse("#=1,.=0,P=2:0").unwrap();
        let parsed = parse_ascii_tilemap_with("#P\n.", Some(&map)).unwrap();
        assert_eq!(parsed.width, 2);
        assert_eq!(parsed.height, 2);
        assert_eq!(parsed.cells, vec![1, 2, 0, 0]);
        assert_eq!(parsed.solid, vec![1, 0, 0, 0]);
        assert!(parse_ascii_tilemap("").is_err());
        assert!(AsciiCharMap::parse("wall=1").is_err());
    }

    #[test]
    fn from_ascii_resizes_default_grid() {
        let mut scene = Scene::new("t");
        add_entity(
            &mut scene,
            "Maze",
            &MutateOpts {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
        )
        .unwrap();
        let out = tilemap_from_ascii(&mut scene, "Maze", "#####\n#...#\n#####", 0, 0, true, None)
            .unwrap();
        assert!(out.resized);
        assert_eq!((out.width, out.height, out.stamped), (5, 3, 15));
        let tm = scene
            .find_entity("Maze")
            .unwrap()
            .components
            .tilemap
            .as_ref()
            .unwrap();
        assert_eq!((tm.width, tm.height), (5, 3));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 0, 0).unwrap(), (1, true));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 1, 1).unwrap(), (0, false));
        assert_eq!(tilemap_get_cell(&scene, "Maze", 4, 2).unwrap(), (1, true));
    }
}
