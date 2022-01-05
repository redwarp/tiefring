use anyhow::Result;
use engine::Engine;

use crate::game::Game;

mod actions;
mod components;
mod engine;
mod game;
mod inputs;
mod map;
mod spawner;
mod systems;
mod utils;

const WIDTH_IN_TILES: i32 = 40;
const HEIGHT_IN_TILES: i32 = 25;

fn main() -> Result<()> {
    env_logger::init();
    let game = Game::new(WIDTH_IN_TILES * 2, HEIGHT_IN_TILES * 2);

    let mut engine = Engine::new(WIDTH_IN_TILES, HEIGHT_IN_TILES);
    engine.run(game)?;

    Ok(())
}
