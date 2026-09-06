//! Entity store with Unity-shaped components (Transform + Sprite/Disc/Camera/Tilemap/Collider/Animation).

use crate::collider::Collider;
use crate::color::Rgba8;
use crate::draw::{Rect, TextureId};
use crate::math::{Quat, Vec2, Vec3};
use crate::tilemap::Tilemap;

/// Host / scene framebuffer size (Wii VI analogue for v0 ortho).
pub const SCREEN_W: f32 = 640.0;
pub const SCREEN_H: f32 = 480.0;

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

/// Optional camera follow: lerp this entity's transform toward a named target.
#[derive(Clone, Debug)]
pub struct Follow {
    pub target: String,
    /// 0 = frozen, 1 = snap. Typical smooth follow is `0.15`.
    pub lerp: f32,
}

impl Follow {
    pub fn new(target: impl Into<String>, lerp: f32) -> Self {
        Self {
            target: target.into(),
            lerp: lerp.clamp(0.0, 1.0),
        }
    }
}

/// Sprite clip playback (Unity Animator / AnimationClip analogue for 2D cells).
#[derive(Clone, Debug)]
pub struct Animation {
    /// Clip asset stem (`assets/<clip>.anim.json`).
    pub clip: String,
    /// Resolved cell names from the clip (catalog lookups).
    pub cells: Vec<String>,
    pub fps: f32,
    pub loop_: bool,
    /// Accumulated seconds in the current cycle.
    pub time: f32,
    /// Current cell index into `cells`.
    pub frame: usize,
}

impl Animation {
    pub fn new(clip: impl Into<String>, cells: Vec<String>, fps: f32, loop_: bool) -> Self {
        Self {
            clip: clip.into(),
            cells,
            fps: fps.max(0.001),
            loop_,
            time: 0.0,
            frame: 0,
        }
    }

    pub fn cell_name(&self) -> Option<&str> {
        self.cells.get(self.frame).map(|s| s.as_str())
    }
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
    follow: Option<Follow>,
    tilemap: Option<Tilemap>,
    collider: Option<Collider>,
    animation: Option<Animation>,
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
            slot.follow = None;
            slot.tilemap = None;
            slot.collider = None;
            slot.animation = None;
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
            follow: None,
            tilemap: None,
            collider: None,
            animation: None,
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

    pub fn follow(&self, id: EntityId) -> Option<&Follow> {
        self.slot(id).and_then(|s| s.follow.as_ref())
    }

    pub fn set_follow(&mut self, id: EntityId, follow: Option<Follow>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.follow = follow;
        }
    }

    /// First live entity with an **active** Camera (spawn order).
    pub fn active_camera(&self) -> Option<(EntityId, &Transform, &Camera)> {
        self.slots.iter().enumerate().find_map(|(i, s)| {
            if !s.live {
                return None;
            }
            let cam = s.camera.as_ref()?;
            if !cam.active {
                return None;
            }
            Some((EntityId(i as u32), &s.transform, cam))
        })
    }

    /// World-space translation subtracted from dests so the active camera is
    /// centered in [`SCREEN_W`]×[`SCREEN_H`]. No active camera → `(0, 0)` (identity).
    pub fn camera_view_offset(&self) -> Vec2 {
        match self.active_camera() {
            Some((_, xf, _)) => Vec2::new(
                xf.translation.x - SCREEN_W * 0.5,
                xf.translation.y - SCREEN_H * 0.5,
            ),
            None => Vec2::ZERO,
        }
    }

    /// Move each entity that has [`Follow`] toward its named target (per-axis lerp).
    pub fn follow_cameras(&mut self) {
        let jobs: Vec<(EntityId, String, f32)> = self
            .iter_entities()
            .filter_map(|id| {
                let f = self.follow(id)?;
                if f.target.is_empty() {
                    return None;
                }
                Some((id, f.target.clone(), f.lerp.clamp(0.0, 1.0)))
            })
            .collect();
        for (cam_id, target, lerp) in jobs {
            let Some(tid) = self.find_by_name(&target) else {
                continue;
            };
            if tid == cam_id {
                continue;
            }
            let Some(tx) = self.transform(tid).map(|t| t.translation) else {
                continue;
            };
            if let Some(xf) = self.transform_mut(cam_id) {
                xf.translation.x += (tx.x - xf.translation.x) * lerp;
                xf.translation.y += (tx.y - xf.translation.y) * lerp;
            }
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

    pub fn collider(&self, id: EntityId) -> Option<&Collider> {
        self.slot(id).and_then(|s| s.collider.as_ref())
    }

    pub fn collider_mut(&mut self, id: EntityId) -> Option<&mut Collider> {
        self.slot_mut(id).and_then(|s| s.collider.as_mut())
    }

    pub fn set_collider(&mut self, id: EntityId, collider: Option<Collider>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.collider = collider;
        }
    }

    pub fn iter_colliders(&self) -> impl Iterator<Item = (EntityId, &Transform, &Collider)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                s.collider
                    .as_ref()
                    .map(|c| (EntityId(i as u32), &s.transform, c))
            } else {
                None
            }
        })
    }

    pub fn animation(&self, id: EntityId) -> Option<&Animation> {
        self.slot(id).and_then(|s| s.animation.as_ref())
    }

    pub fn animation_mut(&mut self, id: EntityId) -> Option<&mut Animation> {
        self.slot_mut(id).and_then(|s| s.animation.as_mut())
    }

    pub fn set_animation(&mut self, id: EntityId, animation: Option<Animation>) {
        if let Some(slot) = self.slot_mut(id) {
            slot.animation = animation;
        }
    }

    pub fn iter_animations(&self) -> impl Iterator<Item = (EntityId, &Animation)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            if s.live {
                s.animation.as_ref().map(|a| (EntityId(i as u32), a))
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

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    #[test]
    fn no_camera_offset_is_identity() {
        let world = World::new();
        assert_eq!(world.camera_view_offset(), Vec2::ZERO);
    }

    #[test]
    fn camera_at_screen_center_is_identity() {
        let mut world = World::new();
        let id = world.spawn_named("Cam", Transform::from_xy(SCREEN_W * 0.5, SCREEN_H * 0.5));
        world.set_camera(id, Some(Camera { active: true }));
        let o = world.camera_view_offset();
        assert!(o.x.abs() < 1e-4 && o.y.abs() < 1e-4);
    }

    #[test]
    fn inactive_camera_does_not_offset() {
        let mut world = World::new();
        let id = world.spawn_named("Cam", Transform::from_xy(100.0, 50.0));
        world.set_camera(id, Some(Camera { active: false }));
        assert_eq!(world.camera_view_offset(), Vec2::ZERO);
    }

    #[test]
    fn follow_lerps_toward_named_target() {
        let mut world = World::new();
        let cam = world.spawn_named("Cam", Transform::from_xy(0.0, 0.0));
        world.set_camera(cam, Some(Camera { active: true }));
        world.set_follow(cam, Some(Follow::new("Player", 0.5)));
        world.spawn_named("Player", Transform::from_xy(100.0, 40.0));
        world.follow_cameras();
        let t = world.transform(cam).unwrap().translation;
        assert!((t.x - 50.0).abs() < 1e-4);
        assert!((t.y - 20.0).abs() < 1e-4);
        world.follow_cameras();
        let t = world.transform(cam).unwrap().translation;
        assert!((t.x - 75.0).abs() < 1e-4);
        assert!((t.y - 30.0).abs() < 1e-4);
    }

    #[test]
    fn follow_snap_when_lerp_one() {
        let mut world = World::new();
        let cam = world.spawn_named("Cam", Transform::from_xy(0.0, 0.0));
        world.set_follow(cam, Some(Follow::new("Player", 1.0)));
        world.spawn_named("Player", Transform::from_xy(320.0, 240.0));
        world.follow_cameras();
        let t = world.transform(cam).unwrap().translation;
        assert_eq!(t.x, 320.0);
        assert_eq!(t.y, 240.0);
    }
}
