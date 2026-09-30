//! Window + input loop for host games.

use std::time::Instant;

use minifb::{Key, KeyRepeat, MouseMode, Window, WindowOptions};

use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::draw::DrawList;
use wiimaker_core::input::Input;
use wiimaker_core::time::Clock;
use wiimaker_core::wiimote_map::{
    apply_accel, apply_ir_aim, host_mouse_tilt_to_accel, map_ir_raw_to_640, IR_GAME_H, IR_GAME_W,
};
use wiimaker_play::{apply_pad_keys, step_app, PadKeys};

use crate::atlas::TextureAtlas;
use crate::raster::{self, Framebuffer};

const DEFAULT_W: usize = 640;
const DEFAULT_H: usize = 480;

/// Run an [`App`] on the desktop until the window closes.
pub fn run<A: App>(app: A) -> Result<(), Box<dyn std::error::Error>> {
    run_with_atlas(app, TextureAtlas::empty())
}

/// Run with a preloaded texture atlas (from `.wpack`).
pub fn run_with_atlas<A: App>(
    mut app: A,
    atlas: TextureAtlas,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut window = Window::new(
        app.title(),
        DEFAULT_W,
        DEFAULT_H,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);

    let mut fb = Framebuffer::new(DEFAULT_W, DEFAULT_H);
    let mut draw = DrawList::new();
    let mut input = Input::new();
    let mut clock = Clock::new(60.0);
    let mut last = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let real_dt = now.duration_since(last).as_secs_f32();
        last = now;

        poll_input(&mut window, &mut input);

        let steps = clock.push_real(real_dt);
        let ctx = FrameCtx {
            input: &input,
            clock: &clock,
            framebuffer_w: DEFAULT_W as u32,
            framebuffer_h: DEFAULT_H as u32,
        };
        for _ in 0..steps {
            step_app(
                &mut app,
                ctx.input,
                ctx.clock,
                ctx.framebuffer_w,
                ctx.framebuffer_h,
            );
        }

        draw.clear_buffer();
        app.render(&ctx, &mut draw);
        raster::flush_with_atlas(&draw, &mut fb, Some(&atlas));

        window.update_with_buffer(&fb.pixels, fb.width, fb.height)?;
    }

    Ok(())
}

fn poll_input(window: &mut Window, input: &mut Input) {
    input.begin_frame();
    apply_pad_keys(
        input,
        PadKeys {
            left: key(window, Key::Left) || key(window, Key::A),
            right: key(window, Key::Right) || key(window, Key::D),
            up: key(window, Key::Up) || key(window, Key::W),
            down: key(window, Key::Down) || key(window, Key::S),
            a: key(window, Key::Z) || key(window, Key::Space),
            b: key(window, Key::X),
            start: key(window, Key::Enter),
        },
    );
    // Host mouse → IR aim in 640×480 when over the game framebuffer and focused.
    // Shift + mouse offset from center → accel tilt (g); valid only while Shift held.
    let shift = key(window, Key::LeftShift) || key(window, Key::RightShift);
    if window.is_active() {
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
            let (x, y) = map_ir_raw_to_640(mx, my, IR_GAME_W, IR_GAME_H);
            apply_ir_aim(input, x, y, true);
            if shift {
                let nx = ((mx / IR_GAME_W) * 2.0 - 1.0).clamp(-1.0, 1.0);
                let ny = ((my / IR_GAME_H) * 2.0 - 1.0).clamp(-1.0, 1.0);
                let (ax, ay, az) = host_mouse_tilt_to_accel(nx, ny, 2.0);
                apply_accel(input, ax, ay, az, true);
            } else {
                apply_accel(input, 0.0, 0.0, 0.0, false);
            }
        } else {
            apply_ir_aim(input, 0.0, 0.0, false);
            apply_accel(input, 0.0, 0.0, 0.0, false);
        }
    } else {
        apply_ir_aim(input, 0.0, 0.0, false);
        apply_accel(input, 0.0, 0.0, 0.0, false);
    }
}

fn key(window: &Window, k: Key) -> bool {
    window.is_key_pressed(k, KeyRepeat::Yes) || window.is_key_down(k)
}
