//! Scaffolded wiimaker game — edit scenes with `wiimaker edit {{name}}`.

use std::path::PathBuf;

use wiimaker_assets::SpriteCatalog;
use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::World;
use wiimaker_host::{load_atlas_for_project, run_with_atlas, HostAudio, TextureAtlas};
use wiimaker_scene::{hydrate_with_catalog, load_project, load_scene, render_world};

struct Game {
    title: String,
    world: World,
    clear: wiimaker_core::Rgba8,
    audio: HostAudio,
    assets: PathBuf,
}

impl Game {
    fn load() -> Result<(Self, TextureAtlas), Box<dyn std::error::Error>> {
        let game_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = load_project(&game_dir)?;
        let atlas = load_atlas_for_project(&game_dir, &project)?;
        let scene = load_scene(&project.scene_path(&game_dir))?;
        let assets = project.assets_path(&game_dir);
        let catalog = SpriteCatalog::load_dir(&assets, |stem| atlas.size_of(stem))?;
        let mut world = hydrate_with_catalog(&scene, atlas.map(), Some(&catalog))?;
        world.queue_awake_audio();
        Ok((
            Self {
                title: project.title.clone(),
                world,
                clear: scene.clear_rgba(),
                audio: HostAudio::new(),
                assets,
            },
            atlas,
        ))
    }
}

impl App for Game {
    fn title(&self) -> &str {
        &self.title
    }

    fn update(&mut self, ctx: &FrameCtx<'_>) {
        self.world.step_grid_movers(ctx.input, ctx.clock.dt);
        self.world.follow_cameras();
        let _ = self.audio.play_world(&mut self.world, &self.assets);
    }

    fn render(&mut self, _ctx: &FrameCtx<'_>, draw: &mut DrawList) {
        render_world(&self.world, draw, self.clear);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (game, atlas) = Game::load()?;
    run_with_atlas(game, atlas)
}
