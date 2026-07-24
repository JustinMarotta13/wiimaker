//! Viewport hit-testing against scene sprites / discs.
//!
//! Geometry matches `render_world`: sprite AABB is centered on transform XY;
//! disc is a circle centered on transform XY with radius scaled by max(sx, sy).

use crate::scene::{EntityData, Scene};

/// Map a pointer position from an on-screen image rect into framebuffer / scene
/// coordinates of size `view_w` × `view_h`.
///
/// Returns `None` if the pointer is outside `image_rect`.
pub fn pointer_to_scene(
    pointer: [f32; 2],
    image_rect_min: [f32; 2],
    image_rect_size: [f32; 2],
    view_w: f32,
    view_h: f32,
) -> Option<[f32; 2]> {
    if image_rect_size[0] <= 0.0 || image_rect_size[1] <= 0.0 {
        return None;
    }
    let lx = pointer[0] - image_rect_min[0];
    let ly = pointer[1] - image_rect_min[1];
    if lx < 0.0 || ly < 0.0 || lx > image_rect_size[0] || ly > image_rect_size[1] {
        return None;
    }
    Some([
        lx / image_rect_size[0] * view_w,
        ly / image_rect_size[1] * view_h,
    ])
}

/// Topmost entity under scene-space point `(sx, sy)`, or `None` if empty space.
///
/// Prefers highest sprite/disc `z`; on ties, later entities in the scene list win
/// (matches later draw overwriting earlier at equal z).
pub fn pick_entity_at(scene: &Scene, sx: f32, sy: f32) -> Option<String> {
    let mut best: Option<(usize, f32, String)> = None;
    for (idx, ent) in scene.entities.iter().enumerate() {
        if let Some(z) = entity_hit_z(ent, sx, sy) {
            let better = match &best {
                None => true,
                Some((bi, bz, _)) => z > *bz || (z == *bz && idx > *bi),
            };
            if better {
                best = Some((idx, z, ent.name.clone()));
            }
        }
    }
    best.map(|(_, _, name)| name)
}

fn entity_hit_z(ent: &EntityData, sx: f32, sy: f32) -> Option<f32> {
    let mut hit_z: Option<f32> = None;
    if let Some(sp) = &ent.components.sprite {
        if point_in_sprite(ent, sp.size, sx, sy) {
            hit_z = Some(sp.z);
        }
    }
    if let Some(d) = &ent.components.disc {
        if point_in_disc(ent, d.radius, sx, sy) {
            hit_z = Some(match hit_z {
                Some(z) => z.max(d.z),
                None => d.z,
            });
        }
    }
    hit_z
}

fn point_in_sprite(ent: &EntityData, size: [f32; 2], sx: f32, sy: f32) -> bool {
    let cx = ent.transform.translation[0];
    let cy = ent.transform.translation[1];
    let hw = size[0] * ent.transform.scale[0] * 0.5;
    let hh = size[1] * ent.transform.scale[1] * 0.5;
    (sx - cx).abs() <= hw && (sy - cy).abs() <= hh
}

fn point_in_disc(ent: &EntityData, radius: f32, sx: f32, sy: f32) -> bool {
    let cx = ent.transform.translation[0];
    let cy = ent.transform.translation[1];
    let r = radius * ent.transform.scale[0].max(ent.transform.scale[1]);
    let dx = sx - cx;
    let dy = sy - cy;
    dx * dx + dy * dy <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{
        EntityData, Scene, SceneComponents, SceneDisc, SceneSprite, SceneTransform,
    };

    fn sprite_ent(name: &str, x: f32, y: f32, w: f32, h: f32, z: f32) -> EntityData {
        EntityData {
            name: name.into(),
            transform: SceneTransform::from_xy(x, y),
            components: SceneComponents {
                sprite: Some(SceneSprite {
                    texture: "t".into(),
                    size: [w, h],
                    color: [255, 255, 255, 255],
                    z,
                }),
                ..Default::default()
            },
            tag: 0,
        }
    }

    fn disc_ent(name: &str, x: f32, y: f32, radius: f32, z: f32) -> EntityData {
        EntityData {
            name: name.into(),
            transform: SceneTransform::from_xy(x, y),
            components: SceneComponents {
                disc: Some(SceneDisc {
                    radius,
                    color: [72, 210, 160, 255],
                    z,
                }),
                ..Default::default()
            },
            tag: 0,
        }
    }

    #[test]
    fn pointer_maps_into_scene_and_rejects_outside() {
        assert_eq!(
            pointer_to_scene([10.0, 20.0], [10.0, 20.0], [320.0, 240.0], 640.0, 480.0),
            Some([0.0, 0.0])
        );
        assert_eq!(
            pointer_to_scene([330.0, 260.0], [10.0, 20.0], [320.0, 240.0], 640.0, 480.0),
            Some([640.0, 480.0])
        );
        // Midpoint of half-size blit → scene center.
        assert_eq!(
            pointer_to_scene([170.0, 140.0], [10.0, 20.0], [320.0, 240.0], 640.0, 480.0),
            Some([320.0, 240.0])
        );
        assert_eq!(
            pointer_to_scene([5.0, 20.0], [10.0, 20.0], [320.0, 240.0], 640.0, 480.0),
            None
        );
    }

    #[test]
    fn sprite_hit_uses_centered_aabb() {
        let mut scene = Scene::new("t");
        scene.entities.push(sprite_ent("A", 100.0, 100.0, 40.0, 20.0, 0.0));
        assert_eq!(pick_entity_at(&scene, 100.0, 100.0).as_deref(), Some("A"));
        assert_eq!(pick_entity_at(&scene, 120.0, 110.0).as_deref(), Some("A"));
        assert_eq!(pick_entity_at(&scene, 121.0, 100.0), None);
        assert_eq!(pick_entity_at(&scene, 100.0, 111.0), None);
    }

    #[test]
    fn sprite_hit_respects_scale() {
        let mut scene = Scene::new("t");
        let mut e = sprite_ent("A", 0.0, 0.0, 10.0, 10.0, 0.0);
        e.transform.scale = [2.0, 3.0, 1.0];
        scene.entities.push(e);
        // half extents = 10, 15
        assert_eq!(pick_entity_at(&scene, 10.0, 15.0).as_deref(), Some("A"));
        assert_eq!(pick_entity_at(&scene, 10.1, 0.0), None);
    }

    #[test]
    fn disc_hit_uses_scaled_radius() {
        let mut scene = Scene::new("t");
        let mut e = disc_ent("D", 50.0, 50.0, 10.0, 0.0);
        e.transform.scale = [2.0, 1.0, 1.0]; // r = 20
        scene.entities.push(e);
        assert_eq!(pick_entity_at(&scene, 50.0, 50.0).as_deref(), Some("D"));
        assert_eq!(pick_entity_at(&scene, 70.0, 50.0).as_deref(), Some("D"));
        assert_eq!(pick_entity_at(&scene, 71.0, 50.0), None);
    }

    #[test]
    fn prefers_higher_z_then_later_entity() {
        let mut scene = Scene::new("t");
        scene.entities.push(sprite_ent("low", 100.0, 100.0, 40.0, 40.0, 0.0));
        scene.entities.push(sprite_ent("high", 100.0, 100.0, 40.0, 40.0, 2.0));
        assert_eq!(pick_entity_at(&scene, 100.0, 100.0).as_deref(), Some("high"));

        let mut scene2 = Scene::new("t");
        scene2.entities.push(sprite_ent("first", 100.0, 100.0, 40.0, 40.0, 1.0));
        scene2.entities.push(sprite_ent("second", 100.0, 100.0, 40.0, 40.0, 1.0));
        assert_eq!(
            pick_entity_at(&scene2, 100.0, 100.0).as_deref(),
            Some("second")
        );
    }

    #[test]
    fn empty_space_returns_none() {
        let mut scene = Scene::new("t");
        scene.entities.push(disc_ent("D", 10.0, 10.0, 5.0, 0.0));
        assert_eq!(pick_entity_at(&scene, 200.0, 200.0), None);
    }
}
