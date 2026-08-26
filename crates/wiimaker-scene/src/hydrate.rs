//! Scene → World hydration with texture / sprite-cell name resolution.

use std::collections::HashMap;

use anyhow::Result;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::draw::{Rect, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::world::{Camera, Disc, Sprite, World};

use crate::scene::{EntityData, Scene};

/// Maps texture asset names → [`TextureId`] (from a loaded `.wpack`).
#[derive(Clone, Debug, Default)]
pub struct TextureMap {
    by_name: HashMap<String, TextureId>,
}

impl TextureMap {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, id: TextureId) {
        self.by_name.insert(name.into(), id);
    }

    pub fn get(&self, name: &str) -> Option<TextureId> {
        self.by_name.get(name).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(|s| s.as_str())
    }

    pub fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut map = Self::new();
        for (i, name) in names.into_iter().enumerate() {
            map.insert(name.as_ref().to_string(), TextureId(i as u32));
        }
        map
    }
}

pub fn hydrate(scene: &Scene, textures: &TextureMap) -> Result<World> {
    hydrate_with_catalog(scene, textures, None)
}

pub fn hydrate_with_catalog(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Result<World> {
    let mut world = World::new();
    hydrate_into_with_catalog(&mut world, scene, textures, catalog)?;
    Ok(world)
}

pub fn hydrate_into(world: &mut World, scene: &Scene, textures: &TextureMap) -> Result<()> {
    hydrate_into_with_catalog(world, scene, textures, None)
}

pub fn hydrate_into_with_catalog(
    world: &mut World,
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Result<()> {
    world.clear();
    for ent in &scene.entities {
        spawn_entity(world, scene, ent, textures, catalog)?;
    }
    Ok(())
}

fn spawn_entity(
    world: &mut World,
    scene: &Scene,
    ent: &EntityData,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Result<()> {
    let xf = scene
        .world_transform(&ent.name)
        .unwrap_or_else(|| ent.transform.clone());
    let id = world.spawn_named(ent.name.clone(), xf.to_runtime());
    world.set_tag(id, ent.tag);

    if let Some(sp) = &ent.components.sprite {
        if sp.enabled {
            let (tex_name, uv, pivot, size) = resolve_sprite(sp, catalog);
            let tex = textures.get(&tex_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "entity '{}': texture '{}' not found in wpack",
                    ent.name,
                    tex_name
                )
            })?;
            let mut sprite = Sprite::new(tex, size);
            sprite.uv = uv;
            sprite.pivot = pivot;
            sprite.color = sp.color_rgba();
            sprite.z = sp.z;
            world.set_sprite(id, Some(sprite));
        }
    }

    if let Some(d) = &ent.components.disc {
        if d.enabled {
            let mut disc = Disc::new(d.radius, d.color_rgba());
            disc.z = d.z;
            world.set_disc(id, Some(disc));
        }
    }

    if let Some(cam) = &ent.components.camera {
        world.set_camera(
            id,
            Some(Camera {
                active: cam.active,
            }),
        );
    }

    Ok(())
}

fn resolve_sprite(
    sp: &crate::scene::SceneSprite,
    catalog: Option<&SpriteCatalog>,
) -> (String, Rect, Vec2, Vec2) {
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(&sp.texture) {
            let uv = Rect::new(r.uv[0], r.uv[1], r.uv[2], r.uv[3]);
            let pivot = Vec2::new(r.pivot[0], r.pivot[1]);
            // Scene size wins when author set it; cells often keep authored size.
            let size = sp.size_vec();
            return (r.sheet_texture.clone(), uv, pivot, size);
        }
    }
    (
        sp.texture.clone(),
        Rect::unit(),
        Vec2::new(0.5, 0.5),
        sp.size_vec(),
    )
}

/// Soft hydrate: missing textures skip the sprite instead of failing (editor preview).
pub fn hydrate_lenient(scene: &Scene, textures: &TextureMap) -> World {
    hydrate_lenient_with_catalog(scene, textures, None)
}

pub fn hydrate_lenient_with_catalog(
    scene: &Scene,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> World {
    let mut world = World::new();
    for ent in &scene.entities {
        let xf = scene
            .world_transform(&ent.name)
            .unwrap_or_else(|| ent.transform.clone());
        let id = world.spawn_named(ent.name.clone(), xf.to_runtime());
        world.set_tag(id, ent.tag);
        if let Some(sp) = &ent.components.sprite {
            if sp.enabled {
                let (tex_name, uv, pivot, size) = resolve_sprite(sp, catalog);
                if let Some(tex) = textures.get(&tex_name) {
                    let mut sprite = Sprite::new(tex, size);
                    sprite.uv = uv;
                    sprite.pivot = pivot;
                    sprite.color = sp.color_rgba();
                    sprite.z = sp.z;
                    world.set_sprite(id, Some(sprite));
                }
            }
        }
        if let Some(d) = &ent.components.disc {
            if d.enabled {
                let mut disc = Disc::new(d.radius, d.color_rgba());
                disc.z = d.z;
                world.set_disc(id, Some(disc));
            }
        }
        if let Some(cam) = &ent.components.camera {
            world.set_camera(id, Some(Camera { active: cam.active }));
        }
    }
    world
}
