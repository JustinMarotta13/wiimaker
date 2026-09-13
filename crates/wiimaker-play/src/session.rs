//! Play session: plugin `App` when available, otherwise WASD fallback.

use std::path::Path;

use anyhow::Result;
use wiimaker_assets::SpriteCatalog;
use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::input::Input;
use wiimaker_core::time::Clock;
use wiimaker_core::world::World;
use wiimaker_scene::TextureMap;

use crate::plugin::{dylib_path, LoadedPlugin};
use crate::scene_tick::{tick_scene_systems, FallbackTickResult, SceneTickOpts};
use crate::status::{build_play_plugin, package_has_cdylib, FORCE_FALLBACK_ENV};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayKind {
    Plugin,
    Fallback,
}

pub struct PlayStart<'a> {
    pub workspace: &'a Path,
    pub package: &'a str,
    pub game_dir: &'a Path,
    pub scene_json: Option<&'a str>,
    /// Skip plugin even if the crate exports one (`WIIMAKER_PLAY_FALLBACK=1`).
    pub force_fallback: bool,
    /// `cargo build -p <game> --lib` when the dylib is missing / stale.
    pub build_if_needed: bool,
}

pub struct PlaySession {
    kind: PlayKind,
    plugin: Option<LoadedPlugin>,
    clock: Clock,
    logs: Vec<String>,
}

impl PlaySession {
    pub fn start(opts: PlayStart<'_>) -> Self {
        let force =
            opts.force_fallback || std::env::var(FORCE_FALLBACK_ENV).ok().as_deref() == Some("1");
        if force {
            return Self {
                kind: PlayKind::Fallback,
                plugin: None,
                clock: Clock::new(60.0),
                logs: vec!["Play Mode · fallback WASD (WIIMAKER_PLAY_FALLBACK)".into()],
            };
        }

        let mut logs = Vec::new();
        if package_has_cdylib(opts.game_dir) {
            let dylib = dylib_path(opts.workspace, opts.package);
            if opts.build_if_needed {
                match build_play_plugin(opts.workspace, opts.package) {
                    Ok(msg) => logs.push(msg),
                    Err(e) => {
                        logs.push(format!("play plugin build failed: {e} · WASD fallback"));
                        return Self {
                            kind: PlayKind::Fallback,
                            plugin: None,
                            clock: Clock::new(60.0),
                            logs,
                        };
                    }
                }
            }
            if dylib.is_file() {
                match LoadedPlugin::load(&dylib, opts.game_dir, opts.scene_json) {
                    Ok(plugin) => {
                        logs.push(format!(
                            "Play Mode · game App plugin ({}) · Esc stops",
                            opts.package
                        ));
                        return Self {
                            kind: PlayKind::Plugin,
                            plugin: Some(plugin),
                            clock: Clock::new(60.0),
                            logs,
                        };
                    }
                    Err(e) => logs.push(format!("play plugin load failed: {e} · WASD fallback")),
                }
            } else {
                logs.push(format!(
                    "play plugin missing ({}) · WASD fallback",
                    dylib.display()
                ));
            }
        } else {
            logs.push("Play Mode · WASD fallback (no cdylib play plugin) · Esc stops".into());
        }

        Self {
            kind: PlayKind::Fallback,
            plugin: None,
            clock: Clock::new(60.0),
            logs,
        }
    }

    pub fn kind(&self) -> PlayKind {
        self.kind
    }

    pub fn is_plugin(&self) -> bool {
        self.kind == PlayKind::Plugin
    }

    pub fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }

    pub fn world(&self) -> Option<&World> {
        self.plugin.as_ref().and_then(|p| p.world())
    }

    /// Fixed-timestep ticks. Plugin runs `App::update`; fallback mutates `world`.
    pub fn tick(
        &mut self,
        input: &Input,
        real_dt: f32,
        fallback_world: Option<&mut World>,
        catalog: Option<&SpriteCatalog>,
        textures: Option<&TextureMap>,
        fb_w: u32,
        fb_h: u32,
    ) -> Result<FallbackTickResult> {
        let mut last = FallbackTickResult::default();
        let steps = self.clock.push_real(real_dt);
        let dt = self.clock.dt;
        match self.kind {
            PlayKind::Plugin => {
                if let Some(p) = self.plugin.as_mut() {
                    for _ in 0..steps {
                        p.update(input, dt, fb_w, fb_h)?;
                    }
                }
            }
            PlayKind::Fallback => {
                if let Some(world) = fallback_world {
                    for _ in 0..steps {
                        last = tick_scene_systems(
                            world,
                            input,
                            dt,
                            catalog,
                            textures,
                            SceneTickOpts::default(),
                        );
                    }
                }
            }
        }
        Ok(last)
    }
}

/// One `App::update` with a borrowed clock (tests + host).
pub fn step_app(app: &mut dyn App, input: &Input, clock: &Clock, fb_w: u32, fb_h: u32) {
    let ctx = FrameCtx {
        input,
        clock,
        framebuffer_w: fb_w,
        framebuffer_h: fb_h,
    };
    app.update(&ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiimaker_core::draw::DrawList;

    struct CounterApp {
        ticks: u32,
    }

    impl App for CounterApp {
        fn update(&mut self, _ctx: &FrameCtx<'_>) {
            self.ticks += 1;
        }
        fn render(&mut self, _ctx: &FrameCtx<'_>, _draw: &mut DrawList) {}
    }

    #[test]
    fn step_app_invokes_update() {
        let mut app = CounterApp { ticks: 0 };
        let input = Input::new();
        let clock = Clock::new(60.0);
        step_app(&mut app, &input, &clock, 640, 480);
        step_app(&mut app, &input, &clock, 640, 480);
        assert_eq!(app.ticks, 2);
    }

    #[test]
    fn fallback_session_ticks_player_not_app_plugin() {
        let mut session = PlaySession {
            kind: PlayKind::Fallback,
            plugin: None,
            clock: Clock::new(60.0),
            logs: vec![],
        };
        let mut world = World::new();
        let id = world.spawn_named(
            "Player",
            wiimaker_core::world::Transform::from_xy(40.0, 40.0),
        );
        let mut input = Input::new();
        input.begin_frame();
        crate::apply_pad_keys(
            &mut input,
            crate::PadKeys {
                right: true,
                ..Default::default()
            },
        );
        session
            .tick(&input, 0.05, Some(&mut world), None, None, 640, 480)
            .unwrap();
        let x = world.transform(id).unwrap().translation.x;
        assert!(x > 40.0, "fallback should move Player, x={x}");
        assert!(!session.is_plugin());
    }
}
