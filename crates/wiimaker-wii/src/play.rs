//! Scene player state — WSCN load + hello-orb-ish Player / OrbShadow tick.

use wiimaker_core::{wiimote_map, Button, Input};

use crate::draw::{flush_entities, tick_tilemaps};
use crate::ffi::{self, WiimakerInput};
use crate::input_map::wiimaker_input_to_core;
use crate::wscn::{parse_wscn, Entity, LoadedScene, ParseError};

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Runtime game state owned by the staticlib.
pub struct GameState {
    pub entities: Vec<Entity>,
    pub input: Input,
    pub screen_w: f32,
    pub screen_h: f32,
    pub player_i: Option<usize>,
    pub shadow_i: Option<usize>,
    pub base_radius: f32,
    pub pulse: f32,
    pub hue: f32,
    pub a_was_down: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            input: Input::new(),
            screen_w: 640.0,
            screen_h: 480.0,
            player_i: None,
            shadow_i: None,
            base_radius: 36.0,
            pulse: 0.0,
            hue: 0.0,
            a_was_down: false,
        }
    }
}

fn find_named(entities: &[Entity], name: &str) -> Option<usize> {
    entities.iter().position(|e| e.name == name)
}

impl GameState {
    pub fn from_scene(scene: LoadedScene, fb_w: u32, fb_h: u32) -> Self {
        let player_i = find_named(&scene.entities, "Player");
        let shadow_i = find_named(&scene.entities, "OrbShadow");
        let base_radius = player_i
            .and_then(|i| {
                let r = scene.entities[i].radius;
                if r > 0.0 {
                    Some(r)
                } else {
                    None
                }
            })
            .unwrap_or(36.0);
        ffi::gx_set_clear(
            scene.clear[0],
            scene.clear[1],
            scene.clear[2],
            scene.clear[3],
        );
        Self {
            entities: scene.entities,
            input: Input::new(),
            screen_w: fb_w as f32,
            screen_h: fb_h as f32,
            player_i,
            shadow_i,
            base_radius,
            pulse: 0.0,
            hue: 0.0,
            a_was_down: false,
        }
    }

    pub fn load_wscn(data: &[u8], fb_w: u32, fb_h: u32) -> Result<Self, ParseError> {
        let scene = parse_wscn(data)?;
        Ok(Self::from_scene(scene, fb_w, fb_h))
    }

    pub fn queue_awake_audio(&self) {
        for e in &self.entities {
            if e.has_audio && e.audio_awake {
                let _ = ffi::audio_queue(e.audio_clip as u32, e.audio_volume);
            }
        }
        ffi::audio_flush();
    }

    /// Init from embedded wpack + wscn bytes (host tests pass slices).
    pub fn init_from_bytes(
        wpack: &[u8],
        wscn: &[u8],
        fb_w: u32,
        fb_h: u32,
    ) -> Result<Self, ParseError> {
        let _ = ffi::tex_load_wpack(wpack);
        let _ = ffi::audio_load_wpack(wpack);
        let g = Self::load_wscn(wscn, fb_w, fb_h)?;
        g.queue_awake_audio();
        Ok(g)
    }

    fn orb_rgba(&self) -> u32 {
        let t = libm_sin(self.hue * 6.2831853) * 0.5 + 0.5;
        let r = lerp(72.0, 255.0, t) + self.pulse * 40.0;
        let g = lerp(210.0, 96.0, t) + self.pulse * 16.0;
        let b = lerp(160.0, 88.0, t) + self.pulse * 8.0;
        let r = r.clamp(0.0, 255.0) as u32;
        let g = g.clamp(0.0, 255.0) as u32;
        let b = b.clamp(0.0, 255.0) as u32;
        (r << 24) | (g << 16) | (b << 8) | 0xff
    }

    /// One VI frame. Returns `true` when Start is held (quit).
    pub fn frame(&mut self, raw: &WiimakerInput, dt: f32) -> bool {
        wiimaker_input_to_core(&mut self.input, raw);

        let mut mx = self.input.main.x;
        let mut my = self.input.main.y;
        if mx.abs() < 0.15 && my.abs() < 0.15 {
            mx = 0.0;
            my = 0.0;
            if self.input.down(Button::DPadLeft) {
                mx -= 1.0;
            }
            if self.input.down(Button::DPadRight) {
                mx += 1.0;
            }
            if self.input.down(Button::DPadUp) {
                my += 1.0;
            }
            if self.input.down(Button::DPadDown) {
                my -= 1.0;
            }
        }

        if let Some(pi) = self.player_i {
            let r = self.base_radius * (1.0 + self.pulse * 0.35);
            let speed = 220.0 * dt;
            let player = &mut self.entities[pi];
            player.x = (player.x + mx * speed).clamp(r, self.screen_w - r);
            player.y = (player.y - my * speed).clamp(r, self.screen_h - r);
            if let Some(si) = self.shadow_i {
                let (px, py) = (player.x, player.y);
                self.entities[si].x = px + 4.0;
                self.entities[si].y = py + 6.0;
            }
        }

        let a_down = self.input.down(Button::A);
        if a_down && !self.a_was_down {
            self.pulse = 1.0;
        }
        self.a_was_down = a_down;
        self.pulse = (self.pulse - dt * 2.5).clamp(0.0, 1.0);

        self.hue += dt * 0.35;
        if self.hue > 1.0 {
            self.hue -= 1.0;
        }

        if let Some(pi) = self.player_i {
            self.entities[pi].radius = self.base_radius * (1.0 + self.pulse * 0.35);
        }

        tick_tilemaps(&mut self.entities, dt);

        let player_rgba = self.player_i.map(|i| (i, self.orb_rgba()));
        flush_entities(&self.entities, player_rgba);

        if self.input.down(Button::A) {
            ffi::gx_draw_disc(24.0, 24.0, 8.0, 0xff6058ff);
        }

        self.input.down(Button::Start)
            || (raw.buttons & wiimote_map::BTN_START) != 0
    }

    pub fn shutdown(&mut self) {
        self.entities.clear();
        ffi::audio_shutdown();
        ffi::tex_shutdown();
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(feature = "std")]
fn libm_sin(x: f32) -> f32 {
    x.sin()
}

#[cfg(not(feature = "std"))]
fn libm_sin(x: f32) -> f32 {
    // glam/libm-backed; avoid pulling libm crate — simple poly for orb pulse.
    let mut x = x % 6.2831853;
    if x > 3.14159265 {
        x -= 6.2831853;
    } else if x < -3.14159265 {
        x += 6.2831853;
    }
    let x2 = x * x;
    x * (1.0 - x2 * (1.0 / 6.0 - x2 * (1.0 / 120.0)))
}
