use bevy_ecs::prelude::{Entity, World};
use tiefring::Color;

use crate::components::{Body, Player, Position};

pub fn player(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Position::new(x, y))
        .insert(Player)
        .insert(Body::new('@', Color::rgb(1.0, 0.0, 0.0)))
        .id()
}

pub fn orc(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Position::new(x, y))
        .insert(Body::new('o', Color::rgb(0.2, 0.9, 0.2)))
        .id()
}
