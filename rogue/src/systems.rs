use bevy_ecs::prelude::{Changed, Commands, Entity, Query, Res, ResMut, With};
use rand::{prelude::StdRng, Rng};
use torchbearer::path::PathMap;

use crate::{
    actions::MoveAction,
    components::{Monster, MoveClose, MoveRandom, Name, Player, Position, Solid, Vision},
    game::PlayerData,
    map::Map,
    utils::find_path,
};

pub fn move_random(
    mut commands: Commands,
    mut rng: ResMut<StdRng>,
    query: Query<(Entity, &Position), With<MoveRandom>>,
) {
    query.for_each_mut(|(entity, position)| {
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

        let move_action = MoveAction { entity, x, y };
        commands.spawn().insert(move_action);
    });
}

pub fn move_to_player(
    mut commands: Commands,
    map: ResMut<Map>,
    player_data: Res<PlayerData>,
    query: Query<(Entity, &Position, &Vision), With<MoveClose>>,
) {
    query.for_each(|(entity, position, vision)| {
        if vision.visible_positions.contains(&player_data.position) {
            if let Some(path) = find_path(
                &*map,
                (position.x, position.y),
                (player_data.position.x, player_data.position.y),
            ) {
                if let Some((x, y)) = path.into_iter().nth(1) {
                    let new_position = Position { x, y };
                    if player_data.position != new_position {
                        let move_action = MoveAction { entity, x, y };
                        commands.spawn().insert(move_action);
                    }
                }
            }
        }
    })
}

pub fn move_action(
    mut map: ResMut<Map>,
    mut commands: Commands,
    move_actions: Query<(Entity, &MoveAction)>,
    mut positions: Query<(&mut Position, Option<&Solid>)>,
) {
    move_actions.for_each(|(action_entity, &MoveAction { entity, x, y })| {
        if let Ok((mut position, solid)) = positions.get_mut(entity) {
            if map.is_walkable((x, y)) {
                if solid.is_some() {
                    let previous_index = map.index_from_position(&position);
                    let new_index = map.index(x, y);

                    map.blocked[previous_index] = false;
                    map.blocked[new_index] = true;
                }

                position.x = x;
                position.y = y;
            }
        }

        commands.entity(action_entity).despawn();
    });
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

pub fn update_visible(mut map: ResMut<Map>, query: Query<&Vision, With<Player>>) {
    map.reset_visible();
    query.for_each(|field_of_view| {
        map.reveal(&field_of_view.visible_positions);
    });
}

pub fn update_blocked(mut map: ResMut<Map>, query: Query<&Position, With<Solid>>) {
    map.reset_blocked();
    query.for_each(|position| {
        let index = map.index_from_position(position);
        map.blocked[index] = true;
    });
}

pub fn insult(player_data: Res<PlayerData>, query: Query<(&Vision, &Name), With<Monster>>) {
    query.for_each(|(vision, Name(name))| {
        if vision.visible_positions.contains(&player_data.position) {
            println!("{} insults you", name);
        }
    });
}
