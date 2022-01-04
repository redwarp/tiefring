use bevy_ecs::prelude::Entity;

pub struct MoveAction {
    pub entity: Entity,
    pub x: i32,
    pub y: i32,
}
