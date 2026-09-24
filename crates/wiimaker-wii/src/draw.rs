//! Flush loaded WSCN entities through GX (or the host test recorder).

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::ffi;
use crate::wscn::{
    rgba_pack, Entity, TilePal, WiiTilemap, KIND_DISC, KIND_SPRITE, KIND_TEXT, KIND_TILEMAP,
    TILE_AUTO_OFF, TILE_AUTO_SOLID, TILE_NO_TEX,
};

fn tm_in_bounds(tm: &WiiTilemap, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < tm.w as i32 && y < tm.h as i32
}

fn tm_cell(tm: &WiiTilemap, x: i32, y: i32) -> u16 {
    if !tm_in_bounds(tm, x, y) || tm.cells.is_empty() {
        return 0;
    }
    tm.cells[(y as u32 * tm.w as u32 + x as u32) as usize]
}

fn tm_solid_at(tm: &WiiTilemap, x: i32, y: i32) -> bool {
    if !tm_in_bounds(tm, x, y) || tm.solid.is_empty() {
        return false;
    }
    let i = (y as u32 * tm.w as u32 + x as u32) as usize;
    (tm.solid[i / 8] >> (i % 8)) & 1 != 0
}

fn tm_autotile_mask(tm: &WiiTilemap, x: i32, y: i32, id: u16, mode: u8) -> u8 {
    let (n, e, s, w) = if mode == TILE_AUTO_SOLID {
        (
            !tm_in_bounds(tm, x, y - 1) || tm_solid_at(tm, x, y - 1),
            !tm_in_bounds(tm, x + 1, y) || tm_solid_at(tm, x + 1, y),
            !tm_in_bounds(tm, x, y + 1) || tm_solid_at(tm, x, y + 1),
            !tm_in_bounds(tm, x - 1, y) || tm_solid_at(tm, x - 1, y),
        )
    } else {
        (
            tm_in_bounds(tm, x, y - 1) && id != 0 && tm_cell(tm, x, y - 1) == id,
            tm_in_bounds(tm, x + 1, y) && id != 0 && tm_cell(tm, x + 1, y) == id,
            tm_in_bounds(tm, x, y + 1) && id != 0 && tm_cell(tm, x, y + 1) == id,
            tm_in_bounds(tm, x - 1, y) && id != 0 && tm_cell(tm, x - 1, y) == id,
        )
    };
    (if n { 1 } else { 0 })
        | (if e { 2 } else { 0 })
        | (if s { 4 } else { 0 })
        | (if w { 8 } else { 0 })
}

fn tm_pal_for(tm: &WiiTilemap, id: u16) -> Option<&TilePal> {
    tm.pal
        .iter()
        .find(|p| p.id == id)
        .or_else(|| tm.pal.first())
}

/// Advance tilemap palette animation clocks.
pub fn tick_tilemaps(entities: &mut [Entity], dt: f32) {
    for e in entities.iter_mut() {
        if e.kind != KIND_TILEMAP {
            continue;
        }
        let Some(tm) = e.tm.as_mut() else { continue };
        for pal in tm.pal.iter_mut() {
            if pal.frame_n <= 1 || pal.fps <= 0.0 {
                continue;
            }
            pal.time += dt;
            let dur = 1.0 / pal.fps;
            let n = pal.frame_n as i32;
            let mut idx = (pal.time / dur) as i32;
            if n > 0 {
                idx %= n;
            }
            if idx < 0 {
                idx = 0;
            }
            let cycle = dur * n as f32;
            if cycle > 0.0 && pal.time >= cycle {
                pal.time %= cycle;
            }
            let i = idx as usize;
            pal.tex = pal.frame_tex[i];
            pal.u0 = pal.frame_u0[i];
            pal.v0 = pal.frame_v0[i];
            pal.u1 = pal.frame_u1[i];
            pal.v1 = pal.frame_v1[i];
        }
    }
}

fn tm_draw(e: &Entity) {
    let Some(tm) = e.tm.as_ref() else { return };
    if tm.cells.is_empty() || tm.w == 0 || tm.h == 0 {
        return;
    }
    let cell_w = tm.cell * e.sx;
    let cell_h = tm.cell * e.sy;
    if cell_w.abs() < 1e-6 || cell_h.abs() < 1e-6 {
        return;
    }
    let ox = e.x + tm.ox * e.sx;
    let oy = e.y + tm.oy * e.sy;
    let default_col = [48u8, 88, 176, 255];

    for y in 0..tm.h as i32 {
        for x in 0..tm.w as i32 {
            let id = tm_cell(tm, x, y);
            if id == 0 {
                continue;
            }
            let pal = tm_pal_for(tm, id);
            let col = pal.map(|p| p.color).unwrap_or(default_col);
            let mut tex = pal.map(|p| p.tex).unwrap_or(TILE_NO_TEX);
            let mut u0 = pal.map(|p| p.u0).unwrap_or(0.0);
            let mut v0 = pal.map(|p| p.v0).unwrap_or(0.0);
            let mut u1 = pal.map(|p| p.u1).unwrap_or(1.0);
            let mut v1 = pal.map(|p| p.v1).unwrap_or(1.0);
            if let Some(p) = pal {
                if p.auto_mode != TILE_AUTO_OFF {
                    let mask = tm_autotile_mask(tm, x, y, id, p.auto_mode) as usize;
                    if p.auto_tex[mask] != TILE_NO_TEX {
                        tex = p.auto_tex[mask];
                        u0 = p.auto_u0[mask];
                        v0 = p.auto_v0[mask];
                        u1 = p.auto_u1[mask];
                        v1 = p.auto_v1[mask];
                    }
                }
            }
            let dx = ox + x as f32 * cell_w;
            let dy = oy + y as f32 * cell_h;
            let rgba = rgba_pack(col);
            if tex != TILE_NO_TEX && (tex as u32) < ffi::tex_count() {
                ffi::gx_draw_sprite(tex as u32, dx, dy, cell_w, cell_h, u0, v0, u1, v1, rgba);
            } else {
                ffi::gx_draw_quad(dx, dy, cell_w, cell_h, rgba);
            }
        }
    }
}

/// Sort by `z` and issue GX draws for every drawable entity.
pub fn flush_entities(entities: &[Entity], player_rgba: Option<(usize, u32)>) {
    let mut order: Vec<usize> = (0..entities.len()).collect();
    order.sort_by(|&a, &b| {
        entities[a]
            .z
            .partial_cmp(&entities[b].z)
            .unwrap_or(core::cmp::Ordering::Equal)
    });

    for &i in &order {
        let e = &entities[i];
        match e.kind {
            KIND_SPRITE => {
                let dw = e.size_w * e.sx;
                let dh = e.size_h * e.sy;
                let x = e.x - dw * e.pivot_x;
                let y = e.y - dh * e.pivot_y;
                ffi::gx_draw_sprite(
                    e.tex as u32,
                    x,
                    y,
                    dw,
                    dh,
                    e.u0,
                    e.v0,
                    e.u1,
                    e.v1,
                    rgba_pack(e.color),
                );
            }
            KIND_DISC => {
                let mut col = rgba_pack(e.color);
                if let Some((pi, rgba)) = player_rgba {
                    if i == pi {
                        col = rgba;
                    }
                }
                let scale = if e.sx > e.sy { e.sx } else { e.sy };
                ffi::gx_draw_disc(e.x, e.y, e.radius * scale, col);
            }
            KIND_TILEMAP => tm_draw(e),
            KIND_TEXT => {
                let mut scale = if e.sx > e.sy { e.sx } else { e.sy };
                if scale < 0.0 {
                    scale = -scale;
                }
                ffi::gx_draw_text(
                    e.x,
                    e.y,
                    &e.text,
                    e.text_size * scale,
                    e.text_align,
                    rgba_pack(e.color),
                );
            }
            _ => {}
        }
    }
}
