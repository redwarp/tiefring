use bevy_ecs::prelude::Query;

use crate::components::{LeftMover, Position};

pub fn move_left(query: Query<(&mut Position, &LeftMover)>) {
    query.for_each_mut(|(mut position, _)| {
        let new_x = (position.x - 1).rem_euclid(20);
        position.x = new_x;
    });
}
