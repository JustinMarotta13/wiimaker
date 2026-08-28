//! Grid tilemap + solid-cell queries (Unity Tilemap analogue).

use crate::color::Rgba8;
use crate::draw::{Rect, TextureId};
use crate::math::Vec2;
use crate::world::{EntityId, Transform, World};

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// One palette entry resolved at hydrate time.
#[derive(Clone, Copy, Debug)]
pub struct TileVisual {
    pub id: u16,
    /// Packed texture + UV when the palette names a sprite; otherwise a colored quad.
    pub texture: Option<(TextureId, Rect)>,
    pub color: Rgba8,
}

/// Grid of cell ids + packed solid bits, in the entity's local space.
///
/// Cell `(cx, cy)` covers
/// `[origin + (cx,cy)*cell, origin + (cx+1,cy+1)*cell)` after the entity transform.
#[derive(Clone, Debug)]
pub struct Tilemap {
    pub cell: f32,
    pub origin: Vec2,
    pub width: u32,
    pub height: u32,
    /// Row-major `width * height` cell ids. `0` = empty.
    pub cells: Vec<u16>,
    /// Bit-packed solid flags, row-major, same length as `cells`.
    pub solid: Vec<u8>,
    pub z: f32,
    /// Palette used at render time (id 0 is never drawn).
    pub palette: Vec<TileVisual>,
}

impl Tilemap {
    pub fn new(width: u32, height: u32, cell: f32) -> Self {
        let n = (width as usize).saturating_mul(height as usize);
        Self {
            cell: if cell <= 0.0 { 16.0 } else { cell },
            origin: Vec2::ZERO,
            width,
            height,
            cells: vec![0; n],
            solid: vec![0; (n + 7) / 8],
            z: -1.0,
            palette: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some(y as usize * self.width as usize + x as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u16 {
        self.index(x, y)
            .and_then(|i| self.cells.get(i).copied())
            .unwrap_or(0)
    }

    pub fn solid_at(&self, x: i32, y: i32) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        let byte = i / 8;
        let bit = i % 8;
        self.solid
            .get(byte)
            .map(|b| b & (1 << bit) != 0)
            .unwrap_or(false)
    }

    pub fn set(&mut self, x: i32, y: i32, id: u16, solid: bool) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        if i >= self.cells.len() {
            return false;
        }
        self.cells[i] = id;
        let byte = i / 8;
        let bit = i % 8;
        if byte >= self.solid.len() {
            self.solid.resize(byte + 1, 0);
        }
        if solid {
            self.solid[byte] |= 1 << bit;
        } else {
            self.solid[byte] &= !(1 << bit);
        }
        true
    }

    /// Convert a world-space point into cell coordinates (may be out of bounds).
    pub fn world_to_cell(&self, xf: &Transform, wx: f32, wy: f32) -> (i32, i32) {
        let cell_x = (self.cell * xf.scale.x).abs().max(1e-6);
        let cell_y = (self.cell * xf.scale.y).abs().max(1e-6);
        let ox = xf.translation.x + self.origin.x * xf.scale.x;
        let oy = xf.translation.y + self.origin.y * xf.scale.y;
        let cx = ((wx - ox) / cell_x).floor() as i32;
        let cy = ((wy - oy) / cell_y).floor() as i32;
        (cx, cy)
    }

    /// Top-left of cell `(cx, cy)` in world space.
    pub fn cell_to_world(&self, xf: &Transform, cx: i32, cy: i32) -> Vec2 {
        Vec2::new(
            xf.translation.x + self.origin.x * xf.scale.x + cx as f32 * self.cell * xf.scale.x,
            xf.translation.y + self.origin.y * xf.scale.y + cy as f32 * self.cell * xf.scale.y,
        )
    }

    pub fn contains_world(&self, xf: &Transform, wx: f32, wy: f32) -> bool {
        let (cx, cy) = self.world_to_cell(xf, wx, wy);
        self.in_bounds(cx, cy)
    }

    /// World-space AABB of the whole grid (top-left, size).
    pub fn world_bounds(&self, xf: &Transform) -> (Vec2, Vec2) {
        let origin = self.cell_to_world(xf, 0, 0);
        let size = Vec2::new(
            self.width as f32 * self.cell * xf.scale.x,
            self.height as f32 * self.cell * xf.scale.y,
        );
        (origin, size)
    }

    pub fn visual_for(&self, id: u16) -> Option<&TileVisual> {
        if id == 0 {
            return None;
        }
        self.palette
            .iter()
            .find(|v| v.id == id)
            .or_else(|| self.palette.first())
    }
}

/// Cell coordinates of the first tilemap under `(wx, wy)`.
///
/// Returned coords may be out of that grid's bounds (walker can still query neighbors).
pub fn world_to_cell(world: &World, wx: f32, wy: f32) -> Option<(i32, i32)> {
    world_to_cell_on(world, wx, wy).map(|(_, cx, cy)| (cx, cy))
}

/// Like [`world_to_cell`] but includes the tilemap entity.
pub fn world_to_cell_on(world: &World, wx: f32, wy: f32) -> Option<(EntityId, i32, i32)> {
    let mut fallback: Option<(EntityId, i32, i32)> = None;
    for (id, xf, tm) in world.iter_tilemaps() {
        let (cx, cy) = tm.world_to_cell(xf, wx, wy);
        if tm.in_bounds(cx, cy) {
            return Some((id, cx, cy));
        }
        if fallback.is_none() {
            fallback = Some((id, cx, cy));
        }
    }
    fallback
}

/// `true` if cell `(x, y)` is solid on any tilemap.
///
/// Out-of-bounds cells are solid when at least one tilemap exists (maze walls).
/// With no tilemaps, returns `false` so existing non-grid games stay open.
pub fn tile_solid(world: &World, x: i32, y: i32) -> bool {
    let mut any = false;
    let mut in_grid = false;
    for (_id, _xf, tm) in world.iter_tilemaps() {
        any = true;
        if tm.in_bounds(x, y) {
            in_grid = true;
            if tm.solid_at(x, y) {
                return true;
            }
        }
    }
    if any && !in_grid {
        true
    } else {
        false
    }
}

/// Solid query in world space (walker-friendly). Missing tilemaps → not solid.
pub fn tile_solid_world(world: &World, wx: f32, wy: f32) -> bool {
    match world_to_cell_on(world, wx, wy) {
        Some((_id, cx, cy)) => tile_solid(world, cx, cy),
        None => false,
    }
}

/// Cell id at `(x, y)` on the first tilemap that contains it, or `0`.
pub fn tile_get(world: &World, x: i32, y: i32) -> u16 {
    for (_id, _xf, tm) in world.iter_tilemaps() {
        if tm.in_bounds(x, y) {
            return tm.get(x, y);
        }
    }
    0
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::world::Transform;

    fn maze() -> (World, EntityId) {
        let mut world = World::new();
        let id = world.spawn_named("Maze", Transform::from_xy(0.0, 0.0));
        let mut tm = Tilemap::new(5, 3, 16.0);
        // corridor along y=1: open at (1,1) (2,1) (3,1)
        for x in 0..5 {
            for y in 0..3 {
                let wall = y != 1 || x == 0 || x == 4;
                tm.set(x, y, if wall { 1 } else { 0 }, wall);
            }
        }
        world.set_tilemap(id, Some(tm));
        (world, id)
    }

    #[test]
    fn set_get_solid() {
        let (world, id) = maze();
        let tm = world.tilemap(id).unwrap();
        assert_eq!(tm.get(0, 0), 1);
        assert!(tm.solid_at(0, 0));
        assert_eq!(tm.get(2, 1), 0);
        assert!(!tm.solid_at(2, 1));
        assert_eq!(tm.get(9, 9), 0);
        assert!(!tm.solid_at(-1, 0));
    }

    #[test]
    fn world_to_cell_rounds_down() {
        let (world, _id) = maze();
        assert_eq!(world_to_cell(&world, 0.0, 0.0), Some((0, 0)));
        assert_eq!(world_to_cell(&world, 15.9, 0.1), Some((0, 0)));
        assert_eq!(world_to_cell(&world, 16.0, 16.0), Some((1, 1)));
        assert_eq!(world_to_cell(&world, 40.0, 20.0), Some((2, 1)));
        // center of cell (2,1)
        assert_eq!(world_to_cell(&world, 32.0 + 8.0, 16.0 + 8.0), Some((2, 1)));
    }

    #[test]
    fn tile_solid_blocks_walls_and_oob() {
        let (world, _id) = maze();
        assert!(tile_solid(&world, 0, 0));
        assert!(tile_solid(&world, 0, 1));
        assert!(!tile_solid(&world, 1, 1));
        assert!(!tile_solid(&world, 2, 1));
        assert!(tile_solid(&world, 4, 1));
        // out of bounds is solid when a tilemap exists
        assert!(tile_solid(&world, -1, 1));
        assert!(tile_solid(&world, 5, 1));
        assert!(tile_solid(&world, 2, 9));
    }

    #[test]
    fn walker_cannot_enter_solid() {
        let (world, id) = maze();
        let tm = world.tilemap(id).unwrap();
        let xf = world.transform(id).unwrap();
        // stand in corridor cell (2,1) center
        let pos = tm.cell_to_world(xf, 2, 1);
        let center = Vec2::new(pos.x + 8.0, pos.y + 8.0);
        assert!(!tile_solid_world(&world, center.x, center.y));
        // step left into (1,1) — open
        assert!(!tile_solid_world(&world, center.x - 16.0, center.y));
        // step up into (2,0) — wall
        assert!(tile_solid_world(&world, center.x, center.y - 16.0));
        // step right toward (4,1) wall from (3,1)
        let at3 = tm.cell_to_world(xf, 3, 1);
        assert!(tile_solid_world(&world, at3.x + 16.0 + 8.0, at3.y + 8.0));
    }

    #[test]
    fn empty_world_is_not_solid() {
        let world = World::new();
        assert!(!tile_solid(&world, 0, 0));
        assert_eq!(world_to_cell(&world, 10.0, 10.0), None);
        assert!(!tile_solid_world(&world, 10.0, 10.0));
    }

    #[test]
    fn origin_and_scale_shift_cells() {
        let mut world = World::new();
        let mut xf = Transform::from_xy(100.0, 50.0);
        xf.scale.x = 2.0;
        xf.scale.y = 2.0;
        let id = world.spawn_named("Maze", xf);
        let mut tm = Tilemap::new(4, 4, 10.0);
        tm.origin = Vec2::new(5.0, 0.0);
        tm.set(1, 0, 1, true);
        world.set_tilemap(id, Some(tm));
        // cell (0,0) starts at 100 + 5*2 = 110, size 20
        assert_eq!(world_to_cell(&world, 110.0, 50.0), Some((0, 0)));
        assert_eq!(world_to_cell(&world, 130.0, 50.0), Some((1, 0)));
        assert!(tile_solid(&world, 1, 0));
        assert!(!tile_solid(&world, 0, 0));
    }
}
