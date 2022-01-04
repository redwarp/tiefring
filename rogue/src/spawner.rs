use bevy_ecs::prelude::{Entity, World};
use tiefring::Color;

use crate::components::{
    Body, Health, Monster, MoveClose, MoveRandom, Name, Player, Position, Solid, Vision,
};

pub fn player(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Player)
        .insert(Position::new(x, y))
        .insert(Body::new('@', Color::rgb(1.0, 0.0, 0.0)))
        .insert(Vision::new(8))
        .insert(Health::full_health(32))
        .insert(Solid)
        .insert(Name("Player".to_string()))
        .id()
}

pub fn orc(world: &mut World, name: &str, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Monster)
        .insert(Position::new(x, y))
        .insert(Body::new('o', Color::rgb(0.2, 0.9, 0.2)))
        .insert(Solid)
        .insert(MoveClose)
        .insert(Vision::new(8))
        .insert(Health::full_health(12))
        .insert(Name(format!("Orc number {}", name)))
        .id()
}

pub fn deer(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn()
        .insert(Monster)
        .insert(Position::new(x, y))
        .insert(Body::new('d', Color::rgb(1.0, 0.5, 0.0)))
        .insert(Solid)
        .insert(MoveRandom)
        .insert(Health::full_health(8))
        .insert(Name("A deer".to_string()))
        .id()
}
