//! Game `App` — host `fn main` and in-editor Play both tick this.

use std::path::{Path, PathBuf};

use wiimaker_assets::{AnimClipCatalog, SpriteCatalog};
use wiimaker_core::app::{App, FrameCtx};
use wiimaker_core::draw::DrawList;
use wiimaker_core::world::World;
use wiimaker_host::{load_atlas_for_project, HostAudio, TextureAtlas};
use wiimaker_play::{hydrate_play_world, tick_scene_systems, PlayApp, SceneTickOpts};
use wiimaker_scene::{load_project, render_world, TextureMap};

pub struct Game {
    title: String,
    world: World,
    clear: wiimaker_core::Rgba8,
    audio: HostAudio,
    assets: PathBuf,
    catalog: SpriteCatalog,
    textures: TextureMap,
}

impl Game {
    pub fn load() -> Result<(Self, TextureAtlas), Box<dyn std::error::Error>> {
        Self::load_from(&PathBuf::from(env!("CARGO_MANIFEST_DIR")), None)
    }

    pub fn load_from(
        game_dir: &Path,
        scene_json: Option<&str>,
    ) -> Result<(Self, TextureAtlas), Box<dyn std::error::Error>> {
        let project = load_project(game_dir)?;
        let atlas = load_atlas_for_project(game_dir, &project)?;
        let assets = project.assets_path(game_dir);
        let catalog = SpriteCatalog::load_dir(&assets, |stem| atlas.size_of(stem))?;
        let anims = AnimClipCatalog::load_dir(&assets)?;
        let mut world = World::new();
        let clear = hydrate_play_world(
            &mut world,
            game_dir,
            scene_json,
            atlas.map(),
            &catalog,
            &anims,
        )?;
        world.queue_awake_audio();
        let textures = atlas.map().clone();
        Ok((
            Self {
                title: project.title.clone(),
                world,
                clear,
                audio: HostAudio::new(),
                assets,
                catalog,
                textures,
            },
            atlas,
        ))
    }
}

impl PlayApp for Game {
    fn load_for_play(
        game_dir: &Path,
        scene_json: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::load_from(game_dir, scene_json)?.0)
    }

    fn world(&self) -> &World {
        &self.world
    }

    fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl App for Game {
    fn title(&self) -> &str {
        &self.title
    }

    fn update(&mut self, ctx: &FrameCtx<'_>) {
        tick_scene_systems(
            &mut self.world,
            ctx.input,
            ctx.clock.dt,
            Some(&self.catalog),
            Some(&self.textures),
            SceneTickOpts::default(),
        );
        let _ = self.audio.play_world(&mut self.world, &self.assets);
    }

    fn render(&mut self, _ctx: &FrameCtx<'_>, draw: &mut DrawList) {
        render_world(&self.world, draw, self.clear);
    }
}
