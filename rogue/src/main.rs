use anyhow::Result;
use engine::Engine;

use crate::game::Game;

mod components;
mod engine;
mod game;
mod inputs;
mod map;
mod spawner;
mod systems;

fn main() -> Result<()> {
    let game = Game::new();

    let mut engine = Engine::new();
    engine.run(game)?;

    Ok(())
}
