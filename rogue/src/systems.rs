use bevy_ecs::prelude::{Changed, Query, Res, ResMut, With};
use rand::{prelude::StdRng, Rng};
use torchbearer::Map as FovMap;

use crate::{
    components::{FieldOfView, Player, Position, RandomMover},
    map::Map,
};

pub fn move_randomly(
    mut rng: ResMut<StdRng>,
    map: Res<Map>,
    query: Query<&mut Position, With<RandomMover>>,
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

pub fn field_of_view(
    map: Res<Map>,
    query: Query<(&mut FieldOfView, &Position), Changed<Position>>,
) {
    query.for_each_mut(|(mut field_of_view, position)| {
        field_of_view.visible_positions = torchbearer::fov::field_of_view(
            &*map,
            (position.x, position.y),
            field_of_view.view_distance,
        )
        .iter()
        .map(|&(x, y)| Position::new(x, y))
        .collect();
    });
}

pub fn update_map(mut map: ResMut<Map>, query: Query<&FieldOfView, With<Player>>) {
    map.reset_visible();
    query.for_each(|field_of_view| {
        map.reveal(&field_of_view.visible_positions);
    });
}
