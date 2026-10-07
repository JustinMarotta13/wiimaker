//! Wiimaker core — platform-agnostic engine for Wii homebrew.
//!
//! Games talk to this crate only. Backends (`wiimaker-host`, `runtime/wii`)
//! interpret [`draw::DrawList`] and feed [`input::Input`]. Host and Wii both
//! go `World` → [`render::render_world`] → `DrawList` → backend flush.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod animator;
pub mod app;
pub mod audio;
pub mod collider;
pub mod color;
pub mod draw;
pub mod float;
pub mod grid_mover;
pub mod input;
pub mod math;
pub mod render;
pub mod sorting;
pub mod text;
pub mod tilemap;
pub mod time;
pub mod timeline;
pub mod wiimote_map;
pub mod world;

pub use animator::{
    Animator, AnimatorCondition, AnimatorParam, AnimatorParamKind, AnimatorState,
    AnimatorTransition,
};
pub use app::{App, FrameCtx};
pub use audio::{queue_awake_audio, AudioSource, Oneshot};
pub use collider::{
    move_and_collide, overlap_solid, overlapping, overlaps, triggers_entered, Collider,
    ColliderKind, MoveHit,
};
pub use color::Rgba8;
pub use draw::{DrawCmd, DrawList, MeshId, Rect, TextureId};
pub use grid_mover::{cardinal, cell_center, step_grid_movers, Dir, GridMover, CARDINAL_DEADZONE};
pub use input::{Button, Gesture, Input, Stick, SHAKE_DELTA_G, SWING_G};
pub use render::{render_world, render_world_ex};
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
pub use timeline::{
    Curve, CurveInterp, CurveKey, PlayableDirector, TimelineClipRuntime, TimelineSignal,
    TimelineTrackKind, TimelineTrackRuntime,
};
#[cfg(feature = "std")]
pub use wiimote_map::format_input_status;
pub use wiimote_map::{
    apply_accel, apply_ir_aim, host_mouse_tilt_to_accel, map_ir_raw_to_640, merge_pad,
    scale_wpad_accel_raw_to_g, MapRow, MergedPad, PadSources, WpadExpansion, INPUT_LEGEND,
    IR_GAME_H, IR_GAME_W, MAP_ROWS, STICK_IDLE_DEADZONE, WPAD_ACCEL_RAW_ONE, WPAD_ACCEL_RAW_ZERO,
};
pub use world::{
    Animation, Camera, Disc, EntityId, Follow, Sprite, Transform, World, SCREEN_H, SCREEN_W,
};
