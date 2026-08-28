//! Default World → DrawList renderer (scene player).

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{DrawList, Rect, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::world::World;

/// Sentinel texture: host atlas samples white, then tint supplies the cell color.
const QUAD_TEX: TextureId = TextureId(u32::MAX);

/// Emit clear + tilemaps + Sprite/Disc components.
///
/// Dest origin = `translation - pivot * size * scale` (default pivot is center).
pub fn render_world(world: &World, draw: &mut DrawList, clear: Rgba8) {
    draw.clear(clear);

    let mut tiles: Vec<_> = world.iter_tilemaps().collect();
    tiles.sort_by(|a, b| {
        a.2.z
            .partial_cmp(&b.2.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (_id, xf, tm) in tiles {
        let cell_w = tm.cell * xf.scale.x;
        let cell_h = tm.cell * xf.scale.y;
        if cell_w.abs() < 1e-6 || cell_h.abs() < 1e-6 {
            continue;
        }
        let ox = xf.translation.x + tm.origin.x * xf.scale.x;
        let oy = xf.translation.y + tm.origin.y * xf.scale.y;
        for y in 0..tm.height as i32 {
            for x in 0..tm.width as i32 {
                let id = tm.get(x, y);
                if id == 0 {
                    continue;
                }
                let dest = Rect::new(
                    ox + x as f32 * cell_w,
                    oy + y as f32 * cell_h,
                    cell_w,
                    cell_h,
                );
                if let Some(vis) = tm.visual_for(id) {
                    match vis.texture {
                        Some((tex, uv)) => draw.sprite_ex(tex, dest, uv, vis.color, tm.z),
                        None => draw.sprite_ex(QUAD_TEX, dest, Rect::unit(), vis.color, tm.z),
                    }
                } else {
                    let color = Rgba8::rgb(48, 88, 176);
                    draw.sprite_ex(QUAD_TEX, dest, Rect::unit(), color, tm.z);
                }
            }
        }
    }

    // Collect and sort by z so draw order is stable.
    let mut sprites: Vec<_> = world.iter_sprites().collect();
    sprites.sort_by(|a, b| {
        a.2.z
            .partial_cmp(&b.2.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (_id, xf, sp) in sprites {
        let dest = Rect::new(
            xf.translation.x - sp.size.x * sp.pivot.x * xf.scale.x,
            xf.translation.y - sp.size.y * sp.pivot.y * xf.scale.y,
            sp.size.x * xf.scale.x,
            sp.size.y * xf.scale.y,
        );
        draw.sprite_ex(sp.texture, dest, sp.uv, sp.color, sp.z);
    }

    let mut discs: Vec<_> = world.iter_discs().collect();
    discs.sort_by(|a, b| {
        a.2.z
            .partial_cmp(&b.2.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (_id, xf, d) in discs {
        draw.disc(
            Vec2::new(xf.translation.x, xf.translation.y),
            d.radius * xf.scale.x.max(xf.scale.y),
            d.color,
            d.z,
        );
    }
}
