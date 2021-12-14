use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_ecs::{
    prelude::World,
    schedule::{Schedule, SystemStage},
};
use rand::prelude::StdRng;
use rand::SeedableRng;
use torchbearer::Map as FovMap;

use crate::components::{Player, Position};
use crate::map::Map;
use crate::{inputs::Input, systems};

#[derive(PartialEq, Clone, Copy)]
pub enum RunState {
    Running,
    Paused,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Update {
    Refresh,
    Exit,
    NoOp,
}

pub struct Game {
    pub world: World,
    pub schedule: Schedule,
    stepper: Stepper,
    run_state: RunState,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, StageLabel)]
enum Stages {
    Update,
    Map,
}

impl Game {
    pub fn new(width: i32, height: i32) -> Self {
        let mut schedule = Schedule::default();
        schedule
            .add_stage(
                Stages::Update,
                SystemStage::parallel()
                    .with_system(systems::move_randomly.system())
                    .with_system(systems::field_of_view.system()),
            )
            .add_stage_after(
                Stages::Update,
                Stages::Map,
                SystemStage::parallel().with_system(systems::update_map.system()),
            );

        let stepper = Stepper::new(Duration::new(0, 200_000_000));
        let mut world = World::new();
        let map = Map::empty(width, height).surround().random_walls();
        world.insert_resource(map);
        let rng = StdRng::from_entropy();
        world.insert_resource(rng);

        let run_state = RunState::Paused;

        Self {
            world,
            schedule,
            stepper,
            run_state,
        }
    }

    pub fn update(&mut self, dt: Duration, input: Option<Input>) -> Update {
        match self.run_state {
            RunState::Running => {
                self.schedule.run(&mut self.world);
                self.run_state = RunState::Paused;
                Update::Refresh
            }
            RunState::Paused => {
                if let Some(Input::Escape) = input {
                    Update::Exit
                } else if self.try_move_player(&input) {
                    self.run_state = RunState::Running;
                    Update::Refresh
                } else {
                    Update::NoOp
                }
            }
        }
    }

    fn try_move_player(&mut self, input: &Option<Input>) -> bool {
        match input {
            Some(Input::Up) => self.move_player(0, -1),
            Some(Input::Down) => self.move_player(0, 1),
            Some(Input::Left) => self.move_player(-1, 0),
            Some(Input::Right) => self.move_player(1, 0),
            _ => false,
        }
    }

    fn move_player(&mut self, dx: i32, dy: i32) -> bool {
        let mut moved = false;
        self.world.resource_scope(|world, map: Mut<Map>| {
            world
                .query_filtered::<&mut Position, With<Player>>()
                .for_each_mut(world, |mut position| {
                    let x = position.x + dx;
                    let y = position.y + dy;
                    if map.is_walkable(x, y) {
                        position.x = x;
                        position.y = y;
                        moved = true;
                    }
                });
        });
        moved
    }
}

struct Stepper {
    dt: Duration,
    step: Duration,
}

impl Stepper {
    fn new(step: Duration) -> Self {
        Self {
            dt: Duration::new(0, 0),
            step,
        }
    }

    fn advance(&mut self, dt: Duration) -> bool {
        self.dt += dt;

        if self.dt >= self.step {
            loop {
                self.dt -= self.step;
                if self.dt < self.step {
                    break;
                }
            }
            true
        } else {
            false
        }
    }
}
