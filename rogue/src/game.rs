use std::time::Duration;

use bevy_ecs::prelude::World;

use crate::inputs::Input;

pub struct Game {
    pub world: World,
}

impl Game {
    pub fn new() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub fn update(&mut self, _dt: Duration, input: Option<Input>) -> bool {
        match input {
            Some(Input::Escape) => true,
            _ => false,
        }
    }
}
