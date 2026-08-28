//! Tilemap scene mutations shared by CLI and editor.

use anyhow::{bail, Result};

use crate::scene::{Scene, SceneTilemap};

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

/// Stamp ASCII: `#` = id 1 solid, `.` / space / `0` = empty, `1`-`9` = that id (solid).
pub fn tilemap_stamp_ascii(
    scene: &mut Scene,
    name: &str,
    x: i32,
    y: i32,
    ascii: &str,
) -> Result<u32> {
    let mut width = 0u32;
    let mut row_w = 0u32;
    for ch in ascii.chars() {
        if ch == '\n' {
            if row_w > width {
                width = row_w;
            }
            row_w = 0;
            continue;
        }
        if ch == '\r' {
            continue;
        }
        row_w += 1;
    }
    if row_w > width {
        width = row_w;
    }
    if width == 0 {
        bail!("stamp ascii is empty");
    }
    // rebuild as a rectangular buffer, padding short rows with 0
    let mut rows: Vec<Vec<u16>> = Vec::new();
    let mut row: Vec<u16> = Vec::new();
    for ch in ascii.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            rows.push(row);
            row = Vec::new();
            continue;
        }
        let id = match ch {
            '#' => 1,
            '.' | ' ' | '0' => 0,
            '1'..='9' => (ch as u8 - b'0') as u16,
            _ => 1,
        };
        row.push(id);
    }
    if !row.is_empty() || ascii.ends_with('\n') {
        if !row.is_empty() {
            rows.push(row);
        }
    }
    let mut flat = Vec::new();
    for r in &mut rows {
        r.resize(width as usize, 0);
        flat.extend_from_slice(r);
    }
    tilemap_stamp(scene, name, x, y, width, &flat, None)
}

pub fn tilemap_resize(scene: &mut Scene, name: &str, width: u32, height: u32) -> Result<()> {
    let tm = ensure_tilemap(scene, name)?;
    tm.resize(width.max(1), height.max(1));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_entity, MutateOpts};
    use crate::scene::Scene;

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
}
