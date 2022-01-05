use bevy_ecs::prelude::Entity;

pub struct MoveAction {
    pub entity: Entity,
    pub x: i32,
    pub y: i32,
}

pub struct AttackAction {
    pub entity: Entity,
    pub target: Entity,
}
