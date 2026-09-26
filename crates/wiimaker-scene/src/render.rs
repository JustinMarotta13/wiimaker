//! Default World → DrawList renderer (scene player).

use wiimaker_core::cmp_sorting;
use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{DrawList, Rect, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::text::Text;
use wiimaker_core::tilemap::Tilemap;
use wiimaker_core::world::{Disc, Sprite, Transform, World};

/// Sentinel texture: host atlas samples white, then tint supplies the cell color.
const QUAD_TEX: TextureId = TextureId(u32::MAX);

enum DrawItem<'a> {
    Tilemap {
        layer: u16,
        z: f32,
        xf: &'a Transform,
        tm: &'a Tilemap,
    },
    Sprite {
        layer: u16,
        z: f32,
        xf: &'a Transform,
        sp: &'a Sprite,
    },
    Disc {
        layer: u16,
        z: f32,
        xf: &'a Transform,
        d: &'a Disc,
    },
    Text {
        layer: u16,
        z: f32,
        xf: &'a Transform,
        t: &'a Text,
    },
}

impl DrawItem<'_> {
    fn layer(&self) -> u16 {
        match self {
            DrawItem::Tilemap { layer, .. }
            | DrawItem::Sprite { layer, .. }
            | DrawItem::Disc { layer, .. }
            | DrawItem::Text { layer, .. } => *layer,
        }
    }

    fn z(&self) -> f32 {
        match self {
            DrawItem::Tilemap { z, .. }
            | DrawItem::Sprite { z, .. }
            | DrawItem::Disc { z, .. }
            | DrawItem::Text { z, .. } => *z,
        }
    }
}

/// Emit clear + tilemaps + Sprite/Disc components.
///
/// Dest origin = `translation - pivot * size * scale` (default pivot is center).
/// An **active** Camera offsets dests so its translation is the 640×480 viewport
/// center (camera at `320, 240` matches the no-camera identity). No active camera
/// → today's identity dests. Camera transforms are intentionally not emitted as
/// [`DrawCmd::SetCamera`] because host backends apply the offset in these dests.
///
/// Draw order is Unity Sorting Layer then Order in Layer (`z`) across Sprite,
/// Disc, Tilemap, and Text. Missing layers hydrate to Default.
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

    // Collect tilemaps, then sprites, then discs so equal (layer, z) stays stable
    // (old scenes without named layers still draw tiles → sprites → discs).
    let mut items: Vec<DrawItem<'_>> = Vec::new();
    for (_id, xf, tm) in world.iter_tilemaps() {
        items.push(DrawItem::Tilemap {
            layer: tm.sorting_layer,
            z: tm.z,
            xf,
            tm,
        });
    }
    for (_id, xf, sp) in world.iter_sprites() {
        items.push(DrawItem::Sprite {
            layer: sp.sorting_layer,
            z: sp.z,
            xf,
            sp,
        });
    }
    for (_id, xf, d) in world.iter_discs() {
        items.push(DrawItem::Disc {
            layer: d.sorting_layer,
            z: d.z,
            xf,
            d,
        });
    }
    for (_id, xf, t) in world.iter_texts() {
        items.push(DrawItem::Text {
            layer: t.sorting_layer,
            z: t.z,
            xf,
            t,
        });
    }
    items.sort_by(|a, b| cmp_sorting(a.layer(), a.z(), b.layer(), b.z()));

    for item in items {
        match item {
            DrawItem::Tilemap { xf, tm, z, .. } => {
                emit_tilemap(draw, xf, tm, z, offset);
            }
            DrawItem::Sprite { xf, sp, z, .. } => {
                let dest = Rect::new(
                    xf.translation.x - sp.size.x * sp.pivot.x * xf.scale.x - offset.x,
                    xf.translation.y - sp.size.y * sp.pivot.y * xf.scale.y - offset.y,
                    sp.size.x * xf.scale.x,
                    sp.size.y * xf.scale.y,
                );
                draw.sprite_ex(sp.texture, dest, sp.uv, sp.color, z);
            }
            DrawItem::Disc { xf, d, z, .. } => {
                draw.disc(
                    Vec2::new(xf.translation.x - offset.x, xf.translation.y - offset.y),
                    d.radius * xf.scale.x.max(xf.scale.y),
                    d.color,
                    z,
                );
            }
            DrawItem::Text { xf, t, z, .. } => {
                let size = t.size * xf.scale.x.abs().max(xf.scale.y.abs());
                draw.text_ex(
                    Vec2::new(xf.translation.x - offset.x, xf.translation.y - offset.y),
                    t.string.clone(),
                    t.color,
                    size,
                    t.align,
                    z,
                );
            }
        }
    }
}

fn emit_tilemap(draw: &mut DrawList, xf: &Transform, tm: &Tilemap, z: f32, offset: Vec2) {
    let cell_w = tm.cell * xf.scale.x;
    let cell_h = tm.cell * xf.scale.y;
    if cell_w.abs() < 1e-6 || cell_h.abs() < 1e-6 {
        return;
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
            let color = tm
                .cell_color(x, y)
                .unwrap_or_else(|| Rgba8::rgb(48, 88, 176));
            match tm.cell_texture(x, y) {
                Some((tex, uv)) => draw.sprite_ex(tex, dest, uv, color, z),
                None => draw.sprite_ex(QUAD_TEX, dest, Rect::unit(), color, z),
            }
        }
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

    fn draw_kinds(cmds: &[DrawCmd]) -> Vec<&'static str> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::DrawSprite { .. } => Some("sprite"),
                DrawCmd::DrawDisc { .. } => Some("disc"),
                DrawCmd::DrawText { .. } => Some("text"),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn layer_order_beats_z_across_kinds() {
        let mut world = World::new();
        // Background disc with huge z still draws behind Default sprite.
        let disc_id = world.spawn_named("back", Transform::from_xy(10.0, 10.0));
        let mut disc = Disc::new(4.0, Rgba8::WHITE);
        disc.z = 99.0;
        disc.sorting_layer = world.sorting_layer_index("Background");
        world.set_disc(disc_id, Some(disc));

        let spr_id = world.spawn_named("front", Transform::from_xy(20.0, 20.0));
        let mut spr = Sprite::new(TextureId(0), Vec2::new(8.0, 8.0));
        spr.z = 0.0;
        spr.sorting_layer = world.sorting_layer_index("Default");
        world.set_sprite(spr_id, Some(spr));

        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        assert_eq!(draw_kinds(draw.cmds()), vec!["disc", "sprite"]);
    }

    #[test]
    fn order_in_layer_within_same_layer() {
        let mut world = World::new();
        let a = world.spawn_named("a", Transform::from_xy(0.0, 0.0));
        let mut sa = Sprite::new(TextureId(1), Vec2::new(8.0, 8.0));
        sa.z = 2.0;
        sa.sorting_layer = world.sorting_layer_index("Foreground");
        world.set_sprite(a, Some(sa));
        let b = world.spawn_named("b", Transform::from_xy(40.0, 0.0));
        let mut sb = Sprite::new(TextureId(2), Vec2::new(8.0, 8.0));
        sb.z = -1.0;
        sb.sorting_layer = world.sorting_layer_index("Foreground");
        world.set_sprite(b, Some(sb));

        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        let dests = sprite_dests(draw.cmds());
        // b (z=-1) then a (z=2); pivot 0.5 → dest x = translation - 4
        assert_eq!(dests, vec![(36.0, -4.0), (-4.0, -4.0)]);
    }

    #[test]
    fn missing_layer_uses_default() {
        let mut world = World::new();
        let id = world.spawn_named("orb", Transform::from_xy(100.0, 50.0));
        let mut spr = Sprite::new(TextureId(0), Vec2::new(32.0, 32.0));
        spr.sorting_layer = world.sorting_layer_index("no-such-layer");
        world.set_sprite(id, Some(spr));
        assert_eq!(
            world.sprite(id).unwrap().sorting_layer,
            world.sorting_layer_index("Default")
        );
    }

    #[test]
    fn text_emits_draw_text_and_camera_offset() {
        use wiimaker_core::text::{Text, TextAlign};
        let mut world = World::new();
        let id = world.spawn_named("hud", Transform::from_xy(40.0, 12.0));
        let mut t = Text::new("Hi", 8.0, Rgba8::WHITE);
        t.align = TextAlign::Left;
        t.z = 2.0;
        t.sorting_layer = world.sorting_layer_index("Foreground");
        world.set_text(id, Some(t));
        let cam = world.spawn_named("Cam", Transform::from_xy(420.0, 240.0));
        world.set_camera(cam, Some(Camera { active: true }));
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        let texts: Vec<_> = draw
            .cmds()
            .iter()
            .filter_map(|c| match c {
                DrawCmd::DrawText {
                    pos, text, size, z, ..
                } => Some((pos.x, pos.y, text.clone(), *size, *z)),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 1);
        // offset x = 420 - 320 = 100
        assert!((texts[0].0 - (40.0 - 100.0)).abs() < 1e-4);
        assert!((texts[0].1 - 12.0).abs() < 1e-4);
        assert_eq!(texts[0].2, "Hi");
        assert!((texts[0].3 - 8.0).abs() < 1e-4);
        assert!((texts[0].4 - 2.0).abs() < 1e-4);
    }

    #[test]
    fn parented_disc_draws_at_composed_world_pose() {
        use crate::hydrate::{hydrate_lenient, TextureMap};
        use crate::mutate::{add_entity, set_entity_rotation_z, MutateOpts};
        use crate::scene::Scene;

        let mut scene = Scene::new("orbit");
        add_entity(
            &mut scene,
            "RotParent",
            &MutateOpts {
                x: Some(320.0),
                y: Some(240.0),
                radius: Some(28.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_entity_rotation_z(&mut scene, "RotParent", 45f32.to_radians()).unwrap();
        add_entity(
            &mut scene,
            "RotChild",
            &MutateOpts {
                x: Some(400.0),
                y: Some(240.0),
                radius: Some(16.0),
                ..Default::default()
            },
        )
        .unwrap();
        let child = scene
            .entities
            .iter_mut()
            .find(|e| e.name == "RotChild")
            .unwrap();
        child.transform.translation = [80.0, 0.0, 0.0];
        child.parent = Some("RotParent".into());

        let world = hydrate_lenient(&scene, &TextureMap::new());
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        let centers = disc_centers(draw.cmds());
        assert_eq!(centers.len(), 2);
        let s = 45f32.to_radians().sin();
        let c = 45f32.to_radians().cos();
        let want_child = (320.0 + 80.0 * c, 240.0 + 80.0 * s);
        let child_center = centers
            .iter()
            .find(|(x, y)| (*x - 320.0).abs() > 1.0 || (*y - 240.0).abs() > 1.0)
            .copied()
            .expect("child disc");
        assert!((child_center.0 - want_child.0).abs() < 1e-3);
        assert!((child_center.1 - want_child.1).abs() < 1e-3);
        assert!(centers
            .iter()
            .any(|(x, y)| (*x - 320.0).abs() < 1e-3 && (*y - 240.0).abs() < 1e-3));
    }

    #[test]
    fn text_sorts_with_sprites() {
        use wiimaker_core::text::Text;
        let mut world = World::new();
        let disc_id = world.spawn_named("back", Transform::from_xy(10.0, 10.0));
        let mut disc = Disc::new(4.0, Rgba8::WHITE);
        disc.sorting_layer = world.sorting_layer_index("Background");
        world.set_disc(disc_id, Some(disc));
        let hud = world.spawn_named("hud", Transform::from_xy(20.0, 20.0));
        let mut t = Text::new("GO", 8.0, Rgba8::WHITE);
        t.sorting_layer = world.sorting_layer_index("Foreground");
        world.set_text(hud, Some(t));
        let mut draw = DrawList::new();
        render_world(&world, &mut draw, Rgba8::BLACK);
        assert_eq!(draw_kinds(draw.cmds()), vec!["disc", "text"]);
    }
}
