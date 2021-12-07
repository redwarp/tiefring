use std::time::Duration;

use bevy_ecs::prelude::World;

pub struct Game {
    pub world: World,
}

impl Game {
    pub fn new() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub fn update(&mut self, dt: Duration) {}
}
