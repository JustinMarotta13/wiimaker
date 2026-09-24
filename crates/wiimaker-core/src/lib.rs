//! Wiimaker core — platform-agnostic engine for Wii homebrew.
//!
//! Games talk to this crate only. Backends (`wiimaker-host`, `runtime/wii`)
//! interpret [`draw::DrawList`] and feed [`input::Input`].

#![cfg_attr(not(feature = "std"), no_std)]

pub mod app;
pub mod audio;
pub mod collider;
pub mod color;
pub mod draw;
pub mod float;
pub mod grid_mover;
pub mod input;
pub mod math;
pub mod sorting;
pub mod text;
pub mod tilemap;
pub mod time;
pub mod wiimote_map;
pub mod world;

pub use app::{App, FrameCtx};
pub use audio::{queue_awake_audio, AudioSource, Oneshot};
pub use collider::{
    move_and_collide, overlap_solid, overlapping, overlaps, triggers_entered, Collider,
    ColliderKind, MoveHit,
};
pub use color::Rgba8;
pub use draw::{DrawCmd, DrawList, MeshId, Rect, TextureId};
pub use grid_mover::{cardinal, cell_center, step_grid_movers, Dir, GridMover, CARDINAL_DEADZONE};
pub use input::{Button, Input, Stick};
pub use sorting::{
    cmp_sorting, default_sorting_layer_index, default_sorting_layers,
    is_default_sorting_layer_name, sorting_layer_index, DEFAULT_SORTING_LAYER,
    DEFAULT_SORTING_LAYERS,
};
pub use text::{default_text_size, line_origin_x, text_aabb, Text, TextAlign, FONT_CELL_PX};
pub use tilemap::{
    autotile_bits, tile_get, tile_solid, tile_solid_world, world_to_cell, world_to_cell_on,
    AutoTileMatch, TileVisual, Tilemap, AUTOTILE_E, AUTOTILE_N, AUTOTILE_S, AUTOTILE_W,
};
pub use time::Clock;
#[cfg(feature = "std")]
pub use wiimote_map::format_input_status;
pub use wiimote_map::{
    merge_pad, MapRow, MergedPad, PadSources, WpadExpansion, INPUT_LEGEND, MAP_ROWS,
    STICK_IDLE_DEADZONE,
};
pub use world::{
    Animation, Camera, Disc, EntityId, Follow, Sprite, Transform, World, SCREEN_H, SCREEN_W,
};
