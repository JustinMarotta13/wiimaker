//! Viewport hit-testing against scene sprites / discs.
//!
//! Geometry matches `render_world`: sprite AABB uses pivot (default center);
//! disc is a circle centered on transform XY with radius scaled by max(sx, sy).

use wiimaker_assets::SpriteCatalog;

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
pub fn pick_entity_at(scene: &Scene, sx: f32, sy: f32) -> Option<String> {
    pick_entity_at_with_catalog(scene, sx, sy, None)
}

pub fn pick_entity_at_with_catalog(
    scene: &Scene,
    sx: f32,
    sy: f32,
    catalog: Option<&SpriteCatalog>,
) -> Option<String> {
    let mut best: Option<(usize, f32, String)> = None;
    for (idx, ent) in scene.entities.iter().enumerate() {
        if let Some(z) = entity_hit_z(scene, ent, sx, sy, catalog) {
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

fn entity_hit_z(
    scene: &Scene,
    ent: &EntityData,
    sx: f32,
    sy: f32,
    catalog: Option<&SpriteCatalog>,
) -> Option<f32> {
    let world = scene
        .world_transform(&ent.name)
        .unwrap_or_else(|| ent.transform.clone());
    let mut hit_z: Option<f32> = None;
    if let Some(sp) = &ent.components.sprite {
        if sp.enabled {
            let pivot = catalog
                .and_then(|c| c.lookup(&sp.texture))
                .map(|r| r.pivot)
                .unwrap_or([0.5, 0.5]);
            if point_in_sprite(&world, sp.size, pivot, sx, sy) {
                hit_z = Some(sp.z);
            }
        }
    }
    if let Some(d) = &ent.components.disc {
        if d.enabled && point_in_disc(&world, d.radius, sx, sy) {
            hit_z = Some(match hit_z {
                Some(z) => z.max(d.z),
                None => d.z,
            });
        }
    }
    if let Some(tm) = &ent.components.tilemap {
        if tm.enabled && point_in_tilemap(&world, tm, sx, sy) {
            hit_z = Some(match hit_z {
                Some(z) => z.max(tm.z),
                None => tm.z,
            });
        }
    }
    hit_z
}

fn point_in_tilemap(
    world: &crate::scene::SceneTransform,
    tm: &crate::scene::SceneTilemap,
    sx: f32,
    sy: f32,
) -> bool {
    let cell_w = tm.cell * world.scale[0];
    let cell_h = tm.cell * world.scale[1];
    let left = world.translation[0] + tm.origin[0] * world.scale[0];
    let top = world.translation[1] + tm.origin[1] * world.scale[1];
    let w = tm.width as f32 * cell_w;
    let h = tm.height as f32 * cell_h;
    sx >= left && sx <= left + w && sy >= top && sy <= top + h
}

fn point_in_sprite(
    world: &crate::scene::SceneTransform,
    size: [f32; 2],
    pivot: [f32; 2],
    sx: f32,
    sy: f32,
) -> bool {
    let w = size[0] * world.scale[0];
    let h = size[1] * world.scale[1];
    let left = world.translation[0] - w * pivot[0];
    let top = world.translation[1] - h * pivot[1];
    sx >= left && sx <= left + w && sy >= top && sy <= top + h
}

fn point_in_disc(world: &crate::scene::SceneTransform, radius: f32, sx: f32, sy: f32) -> bool {
    let cx = world.translation[0];
    let cy = world.translation[1];
    let r = radius * world.scale[0].max(world.scale[1]);
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
            parent: None,
            transform: SceneTransform::from_xy(x, y),
            components: SceneComponents {
                sprite: Some(SceneSprite {
                    texture: "t".into(),
                    size: [w, h],
                    color: [255, 255, 255, 255],
                    z,
                    enabled: true,
                }),
                ..Default::default()
            },
            tag: 0,
        }
    }

    fn disc_ent(name: &str, x: f32, y: f32, radius: f32, z: f32) -> EntityData {
        EntityData {
            name: name.into(),
            parent: None,
            transform: SceneTransform::from_xy(x, y),
            components: SceneComponents {
                disc: Some(SceneDisc {
                    radius,
                    color: [72, 210, 160, 255],
                    z,
                    enabled: true,
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
        scene
            .entities
            .push(sprite_ent("A", 100.0, 100.0, 40.0, 20.0, 0.0));
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
        assert_eq!(pick_entity_at(&scene, 10.0, 15.0).as_deref(), Some("A"));
        assert_eq!(pick_entity_at(&scene, 10.1, 0.0), None);
    }

    #[test]
    fn disc_hit_uses_scaled_radius() {
        let mut scene = Scene::new("t");
        let mut e = disc_ent("D", 50.0, 50.0, 10.0, 0.0);
        e.transform.scale = [2.0, 1.0, 1.0];
        scene.entities.push(e);
        assert_eq!(pick_entity_at(&scene, 50.0, 50.0).as_deref(), Some("D"));
        assert_eq!(pick_entity_at(&scene, 70.0, 50.0).as_deref(), Some("D"));
        assert_eq!(pick_entity_at(&scene, 71.0, 50.0), None);
    }

    #[test]
    fn prefers_higher_z_then_later_entity() {
        let mut scene = Scene::new("t");
        scene
            .entities
            .push(sprite_ent("low", 100.0, 100.0, 40.0, 40.0, 0.0));
        scene
            .entities
            .push(sprite_ent("high", 100.0, 100.0, 40.0, 40.0, 2.0));
        assert_eq!(
            pick_entity_at(&scene, 100.0, 100.0).as_deref(),
            Some("high")
        );

        let mut scene2 = Scene::new("t");
        scene2
            .entities
            .push(sprite_ent("first", 100.0, 100.0, 40.0, 40.0, 1.0));
        scene2
            .entities
            .push(sprite_ent("second", 100.0, 100.0, 40.0, 40.0, 1.0));
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

    #[test]
    fn tilemap_hit_uses_grid_aabb() {
        use crate::scene::SceneTilemap;
        let mut scene = Scene::new("t");
        let mut tm = SceneTilemap::new(4, 3, 10.0);
        tm.z = 0.0;
        scene.entities.push(EntityData {
            name: "Maze".into(),
            parent: None,
            transform: SceneTransform::from_xy(0.0, 0.0),
            components: SceneComponents {
                tilemap: Some(tm),
                ..Default::default()
            },
            tag: 0,
        });
        assert_eq!(pick_entity_at(&scene, 5.0, 5.0).as_deref(), Some("Maze"));
        assert_eq!(pick_entity_at(&scene, 39.0, 29.0).as_deref(), Some("Maze"));
        assert_eq!(pick_entity_at(&scene, 41.0, 5.0), None);
    }
}
