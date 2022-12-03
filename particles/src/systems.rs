use std::time::Duration;

use bevy_ecs::{
    prelude::{Component, Entity},
    query::With,
    system::{Commands, Query, Res, ResMut},
};
use rand::Rng;

use crate::{Random, Shared};

#[derive(Component)]
pub struct ParticleLifetime {
    lifetime: Duration,
    current_lifetime: Duration,
    speed: f32,
}

impl ParticleLifetime {
    pub fn freshness(&self) -> f32 {
        self.current_lifetime.as_secs_f32() / self.lifetime.as_secs_f32()
    }
}

#[derive(Component)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Component)]
pub struct SpawnCommand;

pub fn spawn_particles(
    mut commands: Commands,
    query: Query<Entity, With<SpawnCommand>>,
    mut random: ResMut<Random>,
    shared: Res<Shared>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();

        let life = 5;
        let speed = shared.size.height as f32 / life as f32;

        commands.spawn((
            Position {
                x: random.gen_range(0.0..(shared.size.width as f32) - 10.0),
                y: 20.0,
            },
            ParticleLifetime {
                lifetime: Duration::from_secs(life),
                current_lifetime: Duration::from_secs(life),
                speed: random.gen_range(speed - 20.0..speed + 20.0),
            },
        ));
    }
}

pub fn update_particles(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ParticleLifetime, &mut Position)>,
    time: Res<Shared>,
) {
    for (entity, mut lifetime, mut position) in query.iter_mut() {
        if lifetime.current_lifetime < time.elapsed_between_redraw {
            commands.entity(entity).despawn();
        } else {
            // Update!
            lifetime.current_lifetime -= time.elapsed_between_redraw;

            let moved = time.elapsed_between_redraw.as_millis() as f32 / 1000.0 * lifetime.speed;

            position.y += moved;
        }
    }
}
