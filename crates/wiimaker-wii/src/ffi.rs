//! C ABI from `runtime/wii/include/wiimaker_abi.h`.
//!
//! Real GX / tex / audio symbols live in the C runtime. With the `std` feature,
//! [`install_test_backend`] records calls for host unit tests (no libogc).

/// Mirror of `WiimakerInput` (GCN-layout sticks + button bits).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WiimakerInput {
    pub main_x: f32,
    pub main_y: f32,
    pub c_x: f32,
    pub c_y: f32,
    pub buttons: u32,
}

/// One recorded GX / audio / tex call (host tests).
#[cfg(feature = "std")]
#[derive(Clone, Debug, PartialEq)]
pub enum GxCall {
    SetClear { r: u8, g: u8, b: u8, a: u8 },
    DrawDisc { x: f32, y: f32, radius: f32, rgba8: u32 },
    DrawSprite {
        tex_id: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        rgba8: u32,
    },
    DrawText {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        align: u8,
        rgba8: u32,
    },
    DrawQuad { x: f32, y: f32, w: f32, h: f32, rgba8: u32 },
    TexLoad { size: u32 },
    AudioLoad { size: u32 },
    AudioQueue { clip_id: u32, volume: f32 },
    AudioFlush,
    AudioShutdown,
    TexShutdown,
}

#[cfg(feature = "std")]
use std::cell::RefCell;

#[cfg(feature = "std")]
thread_local! {
    static TEST_LOG: RefCell<Option<Vec<GxCall>>> = const { RefCell::new(None) };
}

/// Begin recording GX calls on this thread (unit tests).
#[cfg(feature = "std")]
pub fn install_test_backend() {
    TEST_LOG.with(|log| {
        *log.borrow_mut() = Some(Vec::new());
    });
}

/// Stop recording and return the call log.
#[cfg(feature = "std")]
pub fn take_test_log() -> Vec<GxCall> {
    TEST_LOG.with(|log| log.borrow_mut().take().unwrap_or_default())
}

/// Peek at the current log without clearing (tests).
#[cfg(feature = "std")]
pub fn test_log_snapshot() -> Vec<GxCall> {
    TEST_LOG.with(|log| log.borrow().clone().unwrap_or_default())
}

#[cfg(feature = "std")]
fn push(call: GxCall) {
    TEST_LOG.with(|log| {
        if let Some(v) = log.borrow_mut().as_mut() {
            v.push(call);
        }
    });
}

#[cfg(feature = "std")]
#[allow(dead_code)]
fn use_test() -> bool {
    TEST_LOG.with(|log| log.borrow().is_some())
}

#[cfg(not(feature = "std"))]
#[allow(dead_code)]
fn use_test() -> bool {
    false
}

pub fn gx_set_clear(r: u8, g: u8, b: u8, a: u8) {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::SetClear { r, g, b, a });
        return;
    }
    unsafe { wiimaker_gx_set_clear(r, g, b, a) }
}

pub fn gx_draw_disc(x: f32, y: f32, radius: f32, rgba8: u32) {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::DrawDisc {
            x,
            y,
            radius,
            rgba8,
        });
        return;
    }
    unsafe { wiimaker_gx_draw_disc(x, y, radius, rgba8) }
}

#[allow(clippy::too_many_arguments)]
pub fn gx_draw_sprite(
    tex_id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    rgba8: u32,
) {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::DrawSprite {
            tex_id,
            x,
            y,
            w,
            h,
            u0,
            v0,
            u1,
            v1,
            rgba8,
        });
        return;
    }
    unsafe { wiimaker_gx_draw_sprite(tex_id, x, y, w, h, u0, v0, u1, v1, rgba8) }
}

pub fn gx_draw_text(x: f32, y: f32, text: &str, size: f32, align: u8, rgba8: u32) {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::DrawText {
            x,
            y,
            text: text.to_string(),
            size,
            align,
            rgba8,
        });
        return;
    }
    unsafe {
        wiimaker_gx_draw_text(
            x,
            y,
            text.as_ptr() as *const core::ffi::c_char,
            text.len() as u16,
            size,
            align,
            rgba8,
        )
    }
}

pub fn gx_draw_quad(x: f32, y: f32, w: f32, h: f32, rgba8: u32) {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::DrawQuad { x, y, w, h, rgba8 });
        return;
    }
    unsafe { wiimaker_gx_draw_quad(x, y, w, h, rgba8) }
}

pub fn tex_load_wpack(data: &[u8]) -> i32 {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::TexLoad {
            size: data.len() as u32,
        });
        return 0;
    }
    unsafe { wiimaker_tex_load_wpack(data.as_ptr(), data.len() as u32) }
}

pub fn tex_width(tex_id: u32) -> u16 {
    #[cfg(feature = "std")]
    if use_test() {
        return 64;
    }
    unsafe { wiimaker_tex_width(tex_id) }
}

pub fn tex_height(tex_id: u32) -> u16 {
    #[cfg(feature = "std")]
    if use_test() {
        return 64;
    }
    unsafe { wiimaker_tex_height(tex_id) }
}

pub fn tex_count() -> u32 {
    #[cfg(feature = "std")]
    if use_test() {
        return 8;
    }
    unsafe { wiimaker_tex_count() }
}

pub fn tex_shutdown() {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::TexShutdown);
        return;
    }
    unsafe { wiimaker_tex_shutdown() }
}

pub fn audio_load_wpack(data: &[u8]) -> i32 {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::AudioLoad {
            size: data.len() as u32,
        });
        return 0;
    }
    unsafe { wiimaker_audio_load_wpack(data.as_ptr(), data.len() as u32) }
}

pub fn audio_queue(clip_id: u32, volume: f32) -> i32 {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::AudioQueue { clip_id, volume });
        return 0;
    }
    unsafe { wiimaker_audio_queue(clip_id, volume) }
}

pub fn audio_flush() {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::AudioFlush);
        return;
    }
    unsafe { wiimaker_audio_flush() }
}

pub fn audio_shutdown() {
    #[cfg(feature = "std")]
    if use_test() {
        push(GxCall::AudioShutdown);
        return;
    }
    unsafe { wiimaker_audio_shutdown() }
}

#[cfg(target_arch = "powerpc")]
extern "C" {
    fn wiimaker_gx_set_clear(r: u8, g: u8, b: u8, a: u8);
    fn wiimaker_gx_draw_disc(x: f32, y: f32, radius: f32, rgba8: u32);
    fn wiimaker_gx_draw_sprite(
        tex_id: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        rgba8: u32,
    );
    fn wiimaker_gx_draw_text(
        x: f32,
        y: f32,
        text: *const core::ffi::c_char,
        len: u16,
        size: f32,
        align: u8,
        rgba8: u32,
    );
    fn wiimaker_gx_draw_quad(x: f32, y: f32, w: f32, h: f32, rgba8: u32);
    fn wiimaker_tex_load_wpack(data: *const u8, size: u32) -> i32;
    fn wiimaker_tex_width(tex_id: u32) -> u16;
    fn wiimaker_tex_height(tex_id: u32) -> u16;
    fn wiimaker_tex_count() -> u32;
    fn wiimaker_tex_shutdown();
    fn wiimaker_audio_load_wpack(data: *const u8, size: u32) -> i32;
    fn wiimaker_audio_queue(clip_id: u32, volume: f32) -> i32;
    fn wiimaker_audio_flush();
    fn wiimaker_audio_shutdown();
}

/// Host stubs so `rlib` / unit tests link without libogc.
#[cfg(not(target_arch = "powerpc"))]
mod host_stubs {
    pub unsafe fn wiimaker_gx_set_clear(_r: u8, _g: u8, _b: u8, _a: u8) {}
    pub unsafe fn wiimaker_gx_draw_disc(_x: f32, _y: f32, _radius: f32, _rgba8: u32) {}
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn wiimaker_gx_draw_sprite(
        _tex_id: u32,
        _x: f32,
        _y: f32,
        _w: f32,
        _h: f32,
        _u0: f32,
        _v0: f32,
        _u1: f32,
        _v1: f32,
        _rgba8: u32,
    ) {
    }
    pub unsafe fn wiimaker_gx_draw_text(
        _x: f32,
        _y: f32,
        _text: *const core::ffi::c_char,
        _len: u16,
        _size: f32,
        _align: u8,
        _rgba8: u32,
    ) {
    }
    pub unsafe fn wiimaker_gx_draw_quad(_x: f32, _y: f32, _w: f32, _h: f32, _rgba8: u32) {}
    pub unsafe fn wiimaker_tex_load_wpack(_data: *const u8, _size: u32) -> i32 {
        0
    }
    pub unsafe fn wiimaker_tex_width(_tex_id: u32) -> u16 {
        0
    }
    pub unsafe fn wiimaker_tex_height(_tex_id: u32) -> u16 {
        0
    }
    pub unsafe fn wiimaker_tex_count() -> u32 {
        0
    }
    pub unsafe fn wiimaker_tex_shutdown() {}
    pub unsafe fn wiimaker_audio_load_wpack(_data: *const u8, _size: u32) -> i32 {
        0
    }
    pub unsafe fn wiimaker_audio_queue(_clip_id: u32, _volume: f32) -> i32 {
        0
    }
    pub unsafe fn wiimaker_audio_flush() {}
    pub unsafe fn wiimaker_audio_shutdown() {}
}

#[cfg(not(target_arch = "powerpc"))]
use host_stubs::*;
