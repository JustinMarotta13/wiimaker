//! Scene player state — WSCN → World + hello-orb-ish Player / OrbShadow tick.

use wiimaker_core::color::Rgba8;
use wiimaker_core::draw::DrawList;
use wiimaker_core::render::render_world;
use wiimaker_core::world::{EntityId, World};
use wiimaker_core::{wiimote_map, Button, Input};

use crate::draw::flush_draw_list;
use crate::ffi::{self, WiimakerInput};
use crate::input_map::wiimaker_input_to_core;
use crate::wscn::{parse_audio_clip_id, parse_wscn, LoadedScene, ParseError};

/// Runtime game state owned by the staticlib.
pub struct GameState {
    pub world: World,
    pub draw: DrawList,
    pub clear: Rgba8,
    pub input: Input,
    pub screen_w: f32,
    pub screen_h: f32,
    pub player: Option<EntityId>,
    pub shadow: Option<EntityId>,
    pub base_radius: f32,
    pub pulse: f32,
    pub hue: f32,
    pub a_was_down: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            world: World::new(),
            draw: DrawList::new(),
            clear: Rgba8::BLACK,
            input: Input::new(),
            screen_w: 640.0,
            screen_h: 480.0,
            player: None,
            shadow: None,
            base_radius: 36.0,
            pulse: 0.0,
            hue: 0.0,
            a_was_down: false,
        }
    }
}

impl GameState {
    pub fn from_scene(scene: LoadedScene, fb_w: u32, fb_h: u32) -> Self {
        let player = scene.world.find_by_name("Player");
        let shadow = scene.world.find_by_name("OrbShadow");
        let base_radius = player
            .and_then(|id| scene.world.disc(id).map(|d| d.radius))
            .filter(|r| *r > 0.0)
            .unwrap_or(36.0);
        let clear = scene.clear_rgba();
        ffi::gx_set_clear(
            scene.clear[0],
            scene.clear[1],
            scene.clear[2],
            scene.clear[3],
        );
        Self {
            world: scene.world,
            draw: DrawList::new(),
            clear,
            input: Input::new(),
            screen_w: fb_w as f32,
            screen_h: fb_h as f32,
            player,
            shadow,
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

    pub fn queue_awake_audio(&mut self) {
        self.world.queue_awake_audio();
        for shot in self.world.drain_oneshots() {
            if let Some(id) = parse_audio_clip_id(&shot.clip) {
                let _ = ffi::audio_queue(id, shot.volume);
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
        let mut g = Self::load_wscn(wscn, fb_w, fb_h)?;
        g.queue_awake_audio();
        Ok(g)
    }

    fn orb_rgba(&self) -> Rgba8 {
        let t = libm_sin(self.hue * 6.2831853) * 0.5 + 0.5;
        let r = lerp(72.0, 255.0, t) + self.pulse * 40.0;
        let g = lerp(210.0, 96.0, t) + self.pulse * 16.0;
        let b = lerp(160.0, 88.0, t) + self.pulse * 8.0;
        Rgba8::new(
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
            255,
        )
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

        if let Some(pid) = self.player {
            let r = self.base_radius * (1.0 + self.pulse * 0.35);
            let speed = 220.0 * dt;
            if let Some(xf) = self.world.transform_mut(pid) {
                xf.translation.x = (xf.translation.x + mx * speed).clamp(r, self.screen_w - r);
                xf.translation.y = (xf.translation.y - my * speed).clamp(r, self.screen_h - r);
            }
            if let Some(sid) = self.shadow {
                if let Some((px, py)) = self
                    .world
                    .transform(pid)
                    .map(|t| (t.translation.x, t.translation.y))
                {
                    if let Some(sx) = self.world.transform_mut(sid) {
                        sx.translation.x = px + 4.0;
                        sx.translation.y = py + 6.0;
                    }
                }
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

        if let Some(pid) = self.player {
            let radius = self.base_radius * (1.0 + self.pulse * 0.35);
            let color = self.orb_rgba();
            if let Some(d) = self.world.disc_mut(pid) {
                d.radius = radius;
                d.color = color;
            }
        }

        let _ = self.world.tick_tilemaps(dt);

        self.draw.clear_buffer();
        render_world(&self.world, &mut self.draw, self.clear);
        flush_draw_list(&self.draw);

        if self.input.down(Button::A) {
            ffi::gx_draw_disc(24.0, 24.0, 8.0, 0xff6058ff);
        }

        self.input.down(Button::Start) || (raw.buttons & wiimote_map::BTN_START) != 0
    }

    pub fn shutdown(&mut self) {
        self.world.clear();
        self.draw.clear_buffer();
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
