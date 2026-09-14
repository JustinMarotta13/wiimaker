//! Grid tilemap + solid-cell queries (Unity Tilemap analogue).

use crate::color::Rgba8;
use crate::draw::{Rect, TextureId};
use crate::math::Vec2;
use crate::sorting::default_sorting_layer_index;
use crate::world::{EntityId, Transform, World};

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// 4-neighbor auto-tile match (Unity RuleTile analogue).
///
/// Bitmask is NESW: North=`1`, East=`2`, South=`4`, West=`8` (cell `(0,0)` is top-left, +Y south).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoTileMatch {
    /// Neighbor matches when it has the same non-zero palette id (water blobs, same wall family).
    Id,
    /// Neighbor matches when it is solid. Out-of-bounds is solid (maze walls).
    Solid,
}

pub const AUTOTILE_N: u8 = 1;
pub const AUTOTILE_E: u8 = 2;
pub const AUTOTILE_S: u8 = 4;
pub const AUTOTILE_W: u8 = 8;

/// Pack NESW occupancy into a 0..=15 bitmask.
pub fn autotile_bits(north: bool, east: bool, south: bool, west: bool) -> u8 {
    (if north { AUTOTILE_N } else { 0 })
        | (if east { AUTOTILE_E } else { 0 })
        | (if south { AUTOTILE_S } else { 0 })
        | (if west { AUTOTILE_W } else { 0 })
}

/// One palette entry resolved at hydrate time.
#[derive(Clone, Debug)]
pub struct TileVisual {
    pub id: u16,
    /// Packed texture + UV when the palette names a sprite; otherwise a colored quad.
    /// Animated tiles overwrite this with the current frame.
    pub texture: Option<(TextureId, Rect)>,
    pub color: Rgba8,
    /// Resolved clip frames (Unity AnimatedTile). Length ≤ 1 means static.
    pub frames: Vec<(TextureId, Rect)>,
    pub fps: f32,
    pub time: f32,
    pub loop_: bool,
    pub frame: usize,
    /// When set, render picks `auto_frames[mask]` using NESW occupancy.
    pub auto_tile: Option<AutoTileMatch>,
    /// 16 variant textures indexed by [`autotile_bits`]. Missing → `texture`.
    pub auto_frames: [Option<(TextureId, Rect)>; 16],
}

impl TileVisual {
    pub fn color_only(id: u16, color: Rgba8) -> Self {
        Self {
            id,
            texture: None,
            color,
            frames: Vec::new(),
            fps: 0.0,
            time: 0.0,
            loop_: true,
            frame: 0,
            auto_tile: None,
            auto_frames: [None; 16],
        }
    }

    /// Advance clip time and set [`Self::texture`] to the current frame.
    /// Returns `true` when this entry is actually animated.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.frames.len() <= 1 || self.fps <= 0.0 {
            return false;
        }
        self.time += dt;
        let n = self.frames.len();
        let frame_dur = 1.0 / self.fps;
        let mut idx = (self.time / frame_dur) as usize;
        if self.loop_ {
            idx %= n;
            let cycle = frame_dur * n as f32;
            if cycle > 0.0 && self.time >= cycle {
                self.time %= cycle;
            }
        } else if idx >= n {
            idx = n - 1;
            self.time = frame_dur * n as f32;
        }
        self.frame = idx;
        self.texture = Some(self.frames[idx]);
        true
    }

    pub fn texture_for_mask(&self, mask: u8) -> Option<(TextureId, Rect)> {
        self.auto_frames
            .get(mask as usize)
            .copied()
            .flatten()
            .or(self.texture)
    }
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
    /// Index into [`crate::world::World::sorting_layers`].
    pub sorting_layer: u16,
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
            sorting_layer: default_sorting_layer_index(),
            palette: Vec::new(),
        }
    }

    /// Tick every animated palette entry. Returns `true` if any entry advanced.
    pub fn tick_anims(&mut self, dt: f32) -> bool {
        let mut any = false;
        for vis in &mut self.palette {
            if vis.tick(dt) {
                any = true;
            }
        }
        any
    }

    /// NESW bitmask for cell `(x, y)` using that cell's palette auto-tile mode
    /// (defaults to [`AutoTileMatch::Id`] when the palette has no rule).
    pub fn autotile_mask(&self, x: i32, y: i32) -> u8 {
        let id = self.get(x, y);
        let mode = self
            .visual_for(id)
            .and_then(|v| v.auto_tile)
            .unwrap_or(AutoTileMatch::Id);
        self.autotile_mask_mode(x, y, mode)
    }

    pub fn autotile_mask_mode(&self, x: i32, y: i32, mode: AutoTileMatch) -> u8 {
        let id = self.get(x, y);
        autotile_bits(
            self.neighbor_match(x, y - 1, id, mode),
            self.neighbor_match(x + 1, y, id, mode),
            self.neighbor_match(x, y + 1, id, mode),
            self.neighbor_match(x - 1, y, id, mode),
        )
    }

    fn neighbor_match(&self, nx: i32, ny: i32, id: u16, mode: AutoTileMatch) -> bool {
        match mode {
            AutoTileMatch::Id => self.in_bounds(nx, ny) && id != 0 && self.get(nx, ny) == id,
            AutoTileMatch::Solid => {
                if self.in_bounds(nx, ny) {
                    self.solid_at(nx, ny)
                } else {
                    true
                }
            }
        }
    }

    /// Texture used when drawing cell `(x, y)` (auto-tile variant or current anim frame).
    pub fn cell_texture(&self, x: i32, y: i32) -> Option<(TextureId, Rect)> {
        let id = self.get(x, y);
        let vis = self.visual_for(id)?;
        if vis.auto_tile.is_some() {
            vis.texture_for_mask(self.autotile_mask(x, y))
        } else {
            vis.texture
        }
    }

    pub fn cell_color(&self, x: i32, y: i32) -> Option<Rgba8> {
        let id = self.get(x, y);
        self.visual_for(id).map(|v| v.color)
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
    use crate::color::Rgba8;
    use crate::draw::{Rect, TextureId};
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

    #[test]
    fn autotile_id_nesw_bitmask() {
        let mut tm = Tilemap::new(3, 3, 16.0);
        for x in 0..3 {
            for y in 0..3 {
                tm.set(x, y, 1, true);
            }
        }
        tm.palette.push(TileVisual {
            auto_tile: Some(AutoTileMatch::Id),
            ..TileVisual::color_only(1, Rgba8::rgb(48, 88, 176))
        });
        // Center has all four neighbors.
        assert_eq!(tm.autotile_mask(1, 1), 15);
        // Top-middle: N missing, E/S/W present → 2|4|8 = 14
        assert_eq!(tm.autotile_mask(1, 0), AUTOTILE_E | AUTOTILE_S | AUTOTILE_W);
        tm.set(2, 1, 0, false);
        // Center without east neighbor.
        assert_eq!(tm.autotile_mask(1, 1), AUTOTILE_N | AUTOTILE_S | AUTOTILE_W);
    }

    #[test]
    fn autotile_solid_oob_is_solid() {
        let mut tm = Tilemap::new(2, 1, 16.0);
        tm.set(0, 0, 1, true);
        tm.set(1, 0, 0, false);
        tm.palette.push(TileVisual {
            auto_tile: Some(AutoTileMatch::Solid),
            ..TileVisual::color_only(1, Rgba8::rgb(48, 88, 176))
        });
        // (0,0): N/S/W OOB solid, E is empty → N|S|W = 1|4|8 = 13
        assert_eq!(
            tm.autotile_mask_mode(0, 0, AutoTileMatch::Solid),
            AUTOTILE_N | AUTOTILE_S | AUTOTILE_W
        );
    }

    #[test]
    fn tick_anims_advances_looping_frames() {
        let mut vis = TileVisual::color_only(2, Rgba8::rgb(20, 80, 200));
        vis.frames = vec![(TextureId(1), Rect::unit()), (TextureId(2), Rect::unit())];
        vis.fps = 10.0;
        vis.texture = Some(vis.frames[0]);
        let mut tm = Tilemap::new(1, 1, 16.0);
        tm.set(0, 0, 2, false);
        tm.palette.push(vis);
        assert_eq!(tm.palette[0].frame, 0);
        assert!(tm.tick_anims(0.11));
        assert_eq!(tm.palette[0].frame, 1);
        assert_eq!(tm.palette[0].texture.unwrap().0, TextureId(2));
        assert!(tm.tick_anims(0.11));
        assert_eq!(tm.palette[0].frame, 0);
    }
}
