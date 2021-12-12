use bevy_ecs::prelude::{Query, Res, ResMut, With};
use rand::{prelude::StdRng, Rng};

use crate::{
    components::{LeftMover, Position},
    map::Map,
};

pub fn move_randomly(
    mut rng: ResMut<StdRng>,
    map: Res<Map>,
    query: Query<&mut Position, With<LeftMover>>,
) {
    query.for_each_mut(|mut position| {
        let direction = rng.gen_range(0..4);
        let (dx, dy) = match direction {
            0 => (0, 1),
            1 => (0, -1),
            2 => (1, 0),
            3 => (-1, 0),
            _ => panic!("Random direction is between 0 and 3 inclusive."),
        };
        let x = position.x + dx;
        let y = position.y + dy;

        if map.is_walkable(x, y) {
            position.x = x;
            position.y = y;
        }
    });
}
