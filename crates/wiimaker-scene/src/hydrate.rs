//! Scene → World hydration with texture name resolution.

use std::collections::HashMap;

use anyhow::Result;
use wiimaker_core::draw::TextureId;
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
    let mut world = World::new();
    hydrate_into(&mut world, scene, textures)?;
    Ok(world)
}

pub fn hydrate_into(world: &mut World, scene: &Scene, textures: &TextureMap) -> Result<()> {
    world.clear();
    for ent in &scene.entities {
        spawn_entity(world, ent, textures)?;
    }
    Ok(())
}

fn spawn_entity(world: &mut World, ent: &EntityData, textures: &TextureMap) -> Result<()> {
    let id = world.spawn_named(ent.name.clone(), ent.transform.to_runtime());
    world.set_tag(id, ent.tag);

    if let Some(sp) = &ent.components.sprite {
        let tex = textures.get(&sp.texture).ok_or_else(|| {
            anyhow::anyhow!(
                "entity '{}': texture '{}' not found in wpack",
                ent.name,
                sp.texture
            )
        })?;
        let mut sprite = Sprite::new(tex, sp.size_vec());
        sprite.color = sp.color_rgba();
        sprite.z = sp.z;
        world.set_sprite(id, Some(sprite));
    }

    if let Some(d) = &ent.components.disc {
        let mut disc = Disc::new(d.radius, d.color_rgba());
        disc.z = d.z;
        world.set_disc(id, Some(disc));
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

/// Soft hydrate: missing textures skip the sprite instead of failing (editor preview).
pub fn hydrate_lenient(scene: &Scene, textures: &TextureMap) -> World {
    let mut world = World::new();
    for ent in &scene.entities {
        let id = world.spawn_named(ent.name.clone(), ent.transform.to_runtime());
        world.set_tag(id, ent.tag);
        if let Some(sp) = &ent.components.sprite {
            if let Some(tex) = textures.get(&sp.texture) {
                let mut sprite = Sprite::new(tex, sp.size_vec());
                sprite.color = sp.color_rgba();
                sprite.z = sp.z;
                world.set_sprite(id, Some(sprite));
            }
        }
        if let Some(d) = &ent.components.disc {
            let mut disc = Disc::new(d.radius, d.color_rgba());
            disc.z = d.z;
            world.set_disc(id, Some(disc));
        }
        if let Some(cam) = &ent.components.camera {
            world.set_camera(id, Some(Camera { active: cam.active }));
        }
    }
    world
}
