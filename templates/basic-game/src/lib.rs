//! Play plugin (`cdylib`) so in-editor Play can tick this crate's [`Game`] `App`.

mod game;

pub use game::Game;

wiimaker_play::export_play_app!(Game);
