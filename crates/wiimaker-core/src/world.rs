//! Entity store with Unity-shaped components (Transform + Sprite/Disc/Camera).

use crate::color::Rgba8;
use crate::draw::{Rect, TextureId};
use crate::math::{Quat, Vec2, Vec3};
use crate::tilemap::Tilemap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityId(pub u32);

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(t: Vec3) -> Self {
        Self {
            translation: t,
            ..Default::default()
        }
    }

    pub fn from_xy(x: f32, y: f32) -> Self {
        Self::from_translation(Vec3::new(x, y, 0.0))
    }

    pub fn matrix(&self) -> crate::math::Mat4 {
        crate::math::Mat4::from_scale_rotation_translation(
            self.scale,
            self.rotation,
            self.translation,
        )
    }
}

/// Screen-space sprite (like Unity SpriteRenderer for 2D).
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub texture: TextureId,
    pub size: Vec2,
    pub uv: Rect,
    /// Normalized pivot in sprite space (`0.5, 0.5` = center).
    pub pivot: Vec2,
    pub color: Rgba8,
    pub z: f32,
}

impl Sprite {
    pub fn new(texture: TextureId, size: Vec2) -> Self {
        Self {
            texture,
            size,
            uv: Rect::unit(),
            pivot: Vec2::new(0.5, 0.5),
            color: Rgba8::WHITE,
            z: 0.0,
        }
    }
}

/// Filled disc primitive (handy for prototypes / orbs).
#[derive(Clone, Copy, Debug)]
pub struct Disc {
    pub radius: f32,
    pub color: Rgba8,
    pub z: f32,
}

impl Disc {
    pub fn new(radius: f32, color: Rgba8) -> Self {
        Self {
            radius,
            color,
            z: 0.0,
        }
    }
}

/// Orthographic camera marker (v0: one active camera, screen space if none).
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    pub active: bool,
}

#[cfg(feature = "std")]
mod alloc_types {
    pub use std::string::String;
    pub use std::vec::Vec;
}

#[cfg(not(feature = "std"))]
mod alloc_types {
    extern crate alloc;
    pub use alloc::string::String;
    pub use alloc::vec::Vec;
}

use alloc_types::{String, Vec};

#[derive(Clone, Debug)]
struct Slot {
    live: bool,
    name: String,
    transform: Transform,
    tag: u32,
    sprite: Option<Sprite>,
    disc: Option<Disc>,
    camera: Option<Camera>,
    tilemap: Option<Tilemap>,
}

/// Tiny entity world — Unity GameObject feel without a full ECS.
#[derive(Clone, Debug, Default)]
pub struct World {
    slots: Vec<Slot>,
}

impl World {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    pub fn spawn(&mut self, transform: Transform) -> EntityId {
        self.spawn_named("", transform)
    }

    pub fn spawn_named(&mut self, name: impl Into<String>, transform: Transform) -> EntityId {
        let name = name.into();
        if let Some((idx, slot)) = self.slots.iter_mut().enumerate().find(|(_, s)| !s.live) {
            slot.live = true;
            slot.name = name;
            slot.transform = transform;
            slot.tag = 0;
            slot.sprite = None;
            slot.disc = None;
            slot.camera = None;
            slot.tilemap = None;
            return EntityId(idx as u32);
        }
        let id = EntityId(self.slots.len() as u32);
        self.slots.push(Slot {
            live: true,
            name,
            transform,
            tag: 0,
            sprite: None,
            disc: None,
            camera: None,
            tilemap: None,
        });
        id
    }

    pub fn despawn(&mut self, id: EntityId) {
        if let Some(slot) = self.slots.get_mut(id.0 as usize) {
            slot.live = false;
        }
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }

    pub fn find_by_name(&self, name: &str) -> Option<EntityId> {
        self.slots.iter().enumerate().find_map(|(i, s)| {
            if s.live && s.name == name {
                Some(EntityId(i as u32))
            } else {
                None
            }
        })
    }

    pub fn name(&self, id: EntityId) -> Option<&str> {
        self.slot(id).map(|s| s.name.as_str())
    }

    pub fn set_name(&mut self, id: EntityId, name: impl Into<String>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.name = name.into();
        }
    }

    pub fn transform(&self, id: EntityId) -> Option<&Transform> {
        self.slot(id).map(|s| &s.transform)
    }

    pub fn transform_mut(&mut self, id: EntityId) -> Option<&mut Transform> {
        self.slot_mut(id).map(|s| &mut s.transform)
    }

    pub fn sprite(&self, id: EntityId) -> Option<&Sprite> {
        self.slot(id).and_then(|s| s.sprite.as_ref())
    }

    pub fn sprite_mut(&mut self, id: EntityId) -> Option<&mut Sprite> {
        self.slot_mut(id).and_then(|s| s.sprite.as_mut())
    }

    pub fn set_sprite(&mut self, id: EntityId, sprite: Option<Sprite>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.sprite = sprite;
        }
    }

    pub fn disc(&self, id: EntityId) -> Option<&Disc> {
        self.slot(id).and_then(|s| s.disc.as_ref())
    }

    pub fn disc_mut(&mut self, id: EntityId) -> Option<&mut Disc> {
        self.slot_mut(id).and_then(|s| s.disc.as_mut())
    }

    pub fn set_disc(&mut self, id: EntityId, disc: Option<Disc>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.disc = disc;
        }
    }

    pub fn camera(&self, id: EntityId) -> Option<&Camera> {
        self.slot(id).and_then(|s| s.camera.as_ref())
    }

    pub fn set_camera(&mut self, id: EntityId, camera: Option<Camera>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.camera = camera;
        }
    }

    pub fn set_tag(&mut self, id: EntityId, tag: u32) {
        if let Some(slot) = self.slot_mut(id) {
            slot.tag = tag;
        }
    }

    pub fn tag(&self, id: EntityId) -> Option<u32> {
        self.slot(id).map(|s| s.tag)
    }

    pub fn iter_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                Some(EntityId(i as u32))
            } else {
                None
            }
        })
    }

    pub fn iter_transforms(&self) -> impl Iterator<Item = (EntityId, &Transform)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                Some((EntityId(i as u32), &s.transform))
            } else {
                None
            }
        })
    }

    pub fn iter_sprites(&self) -> impl Iterator<Item = (EntityId, &Transform, &Sprite)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                s.sprite
                    .as_ref()
                    .map(|sp| (EntityId(i as u32), &s.transform, sp))
            } else {
                None
            }
        })
    }

    pub fn iter_discs(&self) -> impl Iterator<Item = (EntityId, &Transform, &Disc)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                s.disc
                    .as_ref()
                    .map(|d| (EntityId(i as u32), &s.transform, d))
            } else {
                None
            }
        })
    }

    pub fn tilemap(&self, id: EntityId) -> Option<&Tilemap> {
        self.slot(id).and_then(|s| s.tilemap.as_ref())
    }

    pub fn tilemap_mut(&mut self, id: EntityId) -> Option<&mut Tilemap> {
        self.slot_mut(id).and_then(|s| s.tilemap.as_mut())
    }

    pub fn set_tilemap(&mut self, id: EntityId, tilemap: Option<Tilemap>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.tilemap = tilemap;
        }
    }

    pub fn iter_tilemaps(&self) -> impl Iterator<Item = (EntityId, &Transform, &Tilemap)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                s.tilemap
                    .as_ref()
                    .map(|tm| (EntityId(i as u32), &s.transform, tm))
            } else {
                None
            }
        })
    }

    fn slot(&self, id: EntityId) -> Option<&Slot> {
        self.slots.get(id.0 as usize).filter(|s| s.live)
    }

    fn slot_mut(&mut self, id: EntityId) -> Option<&mut Slot> {
        self.slots.get_mut(id.0 as usize).filter(|s| s.live)
    }
}
