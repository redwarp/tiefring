use bevy_ecs::prelude::{Component, Entity};

/// An effect that modifies health. If the amount is positif, akin to healing.
/// If negative, you are taking damage.
#[derive(Component)]
pub struct HealthEffect {
    pub entity: Entity,
    pub amount: i32,
}
