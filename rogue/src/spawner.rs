use bevy_ecs::prelude::{Commands, Entity, World};

use crate::components::{
    Body, BodyType, Health, Monster, MoveClose, MoveRandom, Name, Player, Position, Solid, Vision,
};

pub fn player(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn((
            Player,
            Position::new(x, y),
            Body::new(BodyType::Hero),
            Vision::new(8),
            Health::full_health(32),
            Solid,
            Name("Player".to_string()),
        ))
        .id()
}

pub fn orc(world: &mut World, name: &str, x: i32, y: i32) -> Entity {
    world
        .spawn((
            Monster,
            Position::new(x, y),
            Body::new(BodyType::Orc),
            Solid,
            MoveClose,
            Vision::new(8),
            Health::full_health(12),
            Name(format!("Orc number {}", name)),
        ))
        .id()
}

pub fn deer(world: &mut World, x: i32, y: i32) -> Entity {
    world
        .spawn((
            Monster,
            Position::new(x, y),
            Body::new(BodyType::Deer),
            Solid,
            MoveRandom,
            Health::full_health(8),
            Name("A deer".to_string()),
        ))
        .id()
}

pub fn spawn_body(commands: &mut Commands, x: i32, y: i32, name: &str) -> Entity {
    commands
        .spawn((
            Position::new(x, y),
            Body::new(BodyType::BonePile),
            Name(format!("Body of {}", name)),
        ))
        .id()
}
