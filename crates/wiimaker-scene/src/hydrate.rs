//! Scene → World hydration with texture / sprite-cell name resolution.

use std::collections::HashMap;

use anyhow::Result;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::collider::{Collider, ColliderKind};
use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::{Rect, TextureId};
use wiimaker_core::math::Vec2;
use wiimaker_core::tilemap::{TileVisual, Tilemap};
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
        world.set_camera(id, Some(Camera { active: cam.active }));
    }

    if let Some(tm) = &ent.components.tilemap {
        if tm.enabled {
            world.set_tilemap(id, Some(scene_tilemap_to_runtime(tm, textures, catalog)));
        }
    }

    if let Some(c) = &ent.components.collider {
        if c.enabled {
            world.set_collider(id, Some(scene_collider_to_runtime(c)));
        }
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
        if let Some(tm) = &ent.components.tilemap {
            if tm.enabled {
                world.set_tilemap(id, Some(scene_tilemap_to_runtime(tm, textures, catalog)));
            }
        }
        if let Some(c) = &ent.components.collider {
            if c.enabled {
                world.set_collider(id, Some(scene_collider_to_runtime(c)));
            }
        }
    }
    world
}

fn scene_tilemap_to_runtime(
    tm: &crate::scene::SceneTilemap,
    textures: &TextureMap,
    catalog: Option<&SpriteCatalog>,
) -> Tilemap {
    let mut out = Tilemap::new(tm.width.max(1), tm.height.max(1), tm.cell);
    out.origin = Vec2::new(tm.origin[0], tm.origin[1]);
    out.z = tm.z;
    let n = out.len();
    out.cells = tm.cells.clone();
    if out.cells.len() < n {
        out.cells.resize(n, 0);
    } else if out.cells.len() > n {
        out.cells.truncate(n);
    }
    out.solid = vec![0; (n + 7) / 8];
    for (i, flag) in tm.solid.iter().take(n).enumerate() {
        if *flag != 0 {
            let byte = i / 8;
            let bit = i % 8;
            out.solid[byte] |= 1 << bit;
        }
    }
    out.palette = tm
        .palette
        .iter()
        .map(|p| {
            let texture = p.sprite.as_ref().and_then(|name| {
                let (tex_name, uv) = resolve_palette_sprite(name, catalog);
                textures.get(&tex_name).map(|tex| (tex, uv))
            });
            TileVisual {
                id: p.id,
                texture,
                color: p.color_rgba(),
            }
        })
        .collect();
    if out.palette.is_empty() {
        out.palette.push(TileVisual {
            id: 1,
            texture: None,
            color: Rgba8::rgb(48, 88, 176),
        });
    }
    out
}

fn resolve_palette_sprite(name: &str, catalog: Option<&SpriteCatalog>) -> (String, Rect) {
    if let Some(cat) = catalog {
        if let Some(r) = cat.lookup(name) {
            return (
                r.sheet_texture.clone(),
                Rect::new(r.uv[0], r.uv[1], r.uv[2], r.uv[3]),
            );
        }
    }
    (name.to_string(), Rect::unit())
}

fn scene_collider_to_runtime(c: &crate::scene::SceneCollider) -> Collider {
    Collider {
        kind: match c.kind {
            crate::scene::SceneColliderKind::Aabb => ColliderKind::Aabb {
                size: Vec2::new(c.size[0].max(0.0), c.size[1].max(0.0)),
            },
            crate::scene::SceneColliderKind::Circle => ColliderKind::Circle {
                radius: c.radius.max(0.0),
            },
        },
        offset: Vec2::new(c.offset[0], c.offset[1]),
        solid: c.solid,
        trigger: c.trigger,
        filter_tag: c.filter_tag,
    }
}
