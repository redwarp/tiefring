use anyhow::Result;
use engine::Engine;

use crate::game::Game;

mod components;
mod engine;
mod game;
mod inputs;
mod spawner;

fn main() -> Result<()> {
    let game = Game::new();

    let mut engine = Engine::new();
    engine.run(game)?;

    Ok(())
}
