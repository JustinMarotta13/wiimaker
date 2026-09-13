//! C ABI for game `cdylib` play plugins. Layout is POD + opaque handles.
//!
//! `DrawList` / `World` pointers are Rust types shared with the editor (same
//! workspace `wiimaker-core`). Reload the plugin after a rustc or core bump.

use std::ffi::{c_char, c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use wiimaker_core::app::FrameCtx;
use wiimaker_core::draw::DrawList;
use wiimaker_core::input::{Button, Input};
use wiimaker_core::time::Clock;
use wiimaker_core::world::World;

use crate::PlayApp;

/// Bump when the C symbol set or [`PlayInputC`] layout changes.
pub const PLAY_ABI_VERSION: u32 = 1;

/// POD input snapshot (GCN layout bits match [`Input`] masks).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayInputC {
    pub main_x: f32,
    pub main_y: f32,
    pub c_x: f32,
    pub c_y: f32,
    pub l_analog: f32,
    pub r_analog: f32,
    pub down: u32,
    pub pressed: u32,
    pub released: u32,
}

const BUTTONS: [Button; 12] = [
    Button::A,
    Button::B,
    Button::X,
    Button::Y,
    Button::Start,
    Button::Z,
    Button::L,
    Button::R,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

impl PlayInputC {
    pub fn from_input(input: &Input) -> Self {
        let mut down = 0u32;
        let mut pressed = 0u32;
        let mut released = 0u32;
        for b in BUTTONS {
            let m = 1u32 << (b as u32);
            if input.down(b) {
                down |= m;
            }
            if input.pressed(b) {
                pressed |= m;
            }
            if input.released(b) {
                released |= m;
            }
        }
        Self {
            main_x: input.main.x,
            main_y: input.main.y,
            c_x: input.c.x,
            c_y: input.c.y,
            l_analog: input.l_analog,
            r_analog: input.r_analog,
            down,
            pressed,
            released,
        }
    }

    pub fn to_input(self) -> Input {
        let mut input = Input::new();
        input.main.x = self.main_x;
        input.main.y = self.main_y;
        input.c.x = self.c_x;
        input.c.y = self.c_y;
        input.l_analog = self.l_analog;
        input.r_analog = self.r_analog;
        for b in BUTTONS {
            let m = 1u32 << (b as u32);
            input.set_down(b, self.down & m != 0);
        }
        // `set_down` on a fresh Input treats every hold as a press. Restore edges.
        input.set_edge_bits(self.pressed, self.released);
        input
    }
}

pub struct PlayBox<A: PlayApp> {
    pub app: A,
}

fn c_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .ok()
        .filter(|s| !s.is_empty())
}

pub unsafe fn abi_create<A: PlayApp>(
    game_dir: *const c_char,
    scene_json: *const c_char,
) -> *mut c_void {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let dir = c_str(game_dir).map(Path::new);
        let json = c_str(scene_json);
        let Some(dir) = dir else {
            return Err("play plugin: null game_dir".into());
        };
        A::load_for_play(dir, json)
    }));
    match result {
        Ok(Ok(app)) => Box::into_raw(Box::new(PlayBox { app })) as *mut c_void,
        Ok(Err(_)) | Err(_) => std::ptr::null_mut(),
    }
}

pub unsafe fn abi_destroy<A: PlayApp>(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(handle as *mut PlayBox<A>));
    }));
}

fn with_box<A: PlayApp, T>(
    handle: *mut c_void,
    f: impl FnOnce(&mut PlayBox<A>) -> T,
) -> Result<T, ()> {
    if handle.is_null() {
        return Err(());
    }
    catch_unwind(AssertUnwindSafe(|| {
        let b = unsafe { &mut *(handle as *mut PlayBox<A>) };
        f(b)
    }))
    .map_err(|_| ())
}

fn frame_ctx<'a>(
    input: &'a Input,
    dt: f32,
    fb_w: u32,
    fb_h: u32,
    clock: &'a mut Clock,
) -> FrameCtx<'a> {
    let hz = if dt > 1e-6 { 1.0 / dt } else { 60.0 };
    *clock = Clock::new(hz);
    FrameCtx {
        input,
        clock,
        framebuffer_w: fb_w,
        framebuffer_h: fb_h,
    }
}

pub unsafe fn abi_update<A: PlayApp>(
    handle: *mut c_void,
    input: *const PlayInputC,
    dt: f32,
    fb_w: u32,
    fb_h: u32,
) -> i32 {
    if input.is_null() {
        return -1;
    }
    let pad = (*input).to_input();
    match with_box::<A, ()>(handle, |b| {
        let mut clock = Clock::new(60.0);
        let ctx = frame_ctx(&pad, dt, fb_w, fb_h, &mut clock);
        b.app.update(&ctx);
    }) {
        Ok(()) => 0,
        Err(()) => -1,
    }
}

pub unsafe fn abi_render<A: PlayApp>(
    handle: *mut c_void,
    draw: *mut DrawList,
    input: *const PlayInputC,
    dt: f32,
    fb_w: u32,
    fb_h: u32,
) -> i32 {
    if input.is_null() || draw.is_null() {
        return -1;
    }
    let pad = (*input).to_input();
    match with_box::<A, ()>(handle, |b| {
        let mut clock = Clock::new(60.0);
        let ctx = frame_ctx(&pad, dt, fb_w, fb_h, &mut clock);
        b.app.render(&ctx, &mut *draw);
    }) {
        Ok(()) => 0,
        Err(()) => -1,
    }
}

pub unsafe fn abi_world<A: PlayApp>(handle: *mut c_void) -> *mut World {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let b = &mut *(handle as *mut PlayBox<A>);
    b.app.world_mut() as *mut World
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::input::Button;

    #[test]
    fn play_input_roundtrip_stick_and_buttons() {
        let mut src = Input::new();
        src.main.x = 1.0;
        src.main.y = -0.5;
        src.set_down(Button::A, true);
        src.set_down(Button::DPadRight, true);
        let c = PlayInputC::from_input(&src);
        let dst = c.to_input();
        assert!((dst.main.x - 1.0).abs() < 1e-6);
        assert!((dst.main.y + 0.5).abs() < 1e-6);
        assert!(dst.down(Button::A));
        assert!(dst.down(Button::DPadRight));
        assert!(!dst.down(Button::B));
    }
}
