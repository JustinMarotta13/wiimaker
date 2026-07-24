//! Wiimaker core — platform-agnostic engine for Wii homebrew.
//!
//! Games talk to this crate only. Backends (`wiimaker-host`, `runtime/wii`)
//! interpret [`draw::DrawList`] and feed [`input::Input`].

#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod color;
pub mod draw;
pub mod input;
pub mod math;
pub mod time;
pub mod world;

pub use app::{App, FrameCtx};
pub use color::Rgba8;
pub use draw::{DrawCmd, DrawList, MeshId, Rect, TextureId};
pub use input::{Button, Input, Stick};
pub use time::Clock;
pub use world::{Camera, Disc, EntityId, Sprite, Transform, World};
