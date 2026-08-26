//! Default World → DrawList renderer (scene player).

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{DrawList, Rect};
use wiimaker_core::math::Vec2;
use wiimaker_core::world::World;

/// Emit clear + all Sprite/Disc components.
///
/// Dest origin = `translation - pivot * size * scale` (default pivot is center).
pub fn render_world(world: &World, draw: &mut DrawList, clear: Rgba8) {
    draw.clear(clear);

    // Collect and sort by z so draw order is stable.
    let mut sprites: Vec<_> = world.iter_sprites().collect();
    sprites.sort_by(|a, b| a.2.z.partial_cmp(&b.2.z).unwrap_or(std::cmp::Ordering::Equal));
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
    discs.sort_by(|a, b| a.2.z.partial_cmp(&b.2.z).unwrap_or(std::cmp::Ordering::Equal));
    for (_id, xf, d) in discs {
        draw.disc(
            Vec2::new(xf.translation.x, xf.translation.y),
            d.radius * xf.scale.x.max(xf.scale.y),
            d.color,
            d.z,
        );
    }
}
