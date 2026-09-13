//! Scaffolded wiimaker game — edit scenes with `wiimaker edit {{name}}`.

mod game;

use game::Game;
use wiimaker_host::run_with_atlas;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (game, atlas) = Game::load()?;
    run_with_atlas(game, atlas)
}
