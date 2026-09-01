//! Advance sprite Animation clips and push UV/texture onto Sprite.

use wiimaker_assets::SpriteCatalog;
use wiimaker_core::draw::Rect;
use wiimaker_core::math::Vec2;
use wiimaker_core::world::World;

use crate::hydrate::TextureMap;

/// Tick every entity with [`Animation`]: advance frame time and apply the
/// current cell onto the sibling [`wiimaker_core::world::Sprite`] (UV / texture /
/// pivot / size). Color and z are preserved.
///
/// Call from host `App::update` and editor Play (or every editor frame).
pub fn animate_world(
    world: &mut World,
    catalog: &SpriteCatalog,
    textures: &TextureMap,
    dt: f32,
) {
    let ids: Vec<_> = world.iter_entities().collect();
    for id in ids {
        // Split borrow: pull animation fields, then mutate sprite.
        let Some(anim) = world.animation_mut(id) else {
            continue;
        };
        if anim.cells.is_empty() || anim.fps <= 0.0 {
            continue;
        }
        let n = anim.cells.len();
        anim.time += dt;
        let frame_dur = 1.0 / anim.fps;
        let mut idx = (anim.time / frame_dur) as usize;
        if anim.loop_ {
            idx %= n;
            // Keep time bounded so it does not grow forever.
            let cycle = frame_dur * n as f32;
            if cycle > 0.0 && anim.time >= cycle {
                anim.time = anim.time % cycle;
            }
        } else if idx >= n {
            idx = n - 1;
            anim.time = frame_dur * n as f32;
        }
        anim.frame = idx;
        let cell = anim.cells[idx].clone();

        let Some(resolved) = catalog.lookup(&cell) else {
            continue;
        };
        let sheet = resolved.sheet_texture.clone();
        let uv = resolved.uv;
        let pivot = resolved.pivot;
        let pixel_size = resolved.pixel_size;
        let Some(tex_id) = textures.get(&sheet) else {
            continue;
        };
        if let Some(sp) = world.sprite_mut(id) {
            sp.texture = tex_id;
            sp.uv = Rect::new(uv[0], uv[1], uv[2], uv[3]);
            sp.pivot = Vec2::new(pivot[0], pivot[1]);
            sp.size = Vec2::new(pixel_size[0], pixel_size[1]);
        }
    }
}

/// Apply the current animation frame once without advancing time (hydrate / scrub).
pub fn apply_animation_frame(
    world: &mut World,
    catalog: &SpriteCatalog,
    textures: &TextureMap,
    id: wiimaker_core::world::EntityId,
) {
    let Some(cell) = world
        .animation(id)
        .and_then(|a| a.cell_name().map(|s| s.to_string()))
    else {
        return;
    };
    let Some(resolved) = catalog.lookup(&cell) else {
        return;
    };
    let sheet = resolved.sheet_texture.clone();
    let uv = resolved.uv;
    let pivot = resolved.pivot;
    let pixel_size = resolved.pixel_size;
    let Some(tex_id) = textures.get(&sheet) else {
        return;
    };
    if let Some(sp) = world.sprite_mut(id) {
        sp.texture = tex_id;
        sp.uv = Rect::new(uv[0], uv[1], uv[2], uv[3]);
        sp.pivot = Vec2::new(pivot[0], pivot[1]);
        sp.size = Vec2::new(pixel_size[0], pixel_size[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_assets::{AnimClipMeta, ResolvedSprite, SpriteCatalog};
    use wiimaker_core::draw::TextureId;
    use wiimaker_core::math::Vec2;
    use wiimaker_core::world::{Animation, Sprite, Transform};

    fn catalog_two_cells() -> SpriteCatalog {
        // SpriteCatalog has no public insert — build via load_dir is heavy.
        // Use a tiny helper: we only need lookup. Re-implement through write_anim path is overkill.
        // Instead test Animation timing math via a minimal world + manual ResolvedSprite isn't
        // publicly constructible into catalog... SpriteCatalog::empty() and no lookup → skip sprite.
        // So unit-test frame advance without catalog hits.
        SpriteCatalog::empty()
    }

    #[test]
    fn advances_frame_with_loop() {
        let mut world = World::new();
        let id = world.spawn_named("p", Transform::from_xy(0.0, 0.0));
        world.set_sprite(id, Some(Sprite::new(TextureId(0), Vec2::new(16.0, 16.0))));
        world.set_animation(
            id,
            Some(Animation::new(
                "chomp",
                vec!["a".into(), "b".into()],
                10.0,
                true,
            )),
        );
        let cat = catalog_two_cells();
        let tex = TextureMap::new();
        animate_world(&mut world, &cat, &tex, 0.0);
        assert_eq!(world.animation(id).unwrap().frame, 0);
        animate_world(&mut world, &cat, &tex, 0.11);
        assert_eq!(world.animation(id).unwrap().frame, 1);
        animate_world(&mut world, &cat, &tex, 0.11);
        assert_eq!(world.animation(id).unwrap().frame, 0);
    }

    #[test]
    fn clamps_when_not_looping() {
        let mut world = World::new();
        let id = world.spawn(Transform::default());
        world.set_animation(
            id,
            Some(Animation::new("once", vec!["a".into(), "b".into()], 10.0, false)),
        );
        let cat = SpriteCatalog::empty();
        let tex = TextureMap::new();
        animate_world(&mut world, &cat, &tex, 1.0);
        assert_eq!(world.animation(id).unwrap().frame, 1);
    }

    #[test]
    fn _anim_meta_smoke() {
        let m = AnimClipMeta {
            fps: 10.0,
            loop_: true,
            cells: vec!["x".into()],
        };
        assert_eq!(m.cells.len(), 1);
        let _ = ResolvedSprite {
            sheet_texture: "s".into(),
            uv: [0.0, 0.0, 1.0, 1.0],
            pivot: [0.5, 0.5],
            pixel_size: [8.0, 8.0],
            is_cell: true,
        };
    }
}
