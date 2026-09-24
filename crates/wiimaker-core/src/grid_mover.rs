//! 4-way discrete grid-snap mover (Pac-Man-style queued cardinals).
//!
//! Input: existing [`Input`] D-pad, else main stick. Diagonals never apply:
//! **when both axes are held, the horizontal axis wins** (Left/Right over Up/Down).
//! Stick +Y is host Up and maps to [`Dir::Up`] (−Y in this engine's Y-down world).
//!
//! Reverse of the current heading is applied immediately. A 90° queued turn
//! waits until the entity is on a cell center, then snaps and proceeds.

use crate::input::{Button, Input};
use crate::tilemap::tile_solid_world;
use crate::world::{EntityId, World};

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Cardinal heading in world space (Y-down: [`Dir::Up`] is −Y).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn opposite(self) -> Self {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    /// Unit step in world pixels per cell.
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Dir::Up => "Up",
            Dir::Down => "Down",
            Dir::Left => "Left",
            Dir::Right => "Right",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "up" => Some(Dir::Up),
            "down" => Some(Dir::Down),
            "left" => Some(Dir::Left),
            "right" => Some(Dir::Right),
            _ => None,
        }
    }
}

/// Stick / D-pad threshold for treating analog as a cardinal.
pub const CARDINAL_DEADZONE: f32 = 0.5;

/// Optional grid-snap mover. `queued_dir` is the pending 90° turn.
#[derive(Clone, Debug)]
pub struct GridMover {
    /// World-unit cell size. Centers sit at `(i + 0.5) * cell`.
    pub cell: f32,
    /// World units per second along the current cardinal.
    pub speed: f32,
    /// Pending turn (90°). Reverse does not wait here.
    pub queued_dir: Option<Dir>,
    /// Heading we are currently traveling. Runtime-only (not authored).
    pub current_dir: Option<Dir>,
}

impl GridMover {
    pub fn new(cell: f32, speed: f32) -> Self {
        Self {
            cell: if cell <= 0.0 { 16.0 } else { cell },
            speed: speed.max(0.0),
            queued_dir: None,
            current_dir: None,
        }
    }

    /// Fold a frame of pad input into queue / immediate reverse.
    pub fn ingest_input(&mut self, input: &Input) {
        let Some(want) = cardinal(input) else {
            return;
        };
        match self.current_dir {
            Some(cur) if cur.opposite() == want => {
                self.current_dir = Some(want);
                self.queued_dir = None;
            }
            Some(cur) if cur == want => {}
            Some(_) => self.queued_dir = Some(want),
            None => self.queued_dir = Some(want),
        }
    }

    /// Move `(x, y)` toward cell centers. `is_solid(wx, wy)` blocks entering a cell.
    pub fn try_step(&mut self, x: &mut f32, y: &mut f32, dt: f32, is_solid: impl Fn(f32, f32) -> bool) {
        let cell = self.cell.max(1e-4);
        let mut remaining = self.speed.max(0.0) * dt.max(0.0);
        for _ in 0..6 {
            if remaining <= 1e-8 {
                break;
            }
            let cx = cell_center(*x, cell);
            let cy = cell_center(*y, cell);
            let on_center = (*x - cx).abs() <= 1e-3 && (*y - cy).abs() <= 1e-3;
            if on_center {
                *x = cx;
                *y = cy;
                if let Some(q) = self.queued_dir {
                    if dir_open(cx, cy, cell, q, &is_solid) {
                        self.current_dir = Some(q);
                        self.queued_dir = None;
                    }
                }
                if let Some(cur) = self.current_dir {
                    if !dir_open(cx, cy, cell, cur, &is_solid) {
                        self.current_dir = None;
                    }
                }
            }

            if self.current_dir.is_none() && !on_center {
                let dist = crate::float::sqrt(crate::float::powi2(*x - cx) + crate::float::powi2(*y - cy));
                if dist <= remaining {
                    *x = cx;
                    *y = cy;
                    remaining -= dist;
                    continue;
                }
                let s = remaining / dist.max(1e-8);
                *x += (cx - *x) * s;
                *y += (cy - *y) * s;
                break;
            }

            let Some(dir) = self.current_dir else {
                break;
            };
            match dir {
                Dir::Left | Dir::Right => *y = cell_center(*y, cell),
                Dir::Up | Dir::Down => *x = cell_center(*x, cell),
            }
            let (tx, ty) = next_center(*x, *y, dir, cell);
            if is_solid(tx, ty) && on_center {
                self.current_dir = None;
                break;
            }
            let dist = crate::float::sqrt(crate::float::powi2(*x - tx) + crate::float::powi2(*y - ty));
            if dist <= remaining {
                *x = tx;
                *y = ty;
                remaining -= dist;
            } else {
                let s = remaining / dist.max(1e-8);
                *x += (tx - *x) * s;
                *y += (ty - *y) * s;
                remaining = 0.0;
            }
        }
    }
}

/// D-pad if any cardinal is down, otherwise main stick past [`CARDINAL_DEADZONE`].
/// Both axes: **horizontal wins**.
pub fn cardinal(input: &Input) -> Option<Dir> {
    let dpad_x = match (
        input.down(Button::DPadLeft),
        input.down(Button::DPadRight),
    ) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    let dpad_y = match (input.down(Button::DPadUp), input.down(Button::DPadDown)) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    if dpad_x != 0 || dpad_y != 0 {
        return dir_from_axes(dpad_x, dpad_y);
    }
    let s = input.main.deadzone(CARDINAL_DEADZONE);
    let sx = if s.x > CARDINAL_DEADZONE {
        1
    } else if s.x < -CARDINAL_DEADZONE {
        -1
    } else {
        0
    };
    // Host stick +Y is Up → world −Y.
    let sy = if s.y > CARDINAL_DEADZONE {
        -1
    } else if s.y < -CARDINAL_DEADZONE {
        1
    } else {
        0
    };
    dir_from_axes(sx, sy)
}

fn dir_from_axes(x: i32, y: i32) -> Option<Dir> {
    if x != 0 {
        return Some(if x > 0 { Dir::Right } else { Dir::Left });
    }
    if y != 0 {
        return Some(if y > 0 { Dir::Down } else { Dir::Up });
    }
    None
}

/// Cell-center coordinate on one axis (grid origin at 0).
pub fn cell_center(p: f32, cell: f32) -> f32 {
    let cell = cell.max(1e-4);
    crate::float::floor(p / cell) * cell + cell * 0.5
}

fn next_center(x: f32, y: f32, dir: Dir, cell: f32) -> (f32, f32) {
    let cx = cell_center(x, cell);
    let cy = cell_center(y, cell);
    match dir {
        Dir::Right => {
            if x < cx - 1e-4 {
                (cx, cy)
            } else {
                (cx + cell, cy)
            }
        }
        Dir::Left => {
            if x > cx + 1e-4 {
                (cx, cy)
            } else {
                (cx - cell, cy)
            }
        }
        Dir::Down => {
            if y < cy - 1e-4 {
                (cx, cy)
            } else {
                (cx, cy + cell)
            }
        }
        Dir::Up => {
            if y > cy + 1e-4 {
                (cx, cy)
            } else {
                (cx, cy - cell)
            }
        }
    }
}

fn dir_open(cx: f32, cy: f32, cell: f32, dir: Dir, is_solid: &impl Fn(f32, f32) -> bool) -> bool {
    let (dx, dy) = dir.delta();
    !is_solid(cx + dx as f32 * cell, cy + dy as f32 * cell)
}

/// Tick every live [`GridMover`]. Uses [`tile_solid_world`] when a tilemap exists.
pub fn step_grid_movers(world: &mut World, input: &Input, dt: f32) {
    let ids: Vec<EntityId> = world
        .iter_entities()
        .filter(|id| world.grid_mover(*id).is_some())
        .collect();
    for id in ids {
        let Some(mut gm) = world.grid_mover(id).cloned() else {
            continue;
        };
        let Some(xf) = world.transform(id) else {
            continue;
        };
        let mut x = xf.translation.x;
        let mut y = xf.translation.y;
        gm.ingest_input(input);
        gm.try_step(&mut x, &mut y, dt, |wx, wy| tile_solid_world(world, wx, wy));
        if let Some(xf) = world.transform_mut(id) {
            xf.translation.x = x;
            xf.translation.y = y;
        }
        world.set_grid_mover(id, Some(gm));
    }
}

impl World {
    /// See [`step_grid_movers`].
    pub fn step_grid_movers(&mut self, input: &Input, dt: f32) {
        step_grid_movers(self, input, dt);
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::math::Vec2;
    use crate::tilemap::Tilemap;
    use crate::world::Transform;
    use crate::Stick;

    fn pad(up: bool, down: bool, left: bool, right: bool) -> Input {
        let mut i = Input::new();
        i.set_down(Button::DPadUp, up);
        i.set_down(Button::DPadDown, down);
        i.set_down(Button::DPadLeft, left);
        i.set_down(Button::DPadRight, right);
        i
    }

    #[test]
    fn up_plus_right_is_horizontal_only() {
        let i = pad(true, false, false, true);
        assert_eq!(cardinal(&i), Some(Dir::Right));
        let i = pad(true, false, true, false);
        assert_eq!(cardinal(&i), Some(Dir::Left));
    }

    #[test]
    fn stick_diagonal_horizontal_wins() {
        let mut i = Input::new();
        i.main = Stick { x: 1.0, y: 1.0 };
        assert_eq!(cardinal(&i), Some(Dir::Right));
    }

    #[test]
    fn stick_up_is_world_negative_y() {
        let mut i = Input::new();
        i.main = Stick { x: 0.0, y: 1.0 };
        assert_eq!(cardinal(&i), Some(Dir::Up));
        assert_eq!(Dir::Up.delta(), (0, -1));
    }

    #[test]
    fn reverse_applies_immediately_off_center() {
        let mut gm = GridMover::new(20.0, 100.0);
        gm.current_dir = Some(Dir::Right);
        let mut x = 35.0; // between centers 30 and 50
        let mut y = 30.0;
        gm.ingest_input(&pad(false, false, true, false));
        assert_eq!(gm.current_dir, Some(Dir::Left));
        assert_eq!(gm.queued_dir, None);
        gm.try_step(&mut x, &mut y, 0.05, |_, _| false);
        assert!(x < 35.0, "should move left toward 30, got {x}");
        assert!((y - 30.0).abs() < 1e-3);
    }

    #[test]
    fn snap_lands_on_cell_center() {
        let mut gm = GridMover::new(20.0, 400.0);
        gm.queued_dir = Some(Dir::Right);
        let mut x = 30.0;
        let mut y = 30.0;
        // 20 units at 400 u/s → 0.05s reaches next center 50
        gm.try_step(&mut x, &mut y, 0.05, |_, _| false);
        assert!((x - 50.0).abs() < 1e-3, "x={x}");
        assert!((y - 30.0).abs() < 1e-3);
        assert_eq!(gm.current_dir, Some(Dir::Right));
    }

    #[test]
    fn ninety_degree_turn_waits_for_center() {
        let mut gm = GridMover::new(20.0, 200.0);
        gm.current_dir = Some(Dir::Right);
        gm.queued_dir = Some(Dir::Down);
        let mut x = 35.0;
        let mut y = 30.0;
        gm.try_step(&mut x, &mut y, 0.01, |_, _| false); // 2px right, still off-center
        assert!(x > 35.0 && x < 50.0);
        assert!((y - 30.0).abs() < 1e-3);
        assert_eq!(gm.current_dir, Some(Dir::Right));
        assert_eq!(gm.queued_dir, Some(Dir::Down));
    }

    #[test]
    fn step_grid_movers_moves_named_entity() {
        let mut world = World::new();
        let id = world.spawn_named("Player", Transform::from_xy(30.0, 30.0));
        world.set_grid_mover(id, Some(GridMover::new(20.0, 400.0)));
        world.step_grid_movers(&pad(false, false, false, true), 0.05);
        let t = world.transform(id).unwrap().translation;
        assert!((t.x - 50.0).abs() < 1e-3, "x={}", t.x);
        assert!((t.y - 30.0).abs() < 1e-3);
    }

    #[test]
    fn tile_wall_blocks_forward() {
        let mut world = World::new();
        let maze = world.spawn_named("Maze", Transform::from_xy(0.0, 0.0));
        let mut tm = Tilemap::new(4, 4, 20.0);
        tm.set(2, 1, 1, true); // wall at cell whose center is (50, 30)
        world.set_tilemap(maze, Some(tm));
        let id = world.spawn_named("P", Transform::from_xy(30.0, 30.0));
        world.set_grid_mover(id, Some(GridMover::new(20.0, 400.0)));
        world.step_grid_movers(&pad(false, false, false, true), 0.05);
        let t = world.transform(id).unwrap().translation;
        assert!((t.x - 30.0).abs() < 1e-3, "blocked, x={}", t.x);
        let _ = Vec2::ZERO;
    }
}
