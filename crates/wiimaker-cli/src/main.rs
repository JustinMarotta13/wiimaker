//! `wiimaker` CLI — scaffold, cook, scene/entity edits, doctor, edit.

mod args;
mod cmds;
mod pipeline;
mod util;

use anyhow::Result;
use clap::Parser;

use args::{Cli, Cmd};
use cmds::{asset, entity, project, scene, tilemap};
use util::find_root;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = find_root()?;
    match cli.cmd {
        Cmd::New { name } => project::new_game(&root, &name, cli.json),
        Cmd::Run { name } => project::run_game(&root, &name),
        Cmd::Edit { name } => project::edit_game(&root, &name),
        Cmd::Cook {
            name,
            input,
            output,
        } => project::cook(&root, &name, input, output, cli.json),
        Cmd::BakeWii { name } => project::bake_wii(&root, &name, cli.json),
        Cmd::Build { name } => project::build(&root, &name, cli.json),
        Cmd::Dolphin { name } => project::dolphin(&root, &name, cli.json),
        Cmd::PlayWii { name } => project::play_wii(&root, &name, cli.json),
        Cmd::Doctor { name } => project::doctor_game(&root, &name, cli.json),
        Cmd::Scene { cmd } => scene::scene_cmd(&root, cmd, cli.json),
        Cmd::Entity { cmd } => entity::entity_cmd(&root, cmd, cli.json),
        Cmd::Asset { cmd } => asset::asset_cmd(&root, cmd, cli.json),
        Cmd::Tilemap { cmd } => tilemap::tilemap_cmd(&root, cmd, cli.json),
    }
}
