use bevy_ecs::prelude::{Changed, Commands, Entity, Query, Res, ResMut, With};
use log::info;
use rand::Rng;
use torchbearer::path::PathMap;

use crate::{
    actions::{AttackAction, MoveAction},
    components::{
        Health, Monster, MoveClose, MoveRandom, Name, Player, Position, Solid, Stats, Vision,
    },
    effects::HealthEffect,
    game::{PlayerData, Random},
    map::Map,
    spawner,
    utils::find_path,
};

pub fn move_random(
    mut commands: Commands,
    mut rng: ResMut<Random>,
    mut query: Query<(Entity, &Position), With<MoveRandom>>,
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
        commands.spawn(move_action);
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
                        commands.spawn(move_action);
                    }
                }
            }
        }
    })
}

pub fn move_action(
    mut map: ResMut<Map>,
    move_actions: Query<&MoveAction>,
    mut positions: Query<(&mut Position, Option<&Solid>)>,
) {
    move_actions.for_each(|&MoveAction { entity, x, y }| {
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
    });
}

pub fn attack_action(
    mut commands: Commands,
    attack_actions: Query<&AttackAction>,
    attacker_stats: Query<&Stats>,
    health_stats: Query<&Health>,
) {
    attack_actions.for_each(|&AttackAction { attacker, target }| {
        if let Ok(stats) = attacker_stats.get(attacker) {
            if health_stats.get(target).is_ok() {
                commands.spawn(HealthEffect {
                    entity: target,
                    amount: -stats.strength,
                });
            }
        }
    });
}

pub fn health_effect(health_effects: Query<&HealthEffect>, mut health_stats: Query<&mut Health>) {
    health_effects.for_each(|&HealthEffect { entity, amount }| {
        if let Ok(mut health) = health_stats.get_mut(entity) {
            health.hp = (health.hp + amount).max(0).min(health.max_hp);
        }
    });
}

pub fn field_of_view(map: Res<Map>, mut query: Query<(&mut Vision, &Position), Changed<Position>>) {
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

pub fn update_player_position(
    mut player_data: ResMut<PlayerData>,
    query: Query<&Position, With<Player>>,
) {
    query.for_each(|position| {
        // We expect only one.
        player_data.position = *position;
    });
}

pub fn insult(player_data: Res<PlayerData>, query: Query<(&Vision, &Name), With<Monster>>) {
    query.for_each(|(vision, Name(name))| {
        if vision.visible_positions.contains(&player_data.position) {
            info!("{} insults you", name);
        }
    });
}

pub fn cleanup_actions(
    mut commands: Commands,
    move_query: Query<Entity, With<MoveAction>>,
    attack_query: Query<Entity, With<AttackAction>>,
) {
    move_query.for_each(|entity| commands.entity(entity).despawn());
    attack_query.for_each(|entity| commands.entity(entity).despawn());
}

pub fn cleanup_effects(mut commands: Commands, health_effects: Query<Entity, With<HealthEffect>>) {
    health_effects.for_each(|entity| commands.entity(entity).despawn());
}

pub fn death(mut commands: Commands, health_stats: Query<(Entity, &Health, &Position, &Name)>) {
    health_stats.for_each(|(entity, health, position, name)| {
        if health.hp <= 0 {
            spawner::spawn_body(&mut commands, position.x, position.y, &name.0);
            commands.entity(entity).despawn();
        }
    });
}
