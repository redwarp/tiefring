use bevy_ecs::prelude::{Component, Entity};

#[derive(Component)]
pub struct MoveAction {
    pub entity: Entity,
    pub x: i32,
    pub y: i32,
}

#[derive(Component)]
pub struct AttackAction {
    pub attacker: Entity,
    pub target: Entity,
}
