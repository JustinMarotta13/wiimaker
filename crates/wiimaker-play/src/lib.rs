//! Shared play-host for `wiimaker-host` and in-editor Play Mode.
//!
//! Games implement [`PlayApp`] and `export_play_app!` from a `cdylib`. The editor
//! loads that plugin and ticks [`wiimaker_core::App::update`] / `render`. Crates
//! without a plugin keep the WASD / `Player` fallback.

mod abi;
mod input_map;
mod plugin;
mod scene_tick;
mod session;
mod status;

pub use abi::{PlayInputC, PLAY_ABI_VERSION};
pub use input_map::{apply_pad_keys, PadKeys};
pub use plugin::{dylib_name, dylib_path, example_dylib_path, LoadedPlugin};
pub use scene_tick::{
    hydrate_play_world, tick_scene_systems, FallbackTickResult, SceneTickOpts, PLAYER_WASD_SPEED,
};
pub use session::{step_app, PlayKind, PlaySession, PlayStart};
pub use status::{
    build_play_plugin, inspect_play_plugin, package_has_cdylib, PlayStatus, FORCE_FALLBACK_ENV,
};
pub use wiimaker_core::wiimote_map;

pub use wiimaker_core;
pub use wiimaker_core::app::{App, FrameCtx};
pub use wiimaker_core::world::World;

use std::path::Path;

/// Game crate entry used by the play plugin ABI (and host `fn main`).
pub trait PlayApp: App + 'static {
    fn load_for_play(
        game_dir: &Path,
        scene_json: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn world(&self) -> &World;
    fn world_mut(&mut self) -> &mut World;
}

/// Export C ABI symbols so the editor can `dlopen` this crate's `cdylib`.
///
/// `$ty` must implement [`PlayApp`].
#[macro_export]
macro_rules! export_play_app {
    ($ty:ty) => {
        #[no_mangle]
        pub extern "C" fn wiimaker_play_abi_version() -> u32 {
            $crate::PLAY_ABI_VERSION
        }

        #[no_mangle]
        pub unsafe extern "C" fn wiimaker_play_create(
            game_dir: *const ::std::os::raw::c_char,
            scene_json: *const ::std::os::raw::c_char,
        ) -> *mut ::std::os::raw::c_void {
            $crate::abi_create::<$ty>(game_dir, scene_json)
        }

        #[no_mangle]
        pub unsafe extern "C" fn wiimaker_play_destroy(handle: *mut ::std::os::raw::c_void) {
            $crate::abi_destroy::<$ty>(handle)
        }

        #[no_mangle]
        pub unsafe extern "C" fn wiimaker_play_update(
            handle: *mut ::std::os::raw::c_void,
            input: *const $crate::PlayInputC,
            dt: f32,
            fb_w: u32,
            fb_h: u32,
        ) -> i32 {
            $crate::abi_update::<$ty>(handle, input, dt, fb_w, fb_h)
        }

        #[no_mangle]
        pub unsafe extern "C" fn wiimaker_play_render(
            handle: *mut ::std::os::raw::c_void,
            draw: *mut $crate::wiimaker_core::draw::DrawList,
            input: *const $crate::PlayInputC,
            dt: f32,
            fb_w: u32,
            fb_h: u32,
        ) -> i32 {
            $crate::abi_render::<$ty>(handle, draw, input, dt, fb_w, fb_h)
        }

        #[no_mangle]
        pub unsafe extern "C" fn wiimaker_play_world(
            handle: *mut ::std::os::raw::c_void,
        ) -> *mut $crate::World {
            $crate::abi_world::<$ty>(handle)
        }
    };
}

/// ABI helpers invoked from [`export_play_app!`]. Public so the macro can reach them.
pub use abi::{abi_create, abi_destroy, abi_render, abi_update, abi_world};
