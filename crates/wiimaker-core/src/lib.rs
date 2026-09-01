//! Wiimaker core — platform-agnostic engine for Wii homebrew.
//!
//! Games talk to this crate only. Backends (`wiimaker-host`, `runtime/wii`)
//! interpret [`draw::DrawList`] and feed [`input::Input`].

#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod collider;
pub mod color;
pub mod draw;
pub mod input;
pub mod math;
pub mod tilemap;
pub mod time;
pub mod world;

pub use app::{App, FrameCtx};
pub use collider::{
    move_and_collide, overlap_solid, overlapping, overlaps, triggers_entered, Collider,
    ColliderKind, MoveHit,
};
pub use color::Rgba8;
pub use draw::{DrawCmd, DrawList, MeshId, Rect, TextureId};
pub use input::{Button, Input, Stick};
pub use tilemap::{
    tile_get, tile_solid, tile_solid_world, world_to_cell, world_to_cell_on, TileVisual, Tilemap,
};
pub use time::Clock;
pub use world::{Animation, Camera, Disc, EntityId, Sprite, Transform, World};
