use bevy_ecs::prelude::Entity;

/// An effect that modifies health. If the amount is positif, akin to healing.
/// If negative, you are taking damage.
pub struct HealthEffect {
    pub entity: Entity,
    pub amount: i32,
}
