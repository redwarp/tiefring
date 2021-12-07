use anyhow::Result;
use engine::Engine;

use crate::game::Game;

mod engine;
mod game;

fn main() -> Result<()> {
    let mut game = Game::new();

    let mut engine = Engine::new();
    engine.run(game)?;

    Ok(())
}
