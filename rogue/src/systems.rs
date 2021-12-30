use bevy_ecs::prelude::{Changed, Query, Res, ResMut, With};
use rand::{prelude::StdRng, Rng};
use torchbearer::Map as FovMap;

use crate::{
    components::{Monster, MoveClose, MoveRandom, Name, Player, Position, Vision},
    game::PlayerData,
    map::Map,
};

pub fn move_random(
    mut rng: ResMut<StdRng>,
    map: Res<Map>,
    query: Query<&mut Position, With<MoveRandom>>,
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

        if map.is_walkable((x, y)) {
            position.x = x;
            position.y = y;
        }
    });
}

pub fn move_close(
    map: Res<Map>,
    player_data: Res<PlayerData>,
    query: Query<(&mut Position, &Vision), With<MoveClose>>,
) {
    query.for_each_mut(|(mut position, vision)| {
        if vision.visible_positions.contains(&player_data.position) {
            if let Some(path) = torchbearer::path::astar_path_fourwaygrid(
                &*map,
                (position.x, position.y),
                (player_data.position.x, player_data.position.y),
            ) {
                if let Some((x, y)) = path.into_iter().nth(1) {
                    position.x = x;
                    position.y = y;
                }
            }
        }
    })
}

pub fn field_of_view(map: Res<Map>, query: Query<(&mut Vision, &Position), Changed<Position>>) {
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

pub fn update_map(mut map: ResMut<Map>, query: Query<&Vision, With<Player>>) {
    map.reset_visible();
    query.for_each(|field_of_view| {
        map.reveal(&field_of_view.visible_positions);
    });
}

pub fn insult(player_data: Res<PlayerData>, query: Query<(&Vision, &Name), With<Monster>>) {
    query.for_each(|(vision, Name(name))| {
        if vision.visible_positions.contains(&player_data.position) {
            println!("{} insults you", name);
        }
    });
}
