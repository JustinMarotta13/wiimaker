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
/// An **active** Camera offsets dests so its translation is the 640×480 viewport
/// center (camera at `320, 240` matches the no-camera identity). No active camera
/// → today's identity dests. Camera transforms are intentionally not emitted as
/// [`DrawCmd::SetCamera`] because host backends apply the offset in these dests.
pub fn render_world(world: &World, draw: &mut DrawList, clear: Rgba8) {
    render_world_ex(world, draw, clear, true);
}

/// Like [`render_world`], but Scene view can pass `apply_camera = false` so the
/// editor keeps world-space dests and overlays a camera rect gizmo instead.
pub fn render_world_ex(world: &World, draw: &mut DrawList, clear: Rgba8, apply_camera: bool) {
    draw.clear(clear);

    let offset = if apply_camera {
        world.camera_view_offset()
    } else {
        Vec2::ZERO
    };

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
        let ox = xf.translation.x + tm.origin.x * xf.scale.x - offset.x;
        let oy = xf.translation.y + tm.origin.y * xf.scale.y - offset.y;
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
            xf.translation.x - sp.size.x * sp.pivot.x * xf.scale.x - offset.x,
            xf.translation.y - sp.size.y * sp.pivot.y * xf.scale.y - offset.y,
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
            Vec2::new(xf.translation.x - offset.x, xf.translation.y - offset.y),
            d.radius * xf.scale.x.max(xf.scale.y),
            d.color,
            d.z,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::draw::DrawCmd;
    use wiimaker_core::world::{Camera, Disc, Sprite, Transform};

    fn sprite_dests(cmds: &[DrawCmd]) -> Vec<(f32, f32)> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::DrawSprite { dest, .. } => Some((dest.x, dest.y)),
                _ => None,
            })
            .collect()
    }

    fn disc_centers(cmds: &[DrawCmd]) -> Vec<(f32, f32)> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::DrawDisc { center, .. } => Some((center.x, center.y)),
                _ => None,
            })
            .collect()
    }

    fn has_set_camera(cmds: &[DrawCmd]) -> bool {
        cmds.iter().any(|c| matches!(c, DrawCmd::SetCamera { .. }))
    }

    #[test]
    fn no_camera_matches_identity_dests() {
        let mut world = World::new();
        let id = world.spawn_named("orb", Transform::from_xy(100.0, 50.0));
        world.set_sprite(id, Some(Sprite::new(TextureId(0), Vec2::new(32.0, 32.0))));
        world.set_disc(id, Some(Disc::new(8.0, Rgba8::WHITE)));
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        assert!(!has_set_camera(draw.cmds()));
        // pivot 0.5 → dest origin 84, 34
        assert_eq!(sprite_dests(draw.cmds()), vec![(84.0, 34.0)]);
        assert_eq!(disc_centers(draw.cmds()), vec![(100.0, 50.0)]);
    }

    #[test]
    fn camera_at_default_center_matches_no_camera() {
        let mut world = World::new();
        let orb = world.spawn_named("orb", Transform::from_xy(100.0, 50.0));
        world.set_sprite(orb, Some(Sprite::new(TextureId(0), Vec2::new(32.0, 32.0))));
        let cam = world.spawn_named("Cam", Transform::from_xy(320.0, 240.0));
        world.set_camera(cam, Some(Camera { active: true }));
        let mut with_cam = DrawList::new();
        render_world(&world, &mut with_cam, Rgba8::BLACK);
        world.set_camera(cam, None);
        let mut without = DrawList::new();
        render_world(&world, &mut without, Rgba8::BLACK);
        assert!(!has_set_camera(with_cam.cmds()));
        assert!(!has_set_camera(without.cmds()));
        assert_eq!(sprite_dests(with_cam.cmds()), sprite_dests(without.cmds()));
    }

    #[test]
    fn active_camera_offsets_sprite_and_disc() {
        let mut world = World::new();
        let orb = world.spawn_named("orb", Transform::from_xy(100.0, 50.0));
        world.set_sprite(orb, Some(Sprite::new(TextureId(0), Vec2::new(32.0, 32.0))));
        world.set_disc(orb, Some(Disc::new(8.0, Rgba8::WHITE)));
        let cam = world.spawn_named("Cam", Transform::from_xy(420.0, 240.0));
        world.set_camera(cam, Some(Camera { active: true }));
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        // offset x = 420 - 320 = 100
        assert_eq!(sprite_dests(draw.cmds()), vec![(84.0 - 100.0, 34.0)]);
        assert_eq!(disc_centers(draw.cmds()), vec![(0.0, 50.0)]);
        assert!(!has_set_camera(draw.cmds()));
    }

    #[test]
    fn scene_view_skips_camera_offset() {
        let mut world = World::new();
        let orb = world.spawn_named("orb", Transform::from_xy(100.0, 50.0));
        world.set_sprite(orb, Some(Sprite::new(TextureId(0), Vec2::new(32.0, 32.0))));
        let cam = world.spawn_named("Cam", Transform::from_xy(420.0, 240.0));
        world.set_camera(cam, Some(Camera { active: true }));
        let mut draw = DrawList::new();
        render_world_ex(&world, &mut draw, Rgba8::BLACK, false);
        assert!(!has_set_camera(draw.cmds()));
        assert_eq!(sprite_dests(draw.cmds()), vec![(84.0, 34.0)]);
    }
}
