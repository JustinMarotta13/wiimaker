//! Tiny `cdylib` App used by `wiimaker-play` plugin-load tests.

use std::path::Path;

use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::{Transform, World};
use wiimaker_play::{export_play_app, PlayApp};

struct Probe {
    world: World,
    ticks: u32,
}

impl PlayApp for Probe {
    fn load_for_play(
        _game_dir: &Path,
        _scene_json: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = World::new();
        world.spawn_named("Marker", Transform::from_xy(0.0, 0.0));
        Ok(Self { world, ticks: 0 })
    }

    fn world(&self) -> &World {
        &self.world
    }

    fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl App for Probe {
    fn title(&self) -> &str {
        "play-probe"
    }

    fn update(&mut self, ctx: &FrameCtx<'_>) {
        self.ticks += 1;
        if let Some(id) = self.world.find_by_name("Marker") {
            if let Some(xf) = self.world.transform_mut(id) {
                xf.translation.x += 1.0;
                xf.translation.y += ctx.input.main.x;
            }
        }
    }

    fn render(&mut self, _ctx: &FrameCtx<'_>, _draw: &mut DrawList) {}
}

export_play_app!(Probe);
