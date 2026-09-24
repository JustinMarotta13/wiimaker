//! `#[no_mangle]` game entry points called from `bootstrap.c`.

use core::ptr;

use crate::ffi::WiimakerInput;
use crate::play::GameState;

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

static mut GAME: *mut GameState = ptr::null_mut();

#[cfg(target_arch = "powerpc")]
extern "C" {
    static _binary_assets_wpack_start: u8;
    static _binary_assets_wpack_end: u8;
    static _binary_scene_wscn_start: u8;
    static _binary_scene_wscn_end: u8;
}

#[cfg(target_arch = "powerpc")]
unsafe fn embed_slice(start: *const u8, end: *const u8) -> &'static [u8] {
    let len = end.offset_from(start) as usize;
    core::slice::from_raw_parts(start, len)
}

#[no_mangle]
pub unsafe extern "C" fn wiimaker_game_init(fb_w: u32, fb_h: u32) {
    if !GAME.is_null() {
        let old = Box::from_raw(GAME);
        drop(old);
        GAME = ptr::null_mut();
    }

    #[cfg(target_arch = "powerpc")]
    let (wpack, wscn) = {
        let wpack = embed_slice(
            &_binary_assets_wpack_start as *const u8,
            &_binary_assets_wpack_end as *const u8,
        );
        let wscn = embed_slice(
            &_binary_scene_wscn_start as *const u8,
            &_binary_scene_wscn_end as *const u8,
        );
        (wpack, wscn)
    };
    #[cfg(not(target_arch = "powerpc"))]
    let (wpack, wscn): (&[u8], &[u8]) = (&[], &[]);

    match GameState::init_from_bytes(wpack, wscn, fb_w, fb_h) {
        Ok(g) => {
            GAME = Box::into_raw(Box::new(g));
        }
        Err(_) => {
            GAME = Box::into_raw(Box::new(GameState::default()));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn wiimaker_game_frame(input: *const WiimakerInput, dt: f32) -> i32 {
    if GAME.is_null() || input.is_null() {
        return 0;
    }
    let g = &mut *GAME;
    let quit = g.frame(&*input, dt);
    if quit {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn wiimaker_game_shutdown() {
    if GAME.is_null() {
        return;
    }
    let mut g = Box::from_raw(GAME);
    GAME = ptr::null_mut();
    g.shutdown();
}
