use bevy_ecs::prelude::{Entity, World};
use tiefring::Color;

use crate::components::{Body, FieldOfView, Name, Player, Position, RandomMover};

pub fn player(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Position::new(x, y))
        .insert(Player)
        .insert(Body::new('@', Color::rgb(1.0, 0.0, 0.0)))
        .insert(FieldOfView::new(8))
        .insert(Name("Player".to_string()))
        .id()
}

pub fn orc(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Position::new(x, y))
        .insert(Body::new('o', Color::rgb(0.2, 0.9, 0.2)))
        .insert(RandomMover)
        .insert(Name("Orc".to_string()))
        .id()
}
