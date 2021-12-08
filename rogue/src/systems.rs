use bevy_ecs::prelude::{Query, With};

use crate::components::{LeftMover, Position};

pub fn move_left(query: Query<&mut Position, With<LeftMover>>) {
    query.for_each_mut(|mut position| {
        let new_x = (position.x - 1).rem_euclid(20);
        position.x = new_x;
    });
}
